import { Router } from "@grenos/router";
import { log } from "./config.ts";
import { db, emit } from "./db.ts";
import { parseEnvelope, EnvelopeError } from "./envelope.ts";
import type { AgentDefinition } from "./prompts.ts";

/**
 * The conversation that turns an intention into a mission.
 *
 * The human writes into `draft_messages`; this answers. It runs in the worker
 * rather than in a Vercel route for the same reason everything else does: the
 * site holds no model key and executes no agent (D-001). The browser only ever
 * writes a row and watches for the reply over Realtime.
 *
 * A draft mission is invisible to the Master's normal planning cycle until
 * intake finalises it — otherwise the team would start working on a brief that
 * is still being written.
 */

interface DraftMessage {
  id: string;
  role: "user" | "master";
  content: string;
  created_at: string;
}

/** One step of docs/ROADMAP.md, as the roadmap_progress view reports it. */
export interface RoadmapStep {
  position: number;
  key: string;
  title: string;
  goal: string;
  state: string;
  done_when: unknown;
  mission_id: string | null;
}

/** How much of the conversation the model is shown. */
const HISTORY_LIMIT = 40;

export async function runIntake(
  intake: AgentDefinition,
  router: Router,
): Promise<void> {
  // Only drafts where the human spoke last and is still waiting. Messages on a
  // launched mission belong to the Master's cycle (master.ts): filtering them
  // out here keeps a backlog of those from starving a new conversation.
  const { data: pending, error } = await db
    .from("draft_messages")
    .select("id,mission_id,missions!inner(status)")
    .eq("missions.status", "draft")
    .eq("role", "user")
    .is("answered_at", null)
    .order("created_at")
    .limit(5);

  if (error) throw new Error(`intake: lecture des messages — ${error.message}`);
  if (!pending?.length) return;

  // One reply per mission, even if the human sent three messages in a row:
  // they are all part of the same turn and the model should see them together.
  const missions = [...new Set(pending.map((m) => m.mission_id))];

  for (const missionId of missions) {
    await answerOne(missionId, intake, router);
  }
}

async function answerOne(
  missionId: string,
  intake: AgentDefinition,
  router: Router,
): Promise<void> {
  const { data: mission } = await db
    .from("missions")
    .select("id,status,title")
    .eq("id", missionId)
    .maybeSingle();

  // The conversation is over once the mission left draft. Late messages are
  // marked answered so they cannot re-open a running mission.
  if (!mission || mission.status !== "draft") {
    await markAnswered(missionId);
    return;
  }

  const { data: history } = await db
    .from("draft_messages")
    .select("id,role,content,created_at")
    .eq("mission_id", missionId)
    .order("created_at")
    .limit(HISTORY_LIMIT);

  const messages = (history ?? []).map((m: DraftMessage) => ({
    role: m.role === "master" ? ("assistant" as const) : ("user" as const),
    content: m.content,
  }));

  if (messages.length === 0) return;

  // The Master scopes against the roadmap, not in a vacuum: it proposes the
  // next step, starts from that step's done_when, and links the mission to it
  // so the map on the site follows what CI has actually validated. Mission 1
  // predates this and sat on the map as "todo" while it was running.
  const steps = await readRoadmap();

  let envelope;
  let model = "";
  try {
    const result = await router.complete({
      role: intake.modelRole,
      system: [intake.systemPrompt, renderRoadmap(steps)].filter(Boolean).join("\n\n---\n\n"),
      messages,
      maxOutputTokens: 4000,
    });
    model = result.model;
    envelope = parseEnvelope(result.text);

    await db.from("draft_messages").insert({
      mission_id: missionId,
      role: "master",
      content: envelope.summary,
      model: result.model,
      tokens_in: result.tokensIn,
      tokens_out: result.tokensOut,
      answered_at: new Date().toISOString(),
    });
  } catch (err) {
    const detail = err instanceof Error ? err.message : String(err);

    // The human is sitting in front of this chat. Silence would look like the
    // system is thinking; a visible message lets them retry rather than wait.
    await db.from("draft_messages").insert({
      mission_id: missionId,
      role: "master",
      content:
        err instanceof EnvelopeError
          ? "Je n'ai pas réussi à formuler ma réponse. Reformule ta dernière phrase et je reprends."
          : `Le modèle est indisponible pour l'instant (${detail.slice(0, 120)}). Réessaie dans un instant.`,
      answered_at: new Date().toISOString(),
    });
    await markAnswered(missionId);
    log.warn(`intake ${missionId.slice(0, 8)} : ${detail.slice(0, 160)}`);
    return;
  }

  await markAnswered(missionId);

  const finalize = envelope.actions.find((a) => a.type === "finalize_mission");
  if (!finalize || finalize.type !== "finalize_mission") {
    log.info(`intake ${missionId.slice(0, 8)} · question posée · ${model}`);
    return;
  }

  // Launching: the brief written here becomes the contract the Architect
  // receives, instead of criteria it would otherwise have to invent.
  const { error } = await db
    .from("missions")
    .update({
      title: finalize.title,
      description: finalize.description,
      acceptance_criteria: finalize.acceptance_criteria,
      status: "planning",
      intake_done_at: new Date().toISOString(),
    })
    .eq("id", missionId)
    .eq("status", "draft"); // ne relance pas une mission déjà partie

  if (error) {
    log.warn(`intake ${missionId.slice(0, 8)} : lancement refusé — ${error.message}`);
    return;
  }

  if (finalize.roadmap_key) await claimStep(missionId, steps, finalize.roadmap_key);

  await emit({
    missionId,
    agentId: "master",
    level: "info",
    type: "mission_launched",
    message: `Mission cadrée et lancée : ${finalize.title}`,
    payload: {
      criteria: finalize.acceptance_criteria,
      roadmap_key: finalize.roadmap_key ?? null,
      model,
    },
  });

  log.info(`intake ${missionId.slice(0, 8)} · mission lancée : ${finalize.title}`);
}

async function readRoadmap(): Promise<RoadmapStep[]> {
  const { data, error } = await db
    .from("roadmap_progress")
    .select("position,key,title,goal,state,done_when,mission_id")
    .order("position");

  // Scoping without the roadmap still works; it only cannot link the mission.
  // Worth a warning, not worth breaking the conversation the human is in.
  if (error) {
    log.warn(`intake : feuille de route illisible — ${error.message}`);
    return [];
  }
  return (data ?? []) as RoadmapStep[];
}

async function claimStep(missionId: string, steps: RoadmapStep[], key: string): Promise<void> {
  const claim = canClaimStep(steps, key);
  const failure = claim.ok
    ? (await db.from("roadmap").update({ mission_id: missionId }).eq("key", key)).error?.message
    : claim.reason;
  if (!failure) return;

  // The mission is launched either way. A map that lags is a display problem;
  // refusing the mission over it would be a real one.
  await emit({
    missionId,
    agentId: "master",
    level: "warn",
    type: "roadmap_link_refused",
    message: `Mission lancée mais non reliée à la feuille de route : ${failure}`,
  });
}

/**
 * Whether a launched mission may claim a roadmap step.
 *
 * A step belongs to one mission at a time: taking over a step whose mission
 * is still alive, or already done, would hide that mission from the map. An
 * aborted mission frees its step for the next attempt. A key that does not
 * exist is refused, never created — the roadmap is not the model's to extend.
 */
export function canClaimStep(
  steps: RoadmapStep[],
  key: string,
): { ok: true } | { ok: false; reason: string } {
  const step = steps.find((s) => s.key === key);
  if (!step) return { ok: false, reason: `étape inconnue « ${key} »` };
  if (step.mission_id && step.state !== "aborted") {
    return { ok: false, reason: `l'étape « ${key} » appartient déjà à une mission (${step.state})` };
  }
  return { ok: true };
}

/** The roadmap as the intake Master reads it: order, state, and what CI must see. */
export function renderRoadmap(steps: RoadmapStep[]): string {
  if (steps.length === 0) return "";
  return [
    "# Roadmap — live state",
    "",
    ...steps.map(
      (s) =>
        `${s.position}. \`${s.key}\` [${s.state}] ${s.title} — ${s.goal}\n` +
        `   done when: ${JSON.stringify(s.done_when)}`,
    ),
  ].join("\n");
}

async function markAnswered(missionId: string): Promise<void> {
  await db
    .from("draft_messages")
    .update({ answered_at: new Date().toISOString() })
    .eq("mission_id", missionId)
    .eq("role", "user")
    .is("answered_at", null);
}
