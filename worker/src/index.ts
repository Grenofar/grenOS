import { Router } from "@grenos/router";
import { config, log } from "./config.ts";
import { agentsPaused, db, emit, syncAgents, SupabaseUsageStore } from "./db.ts";
import { executeTask, type TaskRow } from "./executor.ts";
import { GitHub } from "./github.ts";
import { runIntake } from "./intake.ts";
import { runMasterCycle } from "./master.ts";
import { loadAgents, type AgentDefinition } from "./prompts.ts";
import { pathsOverlap } from "./sandbox.ts";

/**
 * The brain.
 *
 * One process, one loop, no framework. Every piece of durable state lives in
 * Supabase, so this process owns nothing: kill it mid-task and the worst that
 * happens is a lease expires and the task is picked up again. That property is
 * what makes it safe to run on a 256 MB free tier that can restart at any time
 * (D-002).
 *
 * Realtime wakes the loop when something changes; the poll interval is only a
 * safety net for a dropped subscription.
 */

// `draft` est volontairement absent : une mission en brouillon est encore en
// train d'être cadrée par conversation (intake.ts). La planifier pendant que
// l'humain écrit encore reviendrait à faire travailler l'équipe sur un énoncé
// qui n'est pas fini.
const MISSION_STATES = ["planning", "running"];

let running = true;
let ticking = false;
let wakeUp: (() => void) | null = null;

async function main(): Promise<void> {
  log.info("grenOS worker — démarrage");

  const definitions = loadAgents();
  await syncAgents(definitions);

  const agents = new Map(definitions.map((a) => [a.id, a]));
  const master = agents.get("master");
  if (!master) throw new Error("Agent 'master' introuvable dans agents/");
  const intake = agents.get("intake");
  if (!intake) throw new Error("Agent 'intake' introuvable dans agents/");

  const usage = new SupabaseUsageStore();
  const router = new Router({
    geminiApiKey: config.geminiApiKey,
    nvidiaApiKey: config.nvidiaApiKey,
    usage,
    onEvent: (e) => {
      if (e.outcome !== "ok") {
        log.debug(`router: ${e.model} → ${e.outcome}${e.detail ? ` (${e.detail})` : ""}`);
      }
    },
  });

  const gh = new GitHub();
  log.info(`dépôt ${gh.repo} · ${definitions.filter((a) => a.status === "active").length} agents actifs`);
  log.info(
    config.nvidiaApiKey
      ? "providers : NVIDIA NIM (40 req/min) + Gemini en repli"
      : "providers : Gemini seul — ~20 req/jour et par modèle, ajoute NVIDIA_API_KEY",
  );

  subscribe();
  installShutdown();

  while (running) {
    try {
      await tick(agents, master, intake, router, gh);
    } catch (err) {
      // The loop must survive anything. A crash here on a free tier means the
      // process may not come back for a long time.
      log.warn("tick a échoué:", err instanceof Error ? err.message : err);
      await emit({
        level: "error",
        type: "worker_error",
        message: err instanceof Error ? err.message : String(err),
      });
    }
    await waitForWork();
  }

  log.info("worker arrêté");
}

async function tick(
  agents: Map<string, AgentDefinition>,
  master: AgentDefinition,
  intake: AgentDefinition,
  router: Router,
  gh: GitHub,
): Promise<void> {
  if (ticking) return;
  ticking = true;
  try {
    // Deux crans indépendants : AGENTS_PAUSED est local et survit à une base
    // injoignable ; settings.agents_paused est le bouton de l'interface.
    if (config.pausedLocally) {
      log.debug("agents en pause (AGENTS_PAUSED=true dans .env.local)");
      return;
    }
    if (await agentsPaused()) {
      log.debug("agents en pause (coupe-circuit de l'interface)");
      return;
    }

    // Le cadrage d'abord : une mission lancée par la conversation doit être
    // planifiée dans le même tick, pas au suivant. L'humain vient d'appuyer.
    await runIntake(intake, router);

    const { data: missions } = await db
      .from("missions")
      .select("id,title,description,status,token_budget,tokens_used")
      .in("status", MISSION_STATES)
      .order("created_at");

    for (const mission of missions ?? []) {
      await runMasterCycle(mission, master, agents, router, gh);
    }

    await warnAboutStuckVerifications();
    await dispatchWorkers(agents, router, gh);
  } finally {
    ticking = false;
  }
}

/**
 * Claim and run ready tasks, up to the concurrency limit.
 *
 * Two tasks whose paths can overlap are never started together. The lease table
 * would reject the second one anyway, but a rejected lease costs a full model
 * call — the tokens are spent before the write is attempted. Checking here
 * makes that waste avoidable rather than merely survivable.
 */
async function dispatchWorkers(
  agents: Map<string, AgentDefinition>,
  router: Router,
  gh: GitHub,
): Promise<void> {
  // Only claim work some model can actually answer. When a role's whole
  // cascade is cooling down, claiming its tasks would fail them instantly,
  // requeue them and repeat every tick — noise about an outage the router
  // already knows about, and churn on rows the dashboard is watching.
  const workerIds = [...agents.values()]
    .filter((a) => a.status === "active" && a.id !== "master" && a.id !== "intake")
    .filter((a) => router.available(a.modelRole))
    .map((a) => a.id);
  if (workerIds.length === 0) return;

  const { count } = await db
    .from("tasks")
    .select("id", { count: "exact", head: true })
    .eq("status", "in_progress");

  let slots = config.maxConcurrentTasks - (count ?? 0);
  if (slots <= 0) return;

  const inFlightPaths: string[][] = [];
  const inFlight: Promise<void>[] = [];

  while (slots > 0) {
    const { data: task, error } = await db.rpc("claim_next_task", {
      p_agent_ids: workerIds,
    });
    if (error) throw new Error(`claim_next_task: ${error.message}`);

    // A plpgsql function returning a composite type can hand back a row of all
    // nulls rather than null itself when nothing matched, so an id check is
    // the reliable "queue is empty" signal.
    const row = task as TaskRow | null;
    if (!row?.id) break;
    const agent = agents.get(row.assigned_to);
    if (!agent) {
      await db
        .from("tasks")
        .update({ status: "failed", failure: "capability_gap", failure_detail: "Agent inconnu" })
        .eq("id", row.id);
      continue;
    }

    const paths = row.allowed_paths.length > 0 ? row.allowed_paths : agent.allowedPaths;

    if (inFlightPaths.some((other) => pathsOverlap(other, paths))) {
      // Put it back untouched; a later tick will take it once the conflicting
      // task is done. No attempt consumed, no tokens spent.
      await db.from("tasks").update({ status: "ready", started_at: null }).eq("id", row.id);
      break;
    }

    inFlightPaths.push(paths);
    inFlight.push(
      executeTask(row, agent, router, gh).catch(async (err) => {
        const detail = err instanceof Error ? err.message : String(err);
        log.warn(`tâche ${row.id.slice(0, 8)} a levé:`, detail);
        // Something outside the model broke — GitHub, a lease RPC, the
        // database. Not the agent's failure, so failure_detail (which the
        // agent reads on its next attempt) is left untouched. Not retried
        // blindly either: a deterministic error would loop, paying a model
        // call each time. Blocked and reported; the Master decides.
        await db.from("tasks").update({ status: "blocked" }).eq("id", row.id);
        await db.rpc("release_leases", { p_task_id: row.id });
        await emit({
          missionId: row.mission_id,
          taskId: row.id,
          agentId: row.assigned_to,
          level: "error",
          type: "infrastructure_error",
          message: detail.slice(0, 500),
        });
      }),
    );
    slots -= 1;
  }

  if (inFlight.length > 0) await Promise.all(inFlight);
}

/** Tasks already reported, so the warning is emitted once and not every tick. */
const warnedStuck = new Set<string>();

/**
 * A verdict that never arrives is the quietest failure this system has.
 *
 * `awaiting_verification` is a correct, expected state — nothing is done until
 * CI says so (D-009). But if the verdict never comes, the task sits there
 * forever: no error, no retry, no escalation, and the mission simply stops
 * making progress while looking perfectly healthy.
 *
 * The usual cause is the two Actions secrets being absent, in which case CI
 * builds and tests normally and then discards its own verdict — which is why
 * this says so by name rather than reporting a timeout.
 */
async function warnAboutStuckVerifications(): Promise<void> {
  const cutoff = new Date(Date.now() - STUCK_AFTER_MS).toISOString();

  const { data: stuck } = await db
    .from("tasks")
    .select("id,mission_id,assigned_to,branch,updated_at")
    .eq("status", "awaiting_verification")
    .lt("updated_at", cutoff);

  for (const task of stuck ?? []) {
    if (warnedStuck.has(task.id)) continue;

    // A run row means CI did reach us; the task is merely slow, not stranded.
    const { count } = await db
      .from("runs")
      .select("id", { count: "exact", head: true })
      .eq("task_id", task.id);
    if ((count ?? 0) > 0) continue;

    warnedStuck.add(task.id);
    await emit({
      missionId: task.mission_id,
      taskId: task.id,
      agentId: task.assigned_to,
      level: "error",
      type: "verdict_missing",
      message:
        `Aucun verdict de CI depuis ${Math.round(STUCK_AFTER_MS / 60000)} min sur ${task.branch}. ` +
        `La tâche restera bloquée tant qu'un run n'arrive pas. Cause la plus probable : ` +
        `les secrets SUPABASE_URL et SUPABASE_SERVICE_ROLE_KEY manquent dans ` +
        `Settings → Secrets → Actions du dépôt.`,
    });
    log.warn(`verdict manquant sur ${task.branch} — secrets Actions absents ?`);
  }
}

const STUCK_AFTER_MS = 20 * 60 * 1000;

/** Realtime is the primary wake-up; the interval is the fallback. */
function subscribe(): void {
  db.channel("worker")
    .on("postgres_changes", { event: "*", schema: "public", table: "missions" }, nudge)
    .on("postgres_changes", { event: "*", schema: "public", table: "tasks" }, nudge)
    .on("postgres_changes", { event: "*", schema: "public", table: "runs" }, nudge)
    .subscribe((status) => log.debug(`realtime: ${status}`));
}

function nudge(): void {
  wakeUp?.();
}

function waitForWork(): Promise<void> {
  return new Promise((resolve) => {
    let done = false;
    const finish = () => {
      if (done) return;
      done = true;
      wakeUp = null;
      clearTimeout(timer);
      resolve();
    };
    const timer = setTimeout(finish, config.pollIntervalMs);
    wakeUp = finish;
  });
}

function installShutdown(): void {
  for (const signal of ["SIGINT", "SIGTERM"] as const) {
    process.on(signal, () => {
      if (!running) process.exit(1); // second signal: give up waiting
      log.info(`${signal} reçu — arrêt après la tâche en cours`);
      running = false;
      wakeUp?.();
    });
  }
}

await main();
