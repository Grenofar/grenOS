import { db } from "./db.ts";
import { WRITERS } from "./autopilot.ts";

/**
 * Which branch a new task builds on (D-026).
 *
 * Every task used to start from main. But work reaches main only once CI is
 * green, so a follow-up task started from nothing: on mission 1 the Coder's
 * second task was cut from a main with no kernel at all, lost the pinned
 * toolchain its predecessor had finally got to compile, and failed on its
 * first line. A writer's task now continues from the mission's latest writer
 * branch, unless the Master names another one — or main, to start clean.
 */

export interface Lineage {
  id: string;
  assigned_to: string;
  branch: string | null;
  created_at: string;
}

export function pickBase(tasks: Lineage[], assignee: string, continueFrom?: string): string | null {
  if (!WRITERS.has(assignee)) return null;

  const wanted = continueFrom?.trim().replace(/^agent\//, "") ?? "";
  if (wanted === "main") return null;
  if (wanted.length >= 4) {
    const named = tasks.find((t) => t.id.startsWith(wanted) || t.branch === `agent/${wanted}`);
    if (named) return named.branch ?? `agent/${named.id.slice(0, 8)}`;
  }

  const latest = tasks
    .filter((t) => WRITERS.has(t.assigned_to) && t.branch)
    .sort((a, b) => b.created_at.localeCompare(a.created_at))[0];
  return latest?.branch ?? null;
}

export async function baseFor(
  missionId: string,
  assignee: string,
  continueFrom?: string,
): Promise<string | null> {
  const { data } = await db
    .from("tasks")
    .select("id,assigned_to,branch,created_at")
    .eq("mission_id", missionId);
  return pickBase((data ?? []) as Lineage[], assignee, continueFrom);
}
