import { log } from "./config.ts";
import { db, emit } from "./db.ts";
import type { AgentDefinition } from "./prompts.ts";

/**
 * What keeps a mission informed while the Master has no quota (D-023).
 *
 * Work never waited for the Master: a ready task is claimed without it, and a
 * red CI verdict sends the task back to its author through the database
 * trigger. What stops is judgement — and with it the analysis the Master would
 * have asked for. So for every CI verdict on a writer's task that the Master
 * has not seen yet, the Tester records a per-criterion verdict and the Review
 * agent analyses the code. Both are read-only, and both report to the Master:
 * when its quota returns, their results are waiting unseen in its inbox, and
 * master.ts reads unseen results first.
 *
 * Deterministic on purpose. No model decides what gets reviewed, because the
 * model that would decide is precisely the one out of quota.
 */

/** Agents whose tasks produce code that CI judges. */
const WRITERS = new Set(["coder", "kernel", "drivers", "filesystem"]);

/** CI verdicts followed up per tick, so a backlog cannot flood the queue. */
const PER_TICK = 2;

export interface RunRow {
  id: string;
  task_id: string | null;
  mission_id: string;
  branch: string;
  status: string;
  started_at: string;
}

export interface WriterTask {
  id: string;
  mission_id: string;
  assigned_to: string;
  goal: string;
  branch: string | null;
}

type Team = Map<
  string,
  Pick<AgentDefinition, "status" | "maxAttempts" | "maxTokensPerTask" | "allowedPaths">
>;

/** A verdict on code is worth a second look; a verdict on anything else is not. */
export function needsFollowUp(
  run: Pick<RunRow, "status">,
  task: Pick<WriterTask, "assigned_to">,
): boolean {
  // "error" is the runner or the network, and says nothing about the code.
  return WRITERS.has(task.assigned_to) && (run.status === "passed" || run.status === "failed");
}

/** The read-only follow-ups one CI verdict gets, as task rows ready to insert. */
export function followUps(
  run: RunRow,
  task: WriterTask,
  team: Team,
): Array<Record<string, unknown>> {
  // Rendered into the prompt by evidence.ts: the run with its log, the task
  // with its criteria. Also what marks this verdict as already followed up.
  const refs = [`run:${run.id}`, `task:${task.id}`];
  const rows: Array<Record<string, unknown>> = [];

  const tester = team.get("tester");
  if (tester?.status === "active") {
    rows.push({
      mission_id: task.mission_id,
      assigned_to: "tester",
      goal: `Record a per-criterion verdict on the CI run of ${run.branch} (${run.status}). Write no file.`,
      acceptance_criteria: [
        "every acceptance criterion of the task under review is listed as PASS, FAIL or UNVERIFIABLE",
        "every verdict quotes the CI log line it rests on",
      ],
      context_refs: refs,
      allowed_paths: tester.allowedPaths,
      status: "ready",
      max_attempts: tester.maxAttempts,
      token_budget: tester.maxTokensPerTask,
    });
  }

  const review = team.get("review");
  if (review?.status === "active") {
    rows.push({
      mission_id: task.mission_id,
      assigned_to: "review",
      goal: `Review the code on ${run.branch} for the task under review and report every defect you find. Write no file.`,
      acceptance_criteria: [
        "every finding names a file and, where it can, a line",
        "every finding is marked blocking or non-blocking",
      ],
      context_refs: refs,
      allowed_paths: review.allowedPaths,
      // Read the writer's files rather than main: they are what is under
      // review. The Review agent cannot write, so nothing lands on that branch.
      branch: task.branch,
      status: "ready",
      max_attempts: review.maxAttempts,
      token_budget: review.maxTokensPerTask,
    });
  }

  return rows;
}

export async function runAutopilot(team: Map<string, AgentDefinition>): Promise<void> {
  const { data: missions } = await db
    .from("missions")
    .select("id")
    .in("status", ["planning", "running"]);

  let budget = PER_TICK;

  for (const mission of missions ?? []) {
    if (budget <= 0) return;

    // "Not seen yet" means newer than the Master's last decision on this
    // mission: anything older, it already judged.
    const { data: last } = await db
      .from("messages")
      .select("created_at")
      .eq("mission_id", mission.id)
      .eq("kind", "decision")
      .order("created_at", { ascending: false })
      .limit(1)
      .maybeSingle();

    let query = db
      .from("runs")
      .select("id,task_id,mission_id,branch,status,started_at")
      .eq("mission_id", mission.id)
      .not("task_id", "is", null)
      .order("started_at");
    if (last) query = query.gt("started_at", last.created_at);
    const { data: runs } = await query.limit(10);

    for (const run of (runs ?? []) as RunRow[]) {
      if (budget <= 0) return;

      const { count } = await db
        .from("tasks")
        .select("id", { count: "exact", head: true })
        .contains("context_refs", [`run:${run.id}`]);
      if ((count ?? 0) > 0) continue;

      const { data: task } = await db
        .from("tasks")
        .select("id,mission_id,assigned_to,goal,branch")
        .eq("id", run.task_id!)
        .maybeSingle();
      if (!task || !needsFollowUp(run, task)) continue;

      const rows = followUps(run, task as WriterTask, team);
      if (rows.length === 0) continue;

      const { error } = await db.from("tasks").insert(rows);
      if (error) {
        log.warn(`autopilot : suivi refusé pour ${run.branch} — ${error.message}`);
        continue;
      }

      budget -= 1;
      const who = rows.map((r) => r.assigned_to).join(" et ");
      await emit({
        missionId: mission.id,
        agentId: "master",
        level: "info",
        type: "autopilot",
        message: `Maître sans quota : ${who} lancés sur le verdict de ${run.branch}. Leurs rapports l'attendront.`,
        payload: { run: run.id },
      });
      log.info(`autopilot · ${who} sur ${run.branch}`);
    }
  }
}
