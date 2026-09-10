import { test } from "node:test";
import assert from "node:assert/strict";

process.env.GRENOS_ROOT = new URL("../..", import.meta.url).pathname.replace(
  /^\/([A-Za-z]:)/,
  "$1",
);
process.env.SUPABASE_URL ??= "https://test.supabase.co";
process.env.SUPABASE_SERVICE_ROLE_KEY ??= "test";
process.env.GEMINI_API_KEY ??= "test";
process.env.GITHUB_TOKEN ??= "test";

const { failurePatch, specGapPatch } = await import("../src/executor.ts");

test("an agent's own failure parks the task instead of handing it straight back", () => {
  // Mission 1: the Coder said the task could not be done as specified, was
  // given the same task again, and repeated itself. Meanwhile the Master's
  // corrected task was refused as a duplicate of the one still open.
  const patch = specGapPatch("x86_64-grenos.json is missing");
  assert.equal(patch.status, "blocked");
  assert.equal(patch.failure, "spec_gap");
  assert.equal(patch.failure_detail, "x86_64-grenos.json is missing");
  // The attempts belong to the envelope, and the envelope is what was wrong.
  assert.equal(patch.attempt, undefined);
});

test("a parked task's detail is bounded like any other", () => {
  assert.equal((specGapPatch("y".repeat(9_000)).failure_detail as string).length, 4000);
});

test("an infrastructure failure never writes where the agent will read it", () => {
  // failure_detail is quoted into the next prompt as "Previous attempt failed".
  // Mission 1 lost a healthy task when a provider outage was written there:
  // the model read "No model available", concluded it could not work, and
  // reported failed.
  const patch = failurePatch(
    { attempt: 2, max_attempts: 3 },
    "provider_error",
    "No model available for role coder",
    false,
  );
  assert.deepEqual(patch, { status: "ready" });
});

test("an agent failure with attempts left retries and advances the counter", () => {
  const patch = failurePatch({ attempt: 1, max_attempts: 3 }, "compile_error", "error[E0425]", true);
  assert.equal(patch.status, "ready");
  assert.equal(patch.attempt, 2);
  assert.equal(patch.failure_detail, "error[E0425]");
  assert.equal(patch.finished_at, undefined);
});

test("the last attempt fails the task without breaking the CHECK constraint", () => {
  const now = new Date("2026-09-10T12:00:00Z");
  const patch = failurePatch({ attempt: 3, max_attempts: 3 }, "compile_error", "boom", true, now);
  assert.equal(patch.status, "failed");
  // attempt = max_attempts + 1 is rejected by tasks_attempt_within_limit, and
  // the task then stays in_progress forever.
  assert.equal(patch.attempt, undefined);
  assert.equal(patch.finished_at, now.toISOString());
});

test("failure_detail is bounded", () => {
  const patch = failurePatch({ attempt: 1, max_attempts: 3 }, "test_failure", "x".repeat(10_000), true);
  assert.equal((patch.failure_detail as string).length, 4000);
});
