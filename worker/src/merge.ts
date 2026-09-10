import { log } from "./config.ts";
import { db, emit } from "./db.ts";
import { WRITERS } from "./autopilot.ts";
import type { GitHub } from "./github.ts";

/**
 * Verified work reaches main (D-026).
 *
 * Nothing ever merged an agent's branch: main held documents and no kernel,
 * so every new task, and every new mission, started from nothing. A writer's
 * task whose CI run passed — build, clippy and a QEMU boot — is now merged
 * into the default branch, once. "Never merged without a green CI run" was
 * half of the rule; "merged when green" is the other half.
 */

const WINDOW_MS = 7 * 24 * 60 * 60 * 1000;

export async function mergeVerifiedWork(gh: GitHub): Promise<void> {
  const since = new Date(Date.now() - WINDOW_MS).toISOString();
  const { data: tasks } = await db
    .from("tasks")
    .select("id,mission_id,assigned_to,branch")
    .eq("status", "done")
    .in("assigned_to", [...WRITERS])
    .not("branch", "is", null)
    .gte("finished_at", since);

  for (const task of tasks ?? []) {
    const { count: handled } = await db
      .from("events")
      .select("id", { count: "exact", head: true })
      .eq("task_id", task.id)
      .in("type", ["branch_merged", "merge_conflict"]);
    if (handled) continue;

    // A green run is the only licence to merge.
    const { count: passed } = await db
      .from("runs")
      .select("id", { count: "exact", head: true })
      .eq("task_id", task.id)
      .eq("status", "passed");
    if (!passed) continue;

    const outcome = await gh.merge(
      task.branch!,
      `merge ${task.branch}: verified by CI (task ${task.id.slice(0, 8)})`,
    );
    const conflict = outcome === "conflict";
    await emit({
      missionId: task.mission_id,
      taskId: task.id,
      agentId: task.assigned_to,
      level: conflict ? "error" : "info",
      type: conflict ? "merge_conflict" : "branch_merged",
      message: conflict
        ? `${task.branch} vérifiée par la CI mais en conflit avec main : fusion à arbitrer.`
        : `${task.branch} vérifiée par la CI et fusionnée dans main (${outcome}).`,
      payload: { branch: task.branch, outcome },
    });
    log.info(`fusion ${task.branch} → ${outcome}`);
  }
}
