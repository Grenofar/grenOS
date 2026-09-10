import { db } from "./db.ts";

/**
 * Evidence a task is about, rendered into its prompt: a CI run to judge, or
 * another task to review (autopilot.ts). Named in context_refs as `run:<id>`
 * and `task:<id>`. An agent told "see run 42" cannot open it, so it is shown
 * the run itself.
 */
export async function renderEvidence(refs: string[]): Promise<string[]> {
  const out: string[] = [];
  for (const ref of refs) {
    if (ref.startsWith("run:")) out.push(await renderRun(ref.slice(4)));
    else if (ref.startsWith("task:")) out.push(await renderTask(ref.slice(5)));
  }
  return out;
}

async function renderRun(id: string): Promise<string> {
  const { data } = await db.from("runs").select("*").eq("id", id).maybeSingle();
  if (!data) return `\n# CI run ${id}\n\n(not found)`;

  const summary = {
    branch: data.branch,
    commit: data.commit_sha,
    status: data.status,
    failure: data.failure,
    verdicts: data.verdicts,
  };
  return (
    `\n# CI run under review\n\n${JSON.stringify(summary, null, 2)}\n\n` +
    `Log excerpt (the end of build, clippy and QEMU output):\n\n` +
    `${FENCE}\n${data.log_excerpt ?? "(none)"}\n${FENCE}`
  );
}

async function renderTask(id: string): Promise<string> {
  const { data } = await db
    .from("tasks")
    .select("goal,acceptance_criteria,assigned_to,branch,status,attempt,max_attempts,failure")
    .eq("id", id)
    .maybeSingle();
  if (!data) return `\n# Task ${id}\n\n(not found)`;
  return `\n# Task under review\n\n${JSON.stringify(data, null, 2)}`;
}

// Four backticks: CI logs quote Rust code, which has its own triple fences.
const FENCE = "````";
