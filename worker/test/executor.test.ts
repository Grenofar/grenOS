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

const { choose, panelSize } = await import("../src/executor.ts");

const usage = { tokensIn: 0, tokensOut: 0, latencyMs: 0 };
const ready = (model: string, files: number, built = false, status = "done") => ({
  kind: "ready" as const,
  model,
  envelope: { status, summary: "", actions: [] } as never,
  changes: Array.from({ length: files }, (_, i) => ({ path: `kernel/src/f${i}.rs`, content: "" })),
  usage,
  consulted: 0,
  corrected: 0,
  built,
});
const failed = (model: string, failure: string, consumesAttempt: boolean) => ({
  kind: "failed" as const,
  model,
  failure,
  detail: failure,
  consumesAttempt,
  usage,
});

test("a panel keeps the answer that built over one that only passed pre-flight", () => {
  const chosen = choose([ready("a", 2), ready("b", 1, true), ready("c", 3, true)]);
  assert.equal(chosen.model, "b");
});

test("between equals, the cascade order decides", () => {
  assert.equal(choose([ready("a", 2), ready("b", 2)]).model, "a");
});

test("work beats an answer that wrote nothing, and both beat saying it cannot be done", () => {
  assert.equal(choose([ready("a", 0), ready("b", 1)]).model, "b");
  assert.equal(choose([ready("a", 0, false, "failed"), ready("b", 0)]).model, "b");
  assert.equal(choose([failed("x", "provider_error", false), ready("a", 0, false, "failed")]).model, "a");
  // A member that tried and did not compile outranks one saying it cannot be done.
  assert.equal(choose([ready("n", 0, false, "failed"), failed("d", "compile_error", true)]).model, "d");
});

test("when every member failed, the agents' own failure outranks an outage", () => {
  const chosen = choose([failed("a", "provider_error", false), failed("b", "compile_error", true)]);
  assert.equal(chosen.kind, "failed");
  assert.equal(chosen.model, "b");
  const outage = choose([failed("a", "provider_error", false)]);
  assert.equal(outage.kind === "failed" && outage.consumesAttempt, false);
});

test("an agent that cannot write answers alone", () => {
  assert.equal(panelSize({ modelRole: "coder", canWrite: false }), 1);
  assert.ok(panelSize({ modelRole: "coder", canWrite: true }) >= 1);
  assert.equal(panelSize({ modelRole: "master", canWrite: true }), 1);
});

const { gather } = await import("../src/executor.ts");

const after = <T,>(ms: number, value: T) => new Promise<T>((resolve) => setTimeout(() => resolve(value), ms));

test("a panel waits for everyone when nobody has a good answer yet", async () => {
  const out = await gather([after(5, "bad"), after(30, "bad")], (v) => v === "good", 10);
  assert.deepEqual(out, ["bad", "bad"]);
});

test("once one member is good, the slow ones get the grace period and no more", async () => {
  const started = Date.now();
  const out = await gather([after(2_000, "late"), after(5, "good"), after(15, "also")], (v) => v !== "late", 40);
  assert.deepEqual(out, ["good", "also"]);
  assert.ok(Date.now() - started < 1_000);
});

test("a member that throws does not hold the panel", async () => {
  const out = await gather([Promise.reject(new Error("boom")), after(5, "good")], () => true, 10);
  assert.deepEqual(out, ["good"]);
});

test("a model that keeps reducing its build errors gets more rounds", async () => {
  const { roundsAllowed, IMPROVING_ROUNDS } = await import("../src/executor.ts");
  const { PREFLIGHT_ROUNDS } = await import("../src/preflight.ts");
  assert.equal(roundsAllowed(3, 3, 7), IMPROVING_ROUNDS);
  assert.equal(roundsAllowed(3, 3, 3), PREFLIGHT_ROUNDS);
  // Not only build errors: the static checks keep their usual allowance.
  assert.equal(roundsAllowed(3, 2, 7), PREFLIGHT_ROUNDS);
  assert.equal(roundsAllowed(1, 1, Number.POSITIVE_INFINITY), IMPROVING_ROUNDS);
});

test("a Markdown fence copied around a source file is taken off", async () => {
  const { withoutFences } = await import("../src/executor.ts");
  assert.equal(withoutFences("kernel/src/http.rs", "````\nfn a() {}\n````\n"), "fn a() {}\n");
  assert.equal(withoutFences("kernel/src/http.rs", "```rust\nfn a() {}\n```"), "fn a() {}");
  assert.equal(withoutFences("kernel/src/http.rs", "fn a() {}\n"), "fn a() {}\n");
  assert.equal(withoutFences("docs/PLAN.md", "```\ncode\n```\n"), "```\ncode\n```\n");
});
