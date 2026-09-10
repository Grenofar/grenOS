import { Router } from "@grenos/router";
import { log } from "./config.ts";
import { db, emit } from "./db.ts";
import { parseEnvelope } from "./envelope.ts";
import { authorize } from "./sandbox.ts";
import type { GitHub } from "./github.ts";
import type { AgentDefinition } from "./prompts.ts";

/**
 * The Master's decision cycle.
 *
 * The Master is not a queued task: it is the loop's own judgement step, run
 * once per mission per tick. Making it a task would mean the queue needs
 * something to decide what goes in the queue, which is circular.
 *
 * It is also the only place where a proposal becomes real work. Workers emit
 * `propose_task`; those sit as messages until the Master decides they are worth
 * doing. That is what keeps nine agents from generating work for each other
 * faster than anyone can spend the quota on it (D-008).
 */

export interface Mission {
  id: string;
  title: string;
  description: string;
  status: string;
  token_budget: number;
  tokens_used: number;
}

const ACTIVE_TASK_STATES = ["ready", "in_progress", "awaiting_verification"];

/**
 * Signature of what the Master last reasoned about, per mission.
 *
 * The loop wakes on every Realtime event and every poll — several times a
 * minute. Thinking on each wake-up would spend the entire daily quota in
 * minutes: the free tier allows about 20 requests per model per day, not the
 * 1500 the published figures claimed. So the Master only reasons when the
 * situation has actually changed, and an unchanged situation costs nothing.
 *
 * In-memory on purpose. A restart re-thinks once per mission, which is the
 * safe direction to be wrong in.
 */
const lastSeen = new Map<string, string>();

export async function runMasterCycle(
  mission: Mission,
  master: AgentDefinition,
  agents: Map<string, AgentDefinition>,
  router: Router,
  gh: GitHub,
): Promise<void> {
  // Budget first, before anything is dispatched. A mission that is already
  // over budget must not start new work while we deliberate about it
  // (agents/README.md §7, step 2).
  if (mission.tokens_used >= mission.token_budget) {
    await db.from("missions").update({ status: "aborted" }).eq("id", mission.id);
    await emit({
      missionId: mission.id,
      agentId: "master",
      level: "warn",
      type: "budget_exhausted",
      message: `Budget épuisé : ${mission.tokens_used}/${mission.token_budget} tokens. Mission arrêtée.`,
    });
    return;
  }

  const state = await gatherState(mission);

  // Nothing has moved since the last decision: same tasks in the same states,
  // no new result, no new verdict. Re-reading the same board would produce the
  // same answer at the price of a request we cannot spare.
  const signature = signatureOf(mission, state);
  if (lastSeen.get(mission.id) === signature) return;

  // Work is in flight and nothing new has come back. Waiting is the correct
  // move, and it is free.
  if (state.tasks.length > 0 && state.pending.length === 0 && state.activeCount > 0) {
    lastSeen.set(mission.id, signature);
    return;
  }

  // Record before calling, not after: if the model call throws, we must not
  // retry the identical board on the next tick and burn the quota twice.
  lastSeen.set(mission.id, signature);

  const result = await router.complete({
    role: "master",
    system: master.systemPrompt,
    messages: [{ role: "user", content: renderState(mission, state) }],
    maxOutputTokens: 8192,
    json: true,
  });

  await db.rpc("add_mission_tokens", {
    p_mission_id: mission.id,
    p_task_id: null,
    p_tokens: result.tokensIn + result.tokensOut,
  });

  const envelope = parseEnvelope(result.text);

  await db.from("messages").insert({
    mission_id: mission.id,
    from_agent: "master",
    to_agent: "human",
    kind: "decision",
    content: envelope as unknown as Record<string, unknown>,
    model: result.model,
    tokens_in: result.tokensIn,
    tokens_out: result.tokensOut,
  });

  let created = 0;
  let escalated = false;

  // Which agents already have work in flight for this mission.
  //
  // The Master is asked to decide again on every state change, and its
  // in-memory "already decided this" marker does not survive a restart. Both
  // of those are fine on their own; together they let it dispatch the same
  // task two or three times, each on its own branch, each burning a full
  // model call and a CI run. Whether it is a prompt lapse or a restart, the
  // outcome must be impossible rather than unlikely.
  const OPEN = ["pending", "ready", "in_progress", "awaiting_verification"];
  const { data: openTasks } = await db
    .from("tasks")
    .select("assigned_to")
    .eq("mission_id", mission.id)
    .in("status", OPEN);
  const busy = new Set((openTasks ?? []).map((t) => t.assigned_to));

  for (const action of envelope.actions) {
    switch (action.type) {
      case "propose_task": {
        if (busy.has(action.assigned_to)) {
          await emit({
            missionId: mission.id,
            agentId: "master",
            level: "warn",
            type: "duplicate_task_refused",
            message:
              `Tâche refusée : ${action.assigned_to} a déjà une tâche ouverte sur cette mission. ` +
              `Attends son résultat plutôt que d'en ouvrir une seconde.`,
            payload: { goal: action.goal },
          });
          break;
        }

        const assignee = agents.get(action.assigned_to);
        if (!assignee || assignee.status !== "active") {
          await emit({
            missionId: mission.id,
            agentId: "master",
            level: "warn",
            type: "routing_error",
            message: `Tâche adressée à "${action.assigned_to}", agent inconnu ou dormant. Ignorée.`,
          });
          break;
        }

        const { error } = await db.from("tasks").insert({
          mission_id: mission.id,
          assigned_to: assignee.id,
          goal: action.goal,
          acceptance_criteria: action.acceptance_criteria,
          allowed_paths: action.allowed_paths ?? assignee.allowedPaths,
          status: "ready",
          max_attempts: assignee.maxAttempts,
          token_budget: assignee.maxTokensPerTask,
        });

        if (error) {
          // The CHECK constraint on acceptance_criteria fires here when the
          // Master writes an unverifiable task. Surfacing it is better than
          // silently dropping the work.
          await emit({
            missionId: mission.id,
            agentId: "master",
            level: "warn",
            type: "task_rejected",
            message: `Tâche refusée par la base : ${error.message}`,
            payload: { goal: action.goal },
          });
        } else {
          created += 1;
          // Guard the rest of this same envelope too: a single reply can
          // legitimately propose two tasks for the same agent.
          busy.add(assignee.id);
        }
        break;
      }

      case "escalate": {
        // One escalation per decision. A reply stating the same blocker three
        // ways produced three identical alerts; the human needs to read one,
        // with its options, not a burst.
        if (escalated) break;
        escalated = true;
        await db.from("missions").update({ status: "blocked" }).eq("id", mission.id);
        await emit({
          missionId: mission.id,
          agentId: "master",
          level: "error",
          type: "escalation",
          message: action.reason,
          payload: { options: action.options ?? [] },
        });
        break;
      }

      case "write_file": {
        // The Master keeps docs/STATE.md truthful. Its sandbox allows nothing
        // else, and this goes straight to the default branch: it is a journal,
        // not code, so there is nothing for CI to verify.
        const verdict = authorize(action.path, {
          allowedPaths: master.allowedPaths,
          forbiddenPaths: master.forbiddenPaths,
          canWrite: master.canWrite,
        });
        if (!verdict.ok) {
          await emit({
            missionId: mission.id,
            agentId: "master",
            level: "error",
            type: "policy_violation",
            message: verdict.reason,
          });
          break;
        }
        await gh.commit({
          branch: await gh.defaultBranch(),
          message: `master: update ${verdict.path}`,
          changes: [{ path: verdict.path, content: action.content }],
        });
        break;
      }

      default:
        // request_build, request_test and request_help are not the Master's to
        // perform. It routes work; it does not do it.
        break;
    }
  }

  // Mark what we just judged, so the next cycle does not re-read it and
  // propose the same work again. Stamping `seen_at` rather than rewriting
  // `kind` keeps the message's own meaning intact in the journal.
  if (state.pending.length > 0) {
    await db
      .from("messages")
      .update({ seen_at: new Date().toISOString() })
      .in("id", state.pending.map((p) => p.id));
  }

  // A task parked on spec_gap waits for exactly this: the Master's answer
  // (executor.ts, specGapPatch). New work in the same decision is that answer,
  // so the parked task is closed rather than left blocked for ever, where
  // nothing would pick it up and the mission could never complete.
  const parked = created > 0 ? parkedSpecGaps(state) : [];
  if (parked.length > 0) {
    await db
      .from("tasks")
      .update({ status: "cancelled", finished_at: new Date().toISOString() })
      .in("id", parked)
      .eq("status", "blocked");
    await emit({
      missionId: mission.id,
      agentId: "master",
      level: "info",
      type: "task_superseded",
      message: `${parked.length} tâche(s) en spec_gap remplacée(s) par la nouvelle décision du Maître.`,
      payload: { tasks: parked },
    });
  }

  if (!escalated && created > 0 && mission.status !== "running") {
    await db.from("missions").update({ status: "running" }).eq("id", mission.id);
  }

  if (!escalated && created === 0 && isMissionComplete(state)) {
    await db
      .from("missions")
      .update({ status: "done", finished_at: new Date().toISOString() })
      .eq("id", mission.id);
    await emit({
      missionId: mission.id,
      agentId: "master",
      level: "info",
      type: "mission_done",
      message: envelope.summary,
    });
  }

  log.info(`master · ${mission.title.slice(0, 40)} · +${created} tâche(s)`);
}

export interface State {
  tasks: Array<{
    id: string;
    assigned_to: string;
    goal: string;
    status: string;
    attempt: number;
    max_attempts: number;
    failure: string | null;
    failure_detail: string | null;
  }>;
  activeCount: number;
  pending: Array<{ id: string; from_agent: string; content: unknown }>;
  runs: Array<{
    branch: string;
    status: string;
    failure: string | null;
    verdicts: unknown;
    log_excerpt: string | null;
  }>;
}

async function gatherState(mission: Mission): Promise<State> {
  const [{ data: tasks }, pendingRes, { data: runs }] = await Promise.all([
    db
      .from("tasks")
      .select("id,assigned_to,goal,status,attempt,max_attempts,failure,failure_detail")
      .eq("mission_id", mission.id)
      .order("created_at"),
    db
      .from("messages")
      .select("id,from_agent,content")
      .eq("mission_id", mission.id)
      .in("kind", ["proposal", "result"])
      .is("seen_at", null)
      .order("created_at", { ascending: false })
      .limit(10),
    db
      .from("runs")
      .select("branch,status,failure,verdicts,log_excerpt")
      .eq("mission_id", mission.id)
      .order("started_at", { ascending: false })
      .limit(5),
  ]);

  // A failure here used to degrade to an empty list, which looks exactly like
  // "nothing new happened" — so the Master would keep planning while never
  // reading a single thing its agents sent back. Silence is the one outcome
  // this query must never produce.
  if (pendingRes.error) {
    throw new Error(
      `Impossible de lire les messages en attente : ${pendingRes.error.message}. ` +
        `Si la colonne seen_at manque, passe packages/db/migrations/0006_repair.sql.`,
    );
  }

  const rows = tasks ?? [];
  return {
    tasks: rows,
    activeCount: rows.filter((t) => ACTIVE_TASK_STATES.includes(t.status)).length,
    pending: pendingRes.data ?? [],
    runs: runs ?? [],
  };
}

/**
 * A compact fingerprint of everything the Master's decision depends on. Two
 * identical signatures mean an identical decision, so the second call can be
 * skipped entirely.
 */
export function signatureOf(mission: Mission, state: State): string {
  const tasks = state.tasks
    .map((t) => `${t.id.slice(0, 8)}:${t.status}:${t.attempt}`)
    .sort()
    .join(",");
  const pending = state.pending.map((p) => p.id).sort().join(",");
  const runs = state.runs.map((r) => `${r.branch}:${r.status}`).join(",");
  return `${mission.status}|${tasks}|${pending}|${runs}`;
}

/**
 * Tasks parked on spec_gap, waiting for the Master to answer them.
 *
 * Only those. A task blocked on a question is resumed by its answer, so
 * discarding it would throw away the work that asked.
 */
export function parkedSpecGaps(state: State): string[] {
  return state.tasks
    .filter((t) => t.status === "blocked" && t.failure === "spec_gap")
    .map((t) => t.id);
}

/**
 * Every task that still counts is done. Cancelled tasks were superseded and do
 * not count: mission 1 carries four, and requiring those to be done too meant
 * it could never close on its own. A failed or blocked task still holds the
 * mission open — that one needs a decision, not a shrug.
 */
export function isMissionComplete(state: State): boolean {
  const counted = state.tasks.filter((t) => t.status !== "cancelled");
  return (
    state.activeCount === 0 && counted.length > 0 && counted.every((t) => t.status === "done")
  );
}

function renderState(mission: Mission, state: State): string {
  const parts: string[] = [];

  parts.push("# Mission\n");
  parts.push(
    JSON.stringify(
      {
        title: mission.title,
        description: mission.description,
        status: mission.status,
        tokens_used: mission.tokens_used,
        token_budget: mission.token_budget,
      },
      null,
      2,
    ),
  );

  parts.push("\n# Tasks\n");
  parts.push(
    state.tasks.length === 0
      ? "(none yet — this mission has no plan)"
      : JSON.stringify(state.tasks, null, 2),
  );

  if (state.runs.length > 0) {
    parts.push("\n# CI verdicts (the only source of truth)\n");
    parts.push(JSON.stringify(state.runs, null, 2));
  }

  if (state.pending.length > 0) {
    parts.push("\n# Awaiting your judgement\n");
    parts.push(JSON.stringify(state.pending, null, 2));
  }

  parts.push(
    "\n# Your turn\n\nRun your decision procedure and return exactly one JSON " +
      "object. Use propose_task to dispatch work, escalate when blocked. " +
      "If there is nothing to do, return status \"done\" with an empty actions array.",
  );

  return parts.join("\n");
}
