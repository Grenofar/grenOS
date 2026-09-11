import { Router } from "@grenos/router";
import { config, log } from "./config.ts";
import { db, emit } from "./db.ts";
import { parseEnvelope, EnvelopeError, type AgentAction, type AgentEnvelope } from "./envelope.ts";
import { authorize } from "./sandbox.ts";
import { repoContext } from "./context.ts";
import { renderEvidence } from "./evidence.ts";
import { consult, CONSULT_ROUNDS } from "./consult.ts";
import {
  crateRoots,
  preflight,
  renderPreflight,
  renderUnreadable,
  PREFLIGHT_ROUNDS,
} from "./preflight.ts";
import { checkDependencies, checkEditions, crateReleases, crateVersions } from "./deps.ts";
import type { GitHub } from "./github.ts";
import type { AgentDefinition } from "./prompts.ts";

/**
 * Runs a single task with a single agent, from envelope to commit.
 *
 * The order of operations matters and is not negotiable:
 *
 *   answer  ->  authorise every write  ->  pre-flight  ->  take every lease  ->  commit once
 *
 * Authorisation before leases means a task that was going to be rejected never
 * locks a file. Leases before the commit means two agents cannot interleave
 * writes to the same path. One commit at the end means a task is atomic: it
 * either landed or it did not, and CI never sees a half-applied task.
 *
 * The answer is a short conversation, not a single call. The agent may first
 * ask to read documentation instead of guessing an API (consult.ts), and a
 * mistake it can fix — a misspelled file name, a crate version that was never
 * published, a patch that does not apply, a path it was not given — goes
 * straight back to it with the list of problems (preflight.ts, deps.ts). All
 * of it happens inside the attempt; before, each one cost a CI run or the
 * attempt itself.
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

type Change = { path: string; content: string | null };
type Violation = { path: string; reason: string };

export async function executeTask(
  task: TaskRow,
  agent: AgentDefinition,
  router: Router,
  gh: GitHub,
): Promise<void> {
  log.info(`▶ ${agent.id} · ${task.goal.slice(0, 70)}`);

  const branch = task.branch ?? `agent/${task.id.slice(0, 8)}`;
  // The work this task builds on (lineage.ts): the mission's latest writer
  // branch, or the one the Master named. Starting from main threw away
  // everything that was not green yet.
  const base = task.context_refs.find((r) => r.startsWith("base:"))?.slice(5) ?? null;
  // Read from the agent's branch if it exists, then from its base, then from
  // the default branch. The branch itself is created by the first commit
  // (github.ts), never up front: creating it early is a push, which ran CI on
  // a branch holding none of the agent's work and produced a verdict that
  // burned an attempt before a single line had been written.
  const readRef = await gh.resolveRef(branch, base);

  // Task paths narrow the agent's own permissions; they never widen them
  // (agents/README.md §3).
  const allowedPaths =
    task.allowed_paths.length > 0 ? task.allowed_paths : agent.allowedPaths;

  // ---- Answer: read if needed, fix what pre-flight finds -------------------
  const conversation: Array<{ role: "user" | "assistant"; content: string }> = [
    { role: "user", content: await buildPrompt(task, agent, allowedPaths, gh, branch, readRef) },
  ];

  let envelope: AgentEnvelope | null = null;
  let changes: Change[] = [];
  let modelUsed = "";
  let usage = { tokensIn: 0, tokensOut: 0, latencyMs: 0 };
  let consulted = 0;
  let corrected = 0;

  for (;;) {
    let text = "";
    try {
      const result = await router.complete({
        role: agent.modelRole,
        system: agent.systemPrompt,
        messages: conversation,
        maxOutputTokens: 8192,
        json: true,
      });
      modelUsed = result.model;
      usage = {
        tokensIn: usage.tokensIn + result.tokensIn,
        tokensOut: usage.tokensOut + result.tokensOut,
        latencyMs: usage.latencyMs + result.latencyMs,
      };

      await db.rpc("add_mission_tokens", {
        p_mission_id: task.mission_id,
        p_task_id: task.id,
        p_tokens: result.tokensIn + result.tokensOut,
      });

      text = result.text;
      envelope = parseEnvelope(text);
    } catch (err) {
      const detail = err instanceof Error ? err.message : String(err);

      // An answer that cannot be read is a slip the model can fix at once,
      // like a pre-flight problem. On 2026-09-11 a Coder answer finally came
      // through an hour of provider outage and was lost to one misplaced
      // character. It comes back with the parser's reason, within the same
      // corrections budget.
      if (err instanceof EnvelopeError && text && corrected < PREFLIGHT_ROUNDS) {
        corrected += 1;
        log.info(`  réponse illisible · renvoyée à l'agent (${detail.slice(0, 80)})`);
        await emit({
          missionId: task.mission_id,
          taskId: task.id,
          agentId: agent.id,
          level: "info",
          type: "unreadable_answer",
          message: detail.slice(0, 500),
        });
        conversation.push(
          { role: "assistant", content: text },
          { role: "user", content: renderUnreadable(detail, PREFLIGHT_ROUNDS - corrected) },
        );
        continue;
      }

      // A malformed envelope is the agent's fault; a router exhaustion is not.
      // Only the first should consume an attempt, otherwise a quiet afternoon of
      // rate limits would burn every retry a task has.
      const isProvider = !(err instanceof EnvelopeError);
      await failTask(task, isProvider ? "provider_error" : "spec_gap", detail, !isProvider);
      return;
    }

    // The agent asks to read before it writes.
    const asks = envelope.actions.filter(isConsult);
    if (asks.length > 0 && consulted < CONSULT_ROUNDS) {
      consulted += 1;
      log.info(`  consulte · ${asks.map((a) => a.url).join(" · ").slice(0, 200)}`);
      conversation.push(
        { role: "assistant", content: text },
        { role: "user", content: await consult(asks, CONSULT_ROUNDS - consulted) },
      );
      continue;
    }

    const resolved = await resolveWrites(envelope, agent, allowedPaths, gh, readRef);

    // The sandbox is the law: nothing outside the allowed paths is ever
    // written, and every attempt is logged. But one stray path — deleting a
    // junk file the task did not list, on mission 1 — used to throw away the
    // whole answer and the attempt with it. The agent is told which paths it
    // has, and answers again; only persisting fails the task.
    for (const v of resolved.violations) {
      await emit({
        missionId: task.mission_id,
        taskId: task.id,
        agentId: agent.id,
        level: "warn",
        type: "policy_violation",
        message: v.reason,
        payload: { path: v.path },
      });
    }

    // An agent reporting failure is not asked to polish its files first.
    // Otherwise the rest of each crate it touches is read from the branch:
    // whether a crate has a panic handler, or which toolchain builds it, can
    // depend on files the answer leaves alone (D-027).
    const crate =
      envelope.status === "failed" ? null : await crateContext(gh, readRef, resolved.changes);
    const problems =
      envelope.status === "failed"
        ? []
        : [
            ...resolved.violations.map(
              (v) =>
                `${v.path}: outside your allowed paths (${allowedPaths.join(", ")}). Remove that action. ` +
                "If the task cannot be done without it, return failed and name the path you need.",
            ),
            ...resolved.problems,
            ...preflight(resolved.changes, crate),
            ...(await checkDependencies(resolved.changes, crateVersions)),
            ...(await checkEditions(resolved.changes, crate, crateReleases)),
          ];
    if (asks.length > 0 && resolved.changes.length === 0 && envelope.status !== "failed") {
      problems.push(
        "you asked to consult again, but no consultation is left in this attempt: answer with your complete work now",
      );
    }

    if (problems.length === 0) {
      changes = resolved.changes;
      break;
    }

    if (corrected < PREFLIGHT_ROUNDS) {
      corrected += 1;
      log.info(`  pré-vol · ${problems.length} problème(s) renvoyé(s) à l'agent`);
      await emit({
        missionId: task.mission_id,
        taskId: task.id,
        agentId: agent.id,
        level: "info",
        type: "preflight",
        message: problems.join(" · ").slice(0, 500),
      });
      conversation.push(
        { role: "assistant", content: text },
        { role: "user", content: renderPreflight(problems, PREFLIGHT_ROUNDS - corrected) },
      );
      continue;
    }

    // Still wrong after every correction: the attempt is spent, a CI run is not.
    await failTask(
      task,
      resolved.violations.length > 0 ? "policy_violation" : "compile_error",
      `Pré-vol toujours en échec après ${PREFLIGHT_ROUNDS} corrections :\n- ${problems.join("\n- ")}`,
      true,
    );
    return;
  }

  if (!envelope) return;

  await db.from("messages").insert({
    mission_id: task.mission_id,
    task_id: task.id,
    from_agent: agent.id,
    to_agent: "master",
    kind: "result",
    content: envelope as unknown as Record<string, unknown>,
    model: modelUsed,
    tokens_in: usage.tokensIn,
    tokens_out: usage.tokensOut,
    latency_ms: usage.latencyMs,
  });

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
      base: docsOnly ? null : base,
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
  // A verifier that wrote nothing has produced a report, not code. There is no
  // commit for CI to judge, so waiting for a verdict would strand the task in
  // awaiting_verification for ever — the fate of every Tester and Review task
  // autopilot.ts creates, since both are told to write no file.
  const reportOnly = agent.roleClass === "verifier" && changes.length === 0;
  const wantsVerification =
    !reportOnly &&
    (envelope.actions.some((a) => a.type === "request_build" || a.type === "request_test") ||
      touchesVerifiableCode);

  const escalation = envelope.actions.find((a) => a.type === "escalate");
  const help = envelope.actions.find((a) => a.type === "request_help");

  // An agent reporting failure is saying the task cannot be done as specified
  // (the Coder protocol's spec_gap). The answer is a better specification,
  // which only the Master routes (agents/README.md §8): the same envelope sent
  // back to the same agent gets the same answer. Mission 1 did exactly that —
  // the Coder spent an attempt repeating itself, while the Master's corrected
  // task was refused as a duplicate because the old one still counted as open.
  // So the task is parked instead; see specGapPatch.
  if (envelope.status === "failed") {
    const detail = [envelope.summary, envelope.reasoning_brief].filter(Boolean).join("\n\n");
    await recordFailure(task, "spec_gap", detail, specGapPatch(detail));
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
      consulted,
      corrected,
      ...(escalation && escalation.type === "escalate"
        ? { escalation: escalation.reason }
        : {}),
    },
  });

  log.info(`  ${status} · ${changes.length} fichier(s) · ${modelUsed}`);
}

/**
 * Authorise every write and turn the envelope into file contents.
 *
 * A write outside the allowed paths is never applied: it is set aside as a
 * violation and reported. A patch that does not apply is a mechanical mistake
 * the agent can fix in seconds, so it is reported with the pre-flight
 * problems rather than failing the attempt.
 */
async function resolveWrites(
  envelope: AgentEnvelope,
  agent: AgentDefinition,
  allowedPaths: string[],
  gh: GitHub,
  readRef: string,
): Promise<{ changes: Change[]; problems: string[]; violations: Violation[] }> {
  const changes: Change[] = [];
  const problems: string[] = [];
  const violations: Violation[] = [];

  for (const action of envelope.actions.filter(isWrite)) {
    const verdict = authorize(action.path, {
      allowedPaths,
      forbiddenPaths: agent.forbiddenPaths,
      canWrite: agent.canWrite,
    });
    if (!verdict.ok) {
      violations.push({ path: action.path, reason: verdict.reason });
      continue;
    }

    const earlier = changes.find((c) => c.path === verdict.path);

    if (action.type === "write_file" || action.type === "delete_file") {
      const content = action.type === "write_file" ? action.content : null;
      if (earlier) earlier.content = content;
      else changes.push({ path: verdict.path, content });
      continue;
    }

    // A patch applies to the file as this envelope has left it so far, and
    // otherwise to the branch.
    const current = earlier ? earlier.content : await gh.readFile(verdict.path, readRef);
    if (current === null) {
      problems.push(`patch_file on ${verdict.path}: the file does not exist. Create it with write_file.`);
      continue;
    }
    const occurrences = current.split(action.old_str).length - 1;
    if (occurrences !== 1) {
      // Ambiguous or absent: applying it would edit the wrong place, or every
      // place. Both are worse than asking again.
      problems.push(
        `patch_file on ${verdict.path}: old_str appears ${occurrences} times and must appear exactly once. ` +
          "Copy it verbatim from the file shown, with enough surrounding lines to be unique, or use write_file.",
      );
      continue;
    }
    // A function replacer: a string one would expand "$1" or "$&" inside
    // new_str, and shell scripts and Rust macros are full of dollars.
    const next = current.replace(action.old_str, () => action.new_str);
    if (earlier) earlier.content = next;
    else changes.push({ path: verdict.path, content: next });
  }

  return { changes, problems, violations };
}

/**
 * The files of each crate an answer touches that the answer itself leaves
 * alone, read from the ref it builds on (D-027): whether a crate has a panic
 * handler, or which toolchain builds it, depends on them. `null` when they
 * cannot all be read, so pre-flight skips those checks instead of guessing.
 */
async function crateContext(
  gh: GitHub,
  readRef: string,
  changes: Change[],
): Promise<Change[] | null> {
  const roots = crateRoots(changes);
  if (roots.length === 0) return [];
  const changed = new Set(changes.map((c) => c.path));

  try {
    const wanted = (await gh.listTree(readRef)).filter(
      ({ path }) => !changed.has(path) && roots.some((root) => inCrate(path, root)),
    );
    // A crate too large to read whole is left to CI: a panic handler in the
    // file left out would be reported missing.
    if (wanted.length > 80 || wanted.some((f) => f.size > 100_000)) return null;

    const files: Change[] = [];
    for (const { path } of wanted) {
      const content = await gh.readFile(path, readRef);
      if (content !== null) files.push({ path, content });
    }
    return files;
  } catch (err) {
    log.warn(`pré-vol : ${readRef} illisible — ${err instanceof Error ? err.message : err}`);
    return null;
  }
}

function inCrate(path: string, root: string): boolean {
  const prefix = root ? `${root}/` : "";
  return (
    path === `${prefix}Cargo.toml` ||
    path === `${prefix}Cargo.lock` ||
    path === `${prefix}rust-toolchain.toml` ||
    (path.startsWith(`${prefix}src/`) && path.endsWith(".rs"))
  );
}

async function buildPrompt(
  task: TaskRow,
  agent: AgentDefinition,
  allowedPaths: string[],
  gh: GitHub,
  branch: string,
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

  // A new branch that continues earlier work: say so, or the agent takes the
  // files it is shown for its own and starts over.
  if (readRef !== branch && readRef.startsWith("agent/")) {
    parts.push(
      `\n# Starting point\n\nYour branch ${branch} starts from ${readRef}, an earlier task of ` +
        "this mission. The files below are that work: build on it, and fix what its " +
        "last CI run reported, rather than starting again.",
    );
  }

  // The repository as it stands on the agent's branch, plus the design
  // documents (context.ts). Before this, agents never saw a single file.
  parts.push(await repoContext(gh, agent, allowedPaths, readRef));

  // Evidence the task is about: a CI run to judge, another task to review.
  parts.push(...(await renderEvidence(task.context_refs)));

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
      "No prose, no code fence. If you are not certain of an API, a version or a " +
      "file format, return only `consult` actions first (protocol §9, rule 1).",
  );

  return parts.join("\n");
}

async function failTask(
  task: TaskRow,
  failure: string,
  detail: string,
  consumesAttempt: boolean,
): Promise<void> {
  await recordFailure(task, failure, detail, failurePatch(task, failure, detail, consumesAttempt));
}

async function recordFailure(
  task: TaskRow,
  failure: string,
  detail: string,
  patch: Record<string, unknown>,
): Promise<void> {
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

const isConsult = (a: AgentAction): a is Extract<AgentAction, { type: "consult" }> =>
  a.type === "consult";

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

/**
 * What an agent's own `failed` writes to its task: parked, not retried.
 *
 * `blocked` is outside the states the Master's duplicate guard counts as open,
 * so the task that replaces this one goes through, and the Master then closes
 * this one (master.ts, parkedSpecGaps). No attempt is consumed: the attempts
 * belong to this envelope, and the envelope is what was wrong.
 */
export function specGapPatch(detail: string): Record<string, unknown> {
  return { status: "blocked", failure: "spec_gap", failure_detail: detail.slice(0, 4000) };
}
