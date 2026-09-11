import { Router } from "@grenos/router";
import { log } from "./config.ts";
import { db, emit } from "./db.ts";
import { parseEnvelope, EnvelopeError, type AgentEnvelope } from "./envelope.ts";
import { authorize } from "./sandbox.ts";
import { baseFor } from "./lineage.ts";
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
 *
 * And it is where the human talks to the Master while a mission runs. Their
 * messages come before everything else, the reply goes back to the mission
 * chat, and what they ask for is kept in the Master's notebook,
 * docs/MASTER.md, which it re-reads on every decision (D-022).
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

/** The Master's notebook: the human's standing instructions, kept by the Master. */
export const NOTEBOOK = "docs/MASTER.md";

const JOURNAL = "## Exchange log";

/** The heading the journal had before D-028: still read, never written again. */
const LEGACY_JOURNAL = "## Journal des échanges";

const NOTEBOOK_HEADER = [
  "# Master's notebook",
  "",
  "> Kept by the Master from what the human says in the mission chat. Re-read",
  "> on every decision and shown to every agent: what is written here binds",
  "> the whole team.",
  "",
  "## Standing instructions",
  "",
  "(none yet)",
].join("\n");

/**
 * The language of everything the human reads from the Master: chat replies,
 * summaries, escalations and their options, the notebook (D-028). English,
 * whatever language they write in: the human asked for it on 2026-09-11.
 * Named outright on every call, because "the human's language" is what
 * models ignore.
 */
export const HUMAN_LANGUAGE = "English";

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

  // No model can answer for the Master right now. Skip without recording a
  // signature: whatever happens meanwhile — results, CI verdicts, the
  // Tester's and Review's reports (autopilot.ts), the human's messages — waits
  // unseen, and the first tick after the quota returns reads all of it.
  // Recording the board here would make the Master believe it had decided.
  if (!router.available("master")) {
    await acknowledgeWhileAway(mission.id);
    return;
  }

  const state = await gatherState(mission);

  // A blocked mission is waiting for the human. Only their message reopens it.
  if (mission.status === "blocked" && state.human.length === 0) return;

  // Nothing has moved since the last decision: same tasks in the same states,
  // no new result, no new verdict, no new message. Re-reading the same board
  // would produce the same answer at the price of a request we cannot spare.
  const signature = signatureOf(mission, state);
  if (lastSeen.get(mission.id) === signature) return;

  // Work is in flight and nothing new has come back. Waiting is the correct
  // move, and it is free.
  if (
    state.tasks.length > 0 &&
    state.pending.length === 0 &&
    state.human.length === 0 &&
    state.activeCount > 0
  ) {
    lastSeen.set(mission.id, signature);
    return;
  }

  // Record before calling, not after: if the model answers badly, we must not
  // retry the identical board on the next tick and burn the quota twice.
  lastSeen.set(mission.id, signature);

  const notebook = await readNotebook(gh);

  let envelope: AgentEnvelope;
  let model = "";
  let tokensIn = 0;
  let tokensOut = 0;
  try {
    const result = await router.complete({
      role: "master",
      system: master.systemPrompt,
      messages: [{ role: "user", content: renderState(mission, state, notebook) }],
      maxOutputTokens: 8192,
      json: true,
    });
    model = result.model;
    tokensIn = result.tokensIn;
    tokensOut = result.tokensOut;

    await db.rpc("add_mission_tokens", {
      p_mission_id: mission.id,
      p_task_id: null,
      p_tokens: tokensIn + tokensOut,
    });

    envelope = parseEnvelope(result.text);
  } catch (err) {
    if (!(err instanceof EnvelopeError)) {
      // No model answered after all: a quota learned mid-call, an outage.
      // Forget the board, so the next tick with a model available decides on
      // it — otherwise the mission would wait for an unrelated change.
      lastSeen.delete(mission.id);
    } else if (state.human.length > 0) {
      // The Master answered, but not in the protocol. The human must not be
      // left in front of "thinking…" for a reply that will never come.
      await replyToHuman(
        mission.id,
        state.human,
        "I could not put my decision into words. Send your message again, or rephrase it, and I will pick it up.",
        model,
        tokensIn,
        tokensOut,
      );
    }
    throw err;
  }

  await db.from("messages").insert({
    mission_id: mission.id,
    from_agent: "master",
    to_agent: "human",
    kind: "decision",
    content: envelope as unknown as Record<string, unknown>,
    model,
    tokens_in: tokensIn,
    tokens_out: tokensOut,
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

  // Documents the Master writes in this decision, committed together at the
  // end: one commit per decision, not one per file.
  const docs = new Map<string, string>();

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

        // A writer's task continues the mission's work instead of starting
        // from a main that holds none of it (lineage.ts).
        const base = await baseFor(mission.id, assignee.id, action.continue_from);

        const { error } = await db.from("tasks").insert({
          mission_id: mission.id,
          assigned_to: assignee.id,
          goal: action.goal,
          acceptance_criteria: action.acceptance_criteria,
          allowed_paths: action.allowed_paths ?? assignee.allowedPaths,
          context_refs: base ? [`base:${base}`] : [],
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
        // The Master keeps docs/STATE.md truthful and docs/MASTER.md current.
        // Its sandbox allows nothing else, and these go straight to the default
        // branch: they are a journal, not code, so CI has nothing to verify.
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
        docs.set(verdict.path, action.content);
        break;
      }

      default:
        // request_build, request_test and request_help are not the Master's to
        // perform. It routes work; it does not do it.
        break;
    }
  }

  // The human spoke: answer them in the chat, and write the exchange into the
  // notebook. The journal is appended here, by code, so no exchange can be
  // lost to a model that forgot to write it down; the Master curates the
  // standing instructions above it.
  if (state.human.length > 0) {
    await replyToHuman(mission.id, state.human, envelope.summary, model, tokensIn, tokensOut);
    const previous = notebook ?? "";
    const next = docs.get(NOTEBOOK) ?? previous;
    docs.set(NOTEBOOK, withJournal(next, previous, journalEntry(state.human, envelope.summary)));
  }

  if (docs.size > 0) {
    await gh.commit({
      branch: await gh.defaultBranch(),
      message: `master: update ${[...docs.keys()].join(", ")}`,
      // The repository is public and the notebook quotes the human: a key
      // pasted into the chat would otherwise be published within the minute.
      changes: [...docs].map(([path, content]) => ({ path, content: redactSecrets(content) })),
    });
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

  // Tasks waiting for exactly this decision: parked on spec_gap (executor.ts,
  // specGapPatch), or failed after their last attempt. New work in the same
  // decision is the answer to them, so they are closed rather than left open
  // for ever — a failed task still on the board has attempts_left 0, and the
  // Master escalated it again on every cycle, even after the human answered.
  const superseded = created > 0 ? supersededByNewWork(state) : [];
  if (superseded.length > 0) {
    await db
      .from("tasks")
      .update({ status: "cancelled", finished_at: new Date().toISOString() })
      .in("id", superseded)
      .in("status", ["blocked", "failed"]);
    await emit({
      missionId: mission.id,
      agentId: "master",
      level: "info",
      type: "task_superseded",
      message: `${superseded.length} tâche(s) remplacée(s) par la nouvelle décision du Maître (spec_gap ou tentatives épuisées).`,
      payload: { tasks: superseded },
    });
  }

  if (!escalated && mission.status === "blocked" && state.human.length > 0) {
    // An escalation is a question to the human. They answered, and the Master
    // did not escalate again: that answer was what the mission waited for.
    await db.from("missions").update({ status: "running" }).eq("id", mission.id);
    await emit({
      missionId: mission.id,
      agentId: "master",
      level: "info",
      type: "mission_resumed",
      message: "Mission reprise après la réponse de l'humain.",
    });
  } else if (!escalated && created > 0 && mission.status !== "running") {
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

  log.info(
    `master · ${mission.title.slice(0, 40)} · +${created} tâche(s)` +
      (state.human.length > 0 ? " · a répondu à l'humain" : ""),
  );
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
  /** The human's messages the Master has not answered yet. */
  human: Array<{ id: string; content: string; created_at: string }>;
  /** The last few exchanges, oldest first, for context. */
  conversation: Array<{ role: string; content: string; created_at: string }>;
}

async function gatherState(mission: Mission): Promise<State> {
  const [{ data: tasks }, pendingRes, { data: runs }, humanRes, { data: recent }] =
    await Promise.all([
      db
        .from("tasks")
        .select("id,assigned_to,goal,status,attempt,max_attempts,failure,failure_detail")
        .eq("mission_id", mission.id)
        .order("created_at"),
      // Up to 30: when the Master comes back from a quota outage, the Tester's
      // and Review's reports have piled up, and it should read them together.
      db
        .from("messages")
        .select("id,from_agent,content")
        .eq("mission_id", mission.id)
        .in("kind", ["proposal", "result"])
        .is("seen_at", null)
        .order("created_at", { ascending: false })
        .limit(30),
      db
        .from("runs")
        .select("branch,status,failure,verdicts,log_excerpt")
        .eq("mission_id", mission.id)
        .order("started_at", { ascending: false })
        .limit(8),
      db
        .from("draft_messages")
        .select("id,content,created_at")
        .eq("mission_id", mission.id)
        .eq("role", "user")
        .is("answered_at", null)
        .order("created_at"),
      db
        .from("draft_messages")
        .select("role,content,created_at")
        .eq("mission_id", mission.id)
        .order("created_at", { ascending: false })
        .limit(12),
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
  if (humanRes.error) {
    throw new Error(`Impossible de lire les messages de l'humain : ${humanRes.error.message}`);
  }

  const rows = tasks ?? [];
  return {
    tasks: rows,
    activeCount: rows.filter((t) => ACTIVE_TASK_STATES.includes(t.status)).length,
    pending: pendingRes.data ?? [],
    runs: runs ?? [],
    human: humanRes.data ?? [],
    conversation: (recent ?? []).reverse(),
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
  const human = state.human.map((h) => h.id).sort().join(",");
  return `${mission.status}|${tasks}|${pending}|${runs}|${human}`;
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
 * Everything new work answers: tasks parked on spec_gap, and tasks that
 * failed their last attempt. A task blocked on a question is not among them.
 */
export function supersededByNewWork(state: State): string[] {
  return [
    ...parkedSpecGaps(state),
    ...state.tasks.filter((t) => t.status === "failed").map((t) => t.id),
  ];
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

/**
 * How many attempts a task has actually spent. `attempt` is the number of the
 * attempt queued or running, not a count: at 3 of 3 and `ready`, the third
 * attempt has not run yet. The Master read "3/3" as exhausted and escalated a
 * task whose last try was still ahead of it.
 */
export function attemptsUsed(t: { status: string; attempt: number }): number {
  return t.status === "ready" || t.status === "pending" ? t.attempt - 1 : t.attempt;
}

/**
 * What the Master needs from a worker's envelope: what it did, not every byte
 * it wrote. A single kernel file quoted in full crowded out the decision
 * itself, and after a quota outage there can be thirty of them waiting.
 */
export function digestOfMessage(content: unknown): unknown {
  if (!isRecord(content) || !Array.isArray(content["actions"])) return content;
  return {
    ...content,
    actions: content["actions"].map((a: unknown) => {
      if (!isRecord(a)) return a;
      const out: Record<string, unknown> = { ...a };
      for (const key of ["content", "old_str", "new_str"]) {
        const v = out[key];
        if (typeof v === "string" && v.length > 400) {
          out[key] = `${v.slice(0, 200)}… (${v.length} characters)`;
        }
      }
      return out;
    }),
  };
}

/**
 * The notebook after an exchange: the standing instructions from the Master's
 * latest version (or the previous one, when it did not rewrite them), then the
 * journal — kept by code, so an exchange is never lost to a model that
 * rewrote the file and dropped the history.
 */
export function withJournal(next: string, previous: string, entry: string, keep = 40): string {
  const body = (splitNotebook(next).body || splitNotebook(previous).body || NOTEBOOK_HEADER).trimEnd();
  const entries = [...splitNotebook(previous).entries, entry].slice(-keep);
  return `${body}\n\n${JOURNAL}\n\n${entries.join("\n")}\n`;
}

function splitNotebook(text: string): { body: string; entries: string[] } {
  // The first journal heading, today's or the one from before D-028.
  const found = [JOURNAL, LEGACY_JOURNAL]
    .map((heading) => ({ heading, at: text.indexOf(heading) }))
    .filter((h) => h.at !== -1)
    .sort((a, b) => a.at - b.at)[0];
  if (!found) return { body: text.trim(), entries: [] };
  return {
    body: text.slice(0, found.at).trim(),
    entries: text
      .slice(found.at + found.heading.length)
      .split("\n")
      .filter((line) => line.startsWith("- ")),
  };
}

/** One exchange, as one line of the journal. */
export function journalEntry(
  human: Array<{ content: string; created_at: string }>,
  reply: string,
): string {
  const flat = (s: string, max: number) => {
    const one = s.replace(/\s+/g, " ").trim();
    return one.length > max ? `${one.slice(0, max - 1)}…` : one;
  };
  const at = (human[human.length - 1]?.created_at ?? new Date().toISOString())
    .slice(0, 16)
    .replace("T", " ");
  const said = human.map((h) => flat(h.content, 400)).join(" / ");
  return `- ${at} UTC · Human: "${flat(said, 600)}" → Master: "${flat(reply, 400)}"`;
}

// Supabase JWTs and secret keys, Google, NVIDIA, GitHub and OpenAI-style keys.
const SECRET =
  /(eyJ[\w-]{10,}\.[\w-]{10,}\.[\w-]{10,}|sb_secret_[\w-]{16,}|AIza[\w-]{30,}|nvapi-[\w-]{20,}|gh[pousr]_[A-Za-z0-9]{30,}|github_pat_\w{30,}|sk-[\w-]{20,})/g;

/** Masks anything shaped like a credential before it reaches the public repo. */
export function redactSecrets(text: string): string {
  return text.replace(SECRET, "[redacted secret]");
}

/**
 * What the Master reads on every decision. Exported for the tests: the
 * language it answers in and the time it is given are rules, not details.
 */
export function renderState(
  mission: Mission,
  state: State,
  notebook: string | null,
  now: Date = new Date(),
): string {
  const parts: string[] = [];
  // Everything the human reads is in English, whether or not they wrote this
  // cycle: replies, summaries, escalations and their options (D-028).
  const language = HUMAN_LANGUAGE;

  // The human first: their message preempts everything (decision step 1).
  if (state.human.length > 0) {
    parts.push("# The human is waiting for your answer\n");
    for (const h of state.human) parts.push(`> ${h.content.replace(/\n/g, "\n> ")}\n`);
    parts.push(
      "Handle this before anything else. Your `summary` is your reply, shown " +
        `verbatim in the mission chat: write it in ${language}, even when they write in another ` +
        "language. If they gave an instruction, a preference or a decision that should last, " +
        "rewrite docs/MASTER.md with write_file — the journal is appended for you.\n",
    );
  }

  // Without it the Master dated docs/STATE.md "At 2026-09-10 [current time]".
  parts.push("# Now\n");
  parts.push(
    `${now.toISOString().slice(0, 16).replace("T", " ")} UTC. Date what you write in ` +
      "docs/STATE.md with it; never write a placeholder.\n",
  );

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

  parts.push("\n# Your notebook (docs/MASTER.md) — the human's standing instructions\n");
  parts.push(renderNotebook(notebook));

  if (state.conversation.length > 0) {
    parts.push("\n# Recent conversation with the human (oldest first)\n");
    for (const m of state.conversation) {
      parts.push(`- ${m.role === "user" ? "human" : "you"}: ${m.content.replace(/\s+/g, " ").slice(0, 600)}`);
    }
  }

  parts.push("\n# Tasks\n");
  parts.push(
    state.tasks.length === 0
      ? "(none yet — this mission has no plan)"
      : JSON.stringify(
          state.tasks.map((t) => ({
            ...t,
            attempts_used: attemptsUsed(t),
            attempts_left: t.max_attempts - attemptsUsed(t),
          })),
          null,
          2,
        ),
  );

  if (state.runs.length > 0) {
    parts.push("\n# CI verdicts (the only source of truth)\n");
    parts.push(JSON.stringify(state.runs.map(ciDigest), null, 2));
  }

  if (state.pending.length > 0) {
    parts.push("\n# Awaiting your judgement\n");
    parts.push(
      JSON.stringify(
        state.pending.map((p) => ({ ...p, content: digestOfMessage(p.content) })),
        null,
        2,
      ),
    );
  }

  parts.push(
    "\n# Your turn\n\nRun your decision procedure and return exactly one JSON " +
      "object. Use propose_task to dispatch work, escalate when blocked. " +
      "If there is nothing to do, return status \"done\" with an empty actions array.\n\n" +
      `Language: write \`summary\`, and the \`reason\` and \`options\` of any escalation, in ${language} — ` +
      "the human reads them, and asked for English whatever language they write in. Task goals " +
      "and acceptance criteria are in English too: agents read those.",
  );

  return parts.join("\n");
}

/**
 * A CI run as the Master needs it: the verdict, and the lines that explain it.
 *
 * The full log tail is 6,000 characters per run and the Master was shown
 * eight: on mission 1 it spent two thirds of the tokens, more than the Coder
 * who writes the code. Judging and routing needs the error, not the build
 * noise. The Coder still receives the whole log with its retry.
 */
export function ciDigest(run: State["runs"][number]): Record<string, unknown> {
  const errors = (run.log_excerpt ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter((line) =>
      /\b(error|warning|panic|fault|failed|Finished)\b|^-->|Marqueur|Aucune image/i.test(line),
    )
    .slice(0, 12)
    .join("\n")
    .slice(0, 1500);
  return {
    branch: run.branch,
    status: run.status,
    failure: run.failure,
    verdicts: run.verdicts,
    errors: errors || null,
  };
}

/** The standing instructions in full, the journal only in its latest lines. */
function renderNotebook(notebook: string | null): string {
  if (!notebook?.trim()) {
    return "(empty — create it as soon as the human gives you an instruction)";
  }
  const { body, entries } = splitNotebook(notebook);
  return entries.length === 0
    ? body
    : `${body}\n\n${JOURNAL} (latest)\n\n${entries.slice(-10).join("\n")}`;
}

async function readNotebook(gh: GitHub): Promise<string | null> {
  try {
    return await gh.readFile(NOTEBOOK, await gh.defaultBranch());
  } catch (err) {
    // Deciding without the notebook is worse than deciding with it, but far
    // better than not deciding: the human is waiting either way.
    log.warn(`carnet du Maître illisible : ${err instanceof Error ? err.message : err}`);
    return null;
  }
}

async function replyToHuman(
  missionId: string,
  human: State["human"],
  text: string,
  model: string,
  tokensIn: number,
  tokensOut: number,
): Promise<void> {
  const now = new Date().toISOString();
  await db.from("draft_messages").insert({
    mission_id: missionId,
    role: "master",
    content: text,
    model: model || null,
    tokens_in: tokensIn,
    tokens_out: tokensOut,
    answered_at: now,
  });
  await db
    .from("draft_messages")
    .update({ answered_at: now })
    .in("id", human.map((h) => h.id));
}

/**
 * The human wrote while the Master has no quota. Say so once, rather than
 * leave them in front of "thinking…" for hours. Their message stays
 * unanswered on purpose: the Master still reads it, and replies, when it can.
 */
async function acknowledgeWhileAway(missionId: string): Promise<void> {
  const { data: last } = await db
    .from("draft_messages")
    .select("role,answered_at")
    .eq("mission_id", missionId)
    .order("created_at", { ascending: false })
    .limit(1)
    .maybeSingle();
  if (!last || last.role !== "user" || last.answered_at) return;

  await db.from("draft_messages").insert({
    mission_id: missionId,
    role: "master",
    content:
      "I am out of quota for now. Your message is kept: I will read it and answer as soon " +
      "as I have quota again. Meanwhile the Coder keeps working, and the Tester and Review " +
      "record what they find for me.",
    model: "system",
    answered_at: new Date().toISOString(),
  });
}

const isRecord = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);
