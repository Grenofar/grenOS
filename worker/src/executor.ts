import { Router } from "@grenos/router";
import { config, log } from "./config.ts";
import { db, emit } from "./db.ts";
import { parseEnvelope, EnvelopeError, type AgentAction } from "./envelope.ts";
import { authorize } from "./sandbox.ts";
import { repoContext } from "./context.ts";
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
  // Read from the agent's branch if it exists, otherwise from the default
  // branch. The branch itself is created by the first commit (github.ts),
  // never up front: creating it early is a push of main's content, which ran
  // CI on a branch holding none of the agent's work and produced a verdict
  // that burned an attempt before a single line had been written.
  const readRef = await gh.resolveRef(branch);

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
      messages: [
        { role: "user", content: await buildPrompt(task, agent, allowedPaths, gh, readRef) },
      ],
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
      const current = await gh.readFile(verdict.path, readRef);
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
  // Documentation-only work goes straight to the default branch. CI never
  // verifies it, and left on a task branch nobody merges it is invisible to
  // every other agent: the Coder's branch is cut from main, so the Architect's
  // plan for mission 1 sat on agent/4002c138 and the Coder never saw it. The
  // rule is docs/ only — code under apps/ or packages/ must never reach main
  // this way, since nothing would have verified it.
  const docsOnly = changes.length > 0 && changes.every((c) => c.path.startsWith("docs/"));
  const commitBranch = docsOnly ? await gh.defaultBranch() : branch;

  let commitSha: string | null = null;
  if (changes.length > 0) {
    const commit = await gh.commit({
      branch: commitBranch,
      message: `${agent.id}: ${firstLine(envelope.summary)} (task ${task.id.slice(0, 8)})`,
      changes,
    });
    commitSha = commit?.sha ?? null;
  }

  await db.rpc("release_leases", { p_task_id: task.id });

  // ---- Record the outcome --------------------------------------------------
  // Only work CI can actually judge goes to CI.
  //
  // Sending every diff for verification looks safe but is not: the pipeline
  // builds the kernel and nothing else, so a documentation-only task — the
  // Architect's normal output — comes back "no kernel to verify", fails, and
  // burns one of its three attempts on a question nobody asked. Design
  // documents are judged by the Master reading them, not by a compiler.
  const VERIFIED_PREFIXES = ["kernel/", "tests/"];
  const touchesVerifiableCode = changes.some((c) =>
    VERIFIED_PREFIXES.some((prefix) => c.path.startsWith(prefix)),
  );
  const wantsVerification =
    envelope.actions.some(
      (a) => a.type === "request_build" || a.type === "request_test",
    ) || touchesVerifiableCode;

  const escalation = envelope.actions.find((a) => a.type === "escalate");
  const help = envelope.actions.find((a) => a.type === "request_help");

  // An agent reporting failure is being honest, and honesty must not be
  // terminal while attempts remain. Marking it failed outright meant one
  // confused reply killed a task that still had retries — which is exactly
  // what happened when the model had been fed a provider error as its own.
  // Routed as spec_gap, per the Coder protocol: "cannot be done as specified".
  if (envelope.status === "failed") {
    const detail = [envelope.summary, envelope.reasoning_brief].filter(Boolean).join("\n\n");
    await failTask(task, "spec_gap", detail, true);
    return;
  }

  let status: string;
  if (escalation || help) status = "blocked";
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
      readRef,
      ...(escalation && escalation.type === "escalate"
        ? { escalation: escalation.reason }
        : {}),
    },
  });

  log.info(`  ${status} · ${changes.length} fichier(s) · ${modelUsed}`);
}

async function buildPrompt(
  task: TaskRow,
  agent: AgentDefinition,
  allowedPaths: string[],
  gh: GitHub,
  readRef: string,
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

  // The repository as it stands on the agent's branch, plus the design
  // documents (context.ts). Before this, agents never saw a single file.
  parts.push(await repoContext(gh, agent, allowedPaths, readRef));

  // Files the Master named explicitly, beyond what the view above shows.
  const files = task.context_refs
    .filter((ref) => ref.startsWith("file:"))
    .map((ref) => ref.slice(5));

  if (files.length > 0) {
    parts.push("\n# Current file contents\n");
    for (const path of files.slice(0, 12)) {
      const content = await gh.readFile(path, readRef);
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
  const patch = failurePatch(task, failure, detail, consumesAttempt);

  const { error } = await db.from("tasks").update(patch).eq("id", task.id);
  if (error) {
    // If we cannot even record the failure, say so loudly: silence here means
    // a task that no one will ever pick up again.
    log.warn(`impossible de marquer la tâche ${task.id.slice(0, 8)}: ${error.message}`);
  }

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

/**
 * What a failure writes to the task row. Pure, so the rule that matters most
 * here is tested without a database.
 *
 * An attempt-neutral failure (no model answered, or anything else outside the
 * agent's control) writes the status and nothing else. failure_detail is
 * quoted verbatim into the next attempt's prompt as "Previous attempt failed":
 * overwriting the real CI log with "No model available" made the model
 * conclude it could not work, report failed, and take a healthy task down;
 * the Master then escalated a provider outage as a broken Coder.
 *
 * `attempt` only advances when another attempt follows. Writing
 * max_attempts + 1 on the final failure violates tasks_attempt_within_limit,
 * the UPDATE is rejected, and the task stays in_progress forever.
 */
export function failurePatch(
  task: Pick<TaskRow, "attempt" | "max_attempts">,
  failure: string,
  detail: string,
  consumesAttempt: boolean,
  now: Date = new Date(),
): Record<string, unknown> {
  if (!consumesAttempt) return { status: "ready" };

  const canRetry = task.attempt < task.max_attempts;
  const patch: Record<string, unknown> = {
    status: canRetry ? "ready" : "failed",
    failure,
    failure_detail: detail.slice(0, 4000),
  };
  if (canRetry) patch["attempt"] = task.attempt + 1;
  else patch["finished_at"] = now.toISOString();
  return patch;
}
