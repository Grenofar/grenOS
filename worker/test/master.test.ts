import { test } from "node:test";
import assert from "node:assert/strict";

process.env.GRENOS_ROOT = new URL("../..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
process.env.SUPABASE_URL ??= "https://test.supabase.co";
process.env.SUPABASE_SERVICE_ROLE_KEY ??= "test";
process.env.GEMINI_API_KEY ??= "test";
process.env.GITHUB_TOKEN ??= "test";

const { signatureOf, parkedSpecGaps, isMissionComplete } = await import("../src/master.ts");

const mission = {
  id: "m1",
  title: "t",
  description: "d",
  status: "running",
  token_budget: 1000,
  tokens_used: 0,
};

const task = (over: Record<string, unknown> = {}) => ({
  id: "aaaaaaaa-1111-2222-3333-444444444444",
  assigned_to: "coder",
  goal: "g",
  status: "in_progress",
  attempt: 1,
  max_attempts: 3,
  failure: null,
  failure_detail: null,
  ...over,
});

const state = (over: Record<string, unknown> = {}) => ({
  tasks: [task()],
  activeCount: 1,
  pending: [],
  runs: [],
  ...over,
});

// The whole point of the signature is quota: the free tier allows about 20
// requests per model per day, and the loop wakes several times a minute. If
// the signature moved when nothing had, the Master would think itself broke.
test("an unchanged board produces an unchanged signature", () => {
  assert.equal(signatureOf(mission, state() as never), signatureOf(mission, state() as never));
});

test("a task changing state changes the signature", () => {
  const before = signatureOf(mission, state() as never);
  const after = signatureOf(mission, state({ tasks: [task({ status: "done" })] }) as never);
  assert.notEqual(before, after);
});

test("a retry changes the signature", () => {
  const before = signatureOf(mission, state() as never);
  const after = signatureOf(mission, state({ tasks: [task({ attempt: 2 })] }) as never);
  assert.notEqual(before, after);
});

test("a returned result changes the signature", () => {
  const before = signatureOf(mission, state() as never);
  const after = signatureOf(mission, state({ pending: [{ id: "msg1", from_agent: "coder", content: {} }] }) as never);
  assert.notEqual(before, after);
});

test("a CI verdict changes the signature", () => {
  const before = signatureOf(mission, state() as never);
  const after = signatureOf(
    mission,
    state({ runs: [{ branch: "agent/aaaaaaaa", status: "failed", failure: null, verdicts: [], log_excerpt: null }] }) as never,
  );
  assert.notEqual(before, after);
});

test("task order does not change the signature", () => {
  const a = task({ id: "aaaaaaaa-0000-0000-0000-000000000000" });
  const b = task({ id: "bbbbbbbb-0000-0000-0000-000000000000" });
  assert.equal(
    signatureOf(mission, state({ tasks: [a, b] }) as never),
    signatureOf(mission, state({ tasks: [b, a] }) as never),
  );
});

test("only tasks parked on spec_gap are waiting for the Master to replace them", () => {
  const parked = task({ id: "p", status: "blocked", failure: "spec_gap" });
  // Blocked on a question: its answer resumes it, it must not be discarded.
  const asking = task({ id: "q", status: "blocked", failure: null });
  const running = task({ id: "r", status: "in_progress", failure: "spec_gap" });
  assert.deepEqual(parkedSpecGaps(state({ tasks: [parked, asking, running] }) as never), ["p"]);
});

test("superseded tasks do not stop a mission from completing", () => {
  // Mission 1 carries four cancelled tasks. Requiring every task to be done
  // meant it could never close on its own, even with a green kernel.
  const board = state({
    tasks: [task({ id: "a", status: "cancelled" }), task({ id: "b", status: "done" })],
    activeCount: 0,
  });
  assert.equal(isMissionComplete(board as never), true);
});

test("a mission is not complete while work is open, failed or absent", () => {
  const open = state({ tasks: [task({ status: "done" }), task({ id: "x", status: "ready" })], activeCount: 1 });
  const failed = state({ tasks: [task({ status: "done" }), task({ id: "y", status: "failed" })], activeCount: 0 });
  const parked = state({ tasks: [task({ status: "blocked", failure: "spec_gap" })], activeCount: 0 });
  const onlyCancelled = state({ tasks: [task({ status: "cancelled" })], activeCount: 0 });
  assert.equal(isMissionComplete(open as never), false);
  assert.equal(isMissionComplete(failed as never), false);
  assert.equal(isMissionComplete(parked as never), false);
  assert.equal(isMissionComplete(onlyCancelled as never), false);
});
