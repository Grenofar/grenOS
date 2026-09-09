import { Router } from "@grenos/router";
import { config, log } from "./config.ts";
import { db, emit } from "./db.ts";
import { parseEnvelope, EnvelopeError, type AgentAction } from "./envelope.ts";
import { authorize } from "./sandbox.ts";
import type { GitHub } from "./github.ts";
import type { AgentDefinition } from "./prompts.ts";

/**
 * Runs a single task with a single agent, from envelope to commit.
 *
 * The order of operations matters and is not negotiable:
 *
 *   authorise every write  ->  take every lease  ->  commit once
 *
 * Authorisation before leases means a task that was going to be rejected never
 * locks a file. Leases before the commit means two agents cannot interleave
 * writes to the same path. One commit at the end means a task is atomic: it
 * either landed or it did not, and CI never sees a half-applied task.
 */

export interface TaskRow {
  id: string;
  mission_id: string;
  assigned_to: string;
  goal: string;
  acceptance_criteria: string[];
  context_refs: string[];
  allowed_paths: string[];
  attempt: number;
  max_attempts: number;
  token_budget: number;
  failure_detail: string | null;
  branch: string | null;
}

export async function executeTask(
  task: TaskRow,
  agent: AgentDefinition,
  router: Router,
  gh: GitHub,
): Promise<void> {
  log.info(`▶ ${agent.id} · ${task.goal.slice(0, 70)}`);

  const branch = task.branch ?? `agent/${task.id.slice(0, 8)}`;
  const baseSha = await gh.ensureBranch(branch);

  // Task paths narrow the agent's own permissions; they never widen them
  // (agents/README.md §3).
  const allowedPaths =
    task.allowed_paths.length > 0 ? task.allowed_paths : agent.allowedPaths;

  let envelope;
  let modelUsed = "";
  try {
    const result = await router.complete({
      role: agent.modelRole,
      system: agent.systemPrompt,
      messages: [{ role: "user", content: await buildPrompt(task, allowedPaths, gh, branch) }],
      maxOutputTokens: 8192,
      json: true,
    });
    modelUsed = result.model;

    await db.rpc("add_mission_tokens", {
      p_mission_id: task.mission_id,
      p_task_id: task.id,
      p_tokens: result.tokensIn + result.tokensOut,
    });

    envelope = parseEnvelope(result.text);

    await db.from("messages").insert({
      mission_id: task.mission_id,
      task_id: task.id,
      from_agent: agent.id,
      to_agent: "master",
      kind: "result",
      content: envelope as unknown as Record<string, unknown>,
      model: result.model,
      tokens_in: result.tokensIn,
      tokens_out: result.tokensOut,
      latency_ms: result.latencyMs,
    });
  } catch (err) {
    const detail = err instanceof Error ? err.message : String(err);

    // A malformed envelope is the agent's fault; a router exhaustion is not.
    // Only the first should consume an attempt, otherwise a quiet afternoon of
    // rate limits would burn every retry a task has.
    const isProvider = !(err instanceof EnvelopeError);
    await failTask(task, isProvider ? "provider_error" : "spec_gap", detail, !isProvider);
    return;
  }

  // ---- Authorise everything before touching anything -----------------------
  const writes = envelope.actions.filter(isWrite);
  const changes: Array<{ path: string; content: string | null }> = [];

  for (const action of writes) {
    const verdict = authorize(action.path, {
      allowedPaths,
      forbiddenPaths: agent.forbiddenPaths,
      canWrite: agent.canWrite,
    });

    if (!verdict.ok) {
      await emit({
        missionId: task.mission_id,
        taskId: task.id,
        agentId: agent.id,
        level: "error",
        type: "policy_violation",
        message: verdict.reason,
        payload: { path: action.path },
      });
      await failTask(task, "policy_violation", verdict.reason, true);
      return;
    }

    if (action.type === "write_file") {
      changes.push({ path: verdict.path, content: action.content });
    } else if (action.type === "delete_file") {
      changes.push({ path: verdict.path, content: null });
    } else {
      const current = await gh.readFile(verdict.path, branch);
      if (current === null) {
        await failTask(
          task,
          "spec_gap",
          `patch_file sur un fichier inexistant : ${verdict.path}`,
          true,
        );
        return;
      }
      const occurrences = current.split(action.old_str).length - 1;
      if (occurrences !== 1) {
        // Ambiguous or absent: applying it would edit the wrong place, or
        // every place. Both are worse than failing the attempt.
        await failTask(
          task,
          "spec_gap",
          `old_str apparaît ${occurrences} fois dans ${verdict.path} (attendu : 1)`,
          true,
        );
        return;
      }
      changes.push({
        path: verdict.path,
        content: current.replace(action.old_str, action.new_str),
      });
    }
  }

  // ---- Take every lease, or none ------------------------------------------
  if (changes.length > 0) {
    const { data: conflicts, error } = await db.rpc("acquire_leases", {
      p_task_id: task.id,
      p_agent_id: agent.id,
      p_paths: changes.map((c) => c.path),
      p_ttl_s: Math.floor(config.leaseTtlMs / 1000),
    });

    if (error) throw new Error(`acquire_leases: ${error.message}`);

    if (Array.isArray(conflicts) && conflicts.length > 0) {
      // Not a failure: the Master simply sequenced two tasks too closely.
      // Back to ready, no attempt consumed.
      await db.from("tasks").update({ status: "ready" }).eq("id", task.id);
      await emit({
        missionId: task.mission_id,
        taskId: task.id,
        agentId: agent.id,
        level: "warn",
        type: "lease_conflict",
        message: `Fichiers déjà verrouillés : ${conflicts.join(", ")}`,
      });
      return;
    }
  }

  // ---- Commit once ---------------------------------------------------------
  let commitSha: string | null = null;
  if (changes.length > 0) {
    const commit = await gh.commit({
      branch,
      message: `${agent.id}: ${firstLine(envelope.summary)} (task ${task.id.slice(0, 8)})`,
      changes,
    });
    commitSha = commit?.sha ?? null;
  }

  await db.rpc("release_leases", { p_task_id: task.id });

  // ---- Record the outcome --------------------------------------------------
  const wantsVerification =
    envelope.actions.some(
      (a) => a.type === "request_build" || a.type === "request_test",
    ) || changes.length > 0;

  const escalation = envelope.actions.find((a) => a.type === "escalate");
  const help = envelope.actions.find((a) => a.type === "request_help");

  let status: string;
  if (envelope.status === "failed") status = "failed";
  else if (escalation || help) status = "blocked";
  else if (wantsVerification) status = "awaiting_verification";
  else status = "done";

  await db
    .from("tasks")
    .update({
      status,
      branch,
      result: envelope as unknown as Record<string, unknown>,
      ...(status === "done" || status === "failed"
        ? { finished_at: new Date().toISOString() }
        : {}),
    })
    .eq("id", task.id);

  // Proposals from a worker are just proposals. Only the Master turns them
  // into real tasks (agents/README.md §4), so they are recorded and left for
  // the next Master cycle to judge.
  for (const action of envelope.actions) {
    if (action.type === "propose_task") {
      await db.from("messages").insert({
        mission_id: task.mission_id,
        task_id: task.id,
        from_agent: agent.id,
        to_agent: "master",
        kind: "proposal",
        content: action as unknown as Record<string, unknown>,
      });
    }
  }

  await emit({
    missionId: task.mission_id,
    taskId: task.id,
    agentId: agent.id,
    level: status === "failed" ? "warn" : "info",
    type: `task_${status}`,
    message: envelope.summary,
    payload: {
      model: modelUsed,
      files: changes.length,
      commit: commitSha,
      baseSha,
      ...(escalation && escalation.type === "escalate"
        ? { escalation: escalation.reason }
        : {}),
    },
  });

  log.info(`  ${status} · ${changes.length} fichier(s) · ${modelUsed}`);
}

async function buildPrompt(
  task: TaskRow,
  allowedPaths: string[],
  gh: GitHub,
  branch: string,
): Promise<string> {
  const parts: string[] = [];

  parts.push("# Task envelope\n");
  parts.push(
    JSON.stringify(
      {
        id: task.id,
        goal: task.goal,
        acceptance_criteria: task.acceptance_criteria,
        allowed_paths: allowedPaths,
        attempt: task.attempt,
        max_attempts: task.max_attempts,
      },
      null,
      2,
    ),
  );

  // Retries must differ from the attempt that failed, so the previous failure
  // is quoted verbatim rather than summarised (agents/README.md §7).
  if (task.attempt > 1 && task.failure_detail) {
    parts.push(
      `\n# Previous attempt failed\n\n${task.failure_detail}\n\n` +
        "Change your approach. Repeating the previous attempt is itself a failure.",
    );
  }

  const files = task.context_refs
    .filter((ref) => ref.startsWith("file:"))
    .map((ref) => ref.slice(5));

  if (files.length > 0) {
    parts.push("\n# Current file contents\n");
    for (const path of files.slice(0, 12)) {
      const content = await gh.readFile(path, branch);
      parts.push(
        content === null
          ? `\n## ${path}\n(does not exist yet)`
          : `\n## ${path}\n\`\`\`\n${content.slice(0, 40_000)}\n\`\`\``,
      );
    }
  }

  parts.push(
    "\n# Answer\n\nReturn exactly one JSON object as specified in the protocol. " +
      "No prose, no code fence.",
  );

  return parts.join("\n");
}

async function failTask(
  task: TaskRow,
  failure: string,
  detail: string,
  consumesAttempt: boolean,
): Promise<void> {
  const nextAttempt = task.attempt + 1;
  const canRetry = consumesAttempt && nextAttempt <= task.max_attempts;

  await db
    .from("tasks")
    .update({
      status: canRetry ? "ready" : consumesAttempt ? "failed" : "ready",
      ...(consumesAttempt ? { attempt: nextAttempt } : {}),
      failure,
      failure_detail: detail.slice(0, 4000),
      ...(consumesAttempt && !canRetry ? { finished_at: new Date().toISOString() } : {}),
    })
    .eq("id", task.id);

  await db.rpc("release_leases", { p_task_id: task.id });

  await emit({
    missionId: task.mission_id,
    taskId: task.id,
    agentId: task.assigned_to,
    level: "warn",
    type: failure,
    message: detail.slice(0, 500),
  });

  log.warn(`  ✕ ${failure}: ${detail.slice(0, 160)}`);
}

const isWrite = (
  a: AgentAction,
): a is Extract<AgentAction, { type: "write_file" | "patch_file" | "delete_file" }> =>
  a.type === "write_file" || a.type === "patch_file" || a.type === "delete_file";

const firstLine = (s: string): string => s.split("\n")[0]!.slice(0, 60);
