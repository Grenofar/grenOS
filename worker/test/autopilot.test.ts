import { test } from "node:test";
import assert from "node:assert/strict";

process.env.GRENOS_ROOT = new URL("../..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
process.env.SUPABASE_URL ??= "https://test.supabase.co";
process.env.SUPABASE_SERVICE_ROLE_KEY ??= "test";
process.env.GEMINI_API_KEY ??= "test";
process.env.GITHUB_TOKEN ??= "test";

const { followUps, needsFollowUp } = await import("../src/autopilot.ts");

// While the Master is out of quota, nothing judges a CI verdict. These are the
// rules for who looks at it instead, so that the Master comes back to reports
// rather than to a pile of raw runs.

const run = {
  id: "r1",
  task_id: "t1",
  mission_id: "m1",
  branch: "agent/t1",
  status: "failed",
  started_at: "2026-09-10T19:29:41Z",
};
const task = { id: "t1", mission_id: "m1", assigned_to: "coder", goal: "g", branch: "agent/t1" };
const def = (status = "active") => ({
  status,
  maxAttempts: 2,
  maxTokensPerTask: 30000,
  allowedPaths: ["tests/**"],
});
const team = new Map([
  ["tester", def()],
  ["review", def()],
]);

test("a verdict the Master has not seen gets a Tester verdict and a Review", () => {
  const rows = followUps(run, task, team as never);
  assert.deepEqual(
    rows.map((r) => r.assigned_to),
    ["tester", "review"],
  );
  // The references are the evidence, and also what marks the run as handled.
  for (const r of rows) assert.deepEqual(r.context_refs, ["run:r1", "task:t1"]);
  for (const r of rows) assert.equal(r.status, "ready");
});

test("the Review reads the writer's branch; the Tester's evidence is the run", () => {
  const [tester, review] = followUps(run, task, team as never);
  assert.equal(review!.branch, "agent/t1");
  assert.equal(tester!.branch, undefined);
});

test("nobody dormant is handed work", () => {
  const rows = followUps(run, task, new Map([["tester", def("dormant")], ["review", def()]]) as never);
  assert.deepEqual(
    rows.map((r) => r.assigned_to),
    ["review"],
  );
});

test("only verdicts on code are followed up", () => {
  assert.equal(needsFollowUp(run, task), true);
  assert.equal(needsFollowUp({ status: "passed" }, { assigned_to: "drivers" }), true);
  // A report on a report is not information.
  assert.equal(needsFollowUp(run, { assigned_to: "tester" }), false);
  // An infrastructure error says nothing about the code.
  assert.equal(needsFollowUp({ status: "error" }, task), false);
});
