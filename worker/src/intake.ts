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

/** How much of the conversation the model is shown. */
const HISTORY_LIMIT = 40;

export async function runIntake(
  intake: AgentDefinition,
  router: Router,
): Promise<void> {
  // Only drafts where the human spoke last and is still waiting.
  const { data: pending, error } = await db
    .from("draft_messages")
    .select("id,mission_id")
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

  let envelope;
  let model = "";
  try {
    const result = await router.complete({
      role: intake.modelRole,
      system: intake.systemPrompt,
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

  await emit({
    missionId,
    agentId: "master",
    level: "info",
    type: "mission_launched",
    message: `Mission cadrée et lancée : ${finalize.title}`,
    payload: { criteria: finalize.acceptance_criteria, model },
  });

  log.info(`intake ${missionId.slice(0, 8)} · mission lancée : ${finalize.title}`);
}

async function markAnswered(missionId: string): Promise<void> {
  await db
    .from("draft_messages")
    .update({ answered_at: new Date().toISOString() })
    .eq("mission_id", missionId)
    .eq("role", "user")
    .is("answered_at", null);
}
