import { db } from "./db.ts";
import { WRITERS } from "./autopilot.ts";

/**
 * Which branch a new task builds on (D-026, D-027).
 *
 * Every task used to start from main. But work reaches main only once CI is
 * green, so a follow-up task started from nothing: on mission 1 the Coder's
 * second task was cut from a main with no kernel at all, lost the pinned
 * toolchain its predecessor had finally got to compile, and failed on its
 * first line.
 *
 * "The latest writer branch" was not the answer either. After mission 1 the
 * latest one did not even parse, while an older one built and failed only on
 * clippy. A writer's task now continues from the branch whose last CI run got
 * furthest — green, then clippy or the boot passing, then built — and the
 * most recent among equals, unless the Master names another one, or main to
 * start clean.
 */

export interface Lineage {
  id: string;
  assigned_to: string;
  branch: string | null;
  created_at: string;
}

export interface RunProgress {
  branch: string;
  status: string;
  verdicts?: unknown;
  log_excerpt?: string | null;
  started_at: string;
}

/**
 * How far a CI run got: 4 green, 3 built with clippy or the boot passing,
 * 2 built, 1 did not build, 0 nothing CI could judge (no kernel, a runner
 * problem, still running).
 */
export function progress(run: Pick<RunProgress, "status" | "verdicts" | "log_excerpt">): number {
  if (run.status === "passed") return 4;
  if (run.status !== "failed" && run.status !== "timeout") return 0;

  // CI lists its steps in the verdict (scripts/ci-payload.py).
  const steps = Array.isArray(run.verdicts)
    ? (run.verdicts as Array<Record<string, unknown> | null>).filter(
        (v) => typeof v?.["step"] === "string",
      )
    : [];
  if (steps.length > 0) {
    return Math.min(3, 1 + steps.filter((v) => v?.["verdict"] === "PASS").length);
  }

  // Runs from before that: CI only runs clippy and the boot once the build
  // has passed, and the log names each step it ran.
  return /^--- (clippy|qemu) ---$/m.test(run.log_excerpt ?? "") ? 2 : 1;
}

export function pickBase(
  tasks: Lineage[],
  assignee: string,
  continueFrom?: string,
  runs: RunProgress[] = [],
): string | null {
  if (!WRITERS.has(assignee)) return null;

  const wanted = continueFrom?.trim().replace(/^agent\//, "") ?? "";
  if (wanted === "main") return null;
  if (wanted.length >= 4) {
    const named = tasks.find((t) => t.id.startsWith(wanted) || t.branch === `agent/${wanted}`);
    if (named) return named.branch ?? `agent/${named.id.slice(0, 8)}`;
  }

  // A branch is judged by its last run, not by its best one: a later commit
  // that broke the build is what the next task would inherit.
  const last = new Map<string, RunProgress>();
  for (const run of runs) {
    const seen = last.get(run.branch);
    if (!seen || run.started_at > seen.started_at) last.set(run.branch, run);
  }
  const score = (t: Lineage): number => {
    const run = last.get(t.branch!);
    return run ? progress(run) : 0;
  };

  const best = tasks
    .filter((t) => WRITERS.has(t.assigned_to) && t.branch)
    .sort((a, b) => score(b) - score(a) || b.created_at.localeCompare(a.created_at))[0];
  return best?.branch ?? null;
}

export async function baseFor(
  missionId: string,
  assignee: string,
  continueFrom?: string,
): Promise<string | null> {
  if (!WRITERS.has(assignee)) return null;

  const [{ data: tasks }, { data: runs }] = await Promise.all([
    db.from("tasks").select("id,assigned_to,branch,created_at").eq("mission_id", missionId),
    db
      .from("runs")
      .select("branch,status,verdicts,log_excerpt,started_at")
      .eq("mission_id", missionId)
      .order("started_at", { ascending: false })
      .limit(100),
  ]);
  return pickBase((tasks ?? []) as Lineage[], assignee, continueFrom, (runs ?? []) as RunProgress[]);
}
