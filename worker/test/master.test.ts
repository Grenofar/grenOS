import { test } from "node:test";
import assert from "node:assert/strict";

process.env.GRENOS_ROOT = new URL("../..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
process.env.SUPABASE_URL ??= "https://test.supabase.co";
process.env.SUPABASE_SERVICE_ROLE_KEY ??= "test";
process.env.GEMINI_API_KEY ??= "test";
process.env.GITHUB_TOKEN ??= "test";

const {
  HUMAN_LANGUAGE,
  UNREADABLE_LIMIT,
  afterUnreadable,
  renderState,
  signatureOf,
  parkedSpecGaps,
  isMissionComplete,
  attemptsUsed,
  digestOfMessage,
  withJournal,
  journalEntry,
  redactSecrets,
} = await import("../src/master.ts");

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
  human: [],
  conversation: [],
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

test("a message from the human changes the signature", () => {
  // Otherwise a human writing while work is in flight would wait for the next
  // CI verdict to be read.
  const before = signatureOf(mission, state() as never);
  const after = signatureOf(
    mission,
    state({ human: [{ id: "h1", content: "stop", created_at: "2026-09-10T20:00:00Z" }] }) as never,
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

test("attempts are counted as spent, not as numbered", () => {
  // Mission 1: the Master read "attempt 3 of 3, ready" as exhausted and
  // escalated a task whose last attempt had not run yet.
  assert.equal(attemptsUsed({ status: "ready", attempt: 3 }), 2);
  assert.equal(attemptsUsed({ status: "in_progress", attempt: 3 }), 3);
  assert.equal(attemptsUsed({ status: "failed", attempt: 3 }), 3);
});

test("the Master reads what an agent did, not every byte it wrote", () => {
  const digest = digestOfMessage({
    status: "done",
    actions: [
      { type: "write_file", path: "kernel/src/main.rs", content: "x".repeat(5000) },
      { type: "request_build" },
    ],
  }) as { actions: Array<Record<string, unknown>> };
  assert.equal(digest.actions[0]!.path, "kernel/src/main.rs");
  assert.ok((digest.actions[0]!.content as string).length < 300);
  assert.deepEqual(digest.actions[1], { type: "request_build" });
  // Anything that is not an envelope passes through untouched.
  assert.equal(digestOfMessage("text"), "text");
});

test("every exchange lands in the notebook journal, whatever the model rewrote", () => {
  const first = withJournal("", "", "- a");
  assert.match(first, /# Master's notebook/);
  assert.match(first, /## Exchange log\n\n- a\n$/);

  // The Master rewrites its instructions and forgets the journal: kept anyway.
  const second = withJournal("# Master's notebook\n\n## Standing instructions\n\n- no x86_64 crate", first, "- b");
  assert.match(second, /- no x86_64 crate/);
  assert.match(second, /- a\n- b\n$/);

  // The Master does not write this time: its instructions stay as they were.
  const third = withJournal(second, second, "- c");
  assert.match(third, /- no x86_64 crate/);
  assert.match(third, /- a\n- b\n- c\n$/);
});

test("a notebook written in French before D-028 keeps its journal", () => {
  // docs/MASTER.md as the Master left it on 2026-09-10.
  const before =
    "# Instructions du Maître\n\n## Consignes en vigueur\n- Le build doit passer.\n\n" +
    "## Journal des échanges\n\n- 2026-09-10 19:54 UTC · Humain : « Le build passe. »\n";
  const after = withJournal(before, before, "- new");
  assert.match(after, /- Le build doit passer\./);
  assert.match(after, /## Exchange log\n\n- 2026-09-10 19:54 UTC · Humain : « Le build passe\. »\n- new\n$/);
  assert.doesNotMatch(after, /Journal des échanges/);
});

test("the journal keeps the most recent exchanges only", () => {
  let text = "";
  for (let i = 0; i < 50; i++) text = withJournal(text, text, `- e${i}`, 40);
  assert.doesNotMatch(text, /- e9\n/);
  assert.match(text, /- e10\n/);
  assert.match(text, /- e49\n$/);
});

test("a journal entry is one bounded line", () => {
  const entry = journalEntry([{ content: "ligne 1\nligne 2", created_at: "2026-09-10T21:40:12Z" }], "ok\nnoted");
  assert.equal(entry, '- 2026-09-10 21:40 UTC · Human: "ligne 1 ligne 2" → Master: "ok noted"');
  const long = journalEntry([{ content: "y".repeat(5000), created_at: "2026-09-10T21:40:12Z" }], "z".repeat(5000));
  assert.ok(long.length < 1200);
  assert.equal(long.split("\n").length, 1);
});

test("the Master answers the human in English, whatever language they write in", () => {
  // Asked by the human on 2026-09-11 (D-028), after a day of replies in French.
  assert.equal(HUMAN_LANGUAGE, "English");
  const text = renderState(
    mission,
    state({
      human: [{ id: "h1", content: "Le build passe. Continue : corrige clippy.", created_at: "2026-09-10T19:54:00Z" }],
    }) as never,
    null,
    new Date("2026-09-11T08:00:00Z"),
  );
  assert.match(text, /write it in English/);
  assert.match(text, /`options` of any escalation, in English/);
  assert.doesNotMatch(text, /French/);
});

test("an unreadable decision is judged again, and the third in a row reaches the human", () => {
  // Mission 1, 2026-09-11: status "escalate", then prose. Both were refused
  // and nothing happened for eight hours.
  assert.equal(UNREADABLE_LIMIT, 3);
  assert.equal(afterUnreadable(1), "retry");
  assert.equal(afterUnreadable(2), "retry");
  assert.equal(afterUnreadable(3), "escalate");
});

test("the Master knows what time it is", () => {
  // Without it, docs/STATE.md said "At 2026-09-10 [current time]".
  const text = renderState(mission, state() as never, null, new Date("2026-09-11T08:05:00Z"));
  assert.match(text, /# Now\n\n2026-09-11 08:05 UTC/);
});

test("a key pasted into the chat never reaches the public repository", () => {
  // Built at runtime so that no credential-shaped literal sits in the source.
  const nvidia = "nvapi-" + "a".repeat(30);
  const google = "AIza" + "S".repeat(35);
  const text = redactSecrets(`voici ${nvidia} et ${google}, merci`);
  assert.equal(text, "voici [redacted secret] et [redacted secret], merci");
  assert.equal(redactSecrets("rien de secret ici"), "rien de secret ici");
});

test("new work closes the tasks it answers, so they are not escalated again", async () => {
  const { supersededByNewWork } = await import("../src/master.ts");
  // Mission 1: two Coder tasks failed their last attempt, and the Master
  // escalated the same one again on every cycle, answer or not.
  const board = state({
    tasks: [
      task({ id: "failed", status: "failed", attempt: 3 }),
      task({ id: "parked", status: "blocked", failure: "spec_gap" }),
      task({ id: "asking", status: "blocked", failure: null }),
      task({ id: "done", status: "done" }),
      task({ id: "open", status: "ready" }),
    ],
  });
  assert.deepEqual(supersededByNewWork(board as never).sort(), ["failed", "parked"]);
});

test("the Master reads a CI run's verdict and its errors, not the whole log", async () => {
  const { ciDigest } = await import("../src/master.ts");
  // Eight full log tails per cycle made the Master the biggest spender of
  // mission 1, ahead of the Coder.
  const log = [
    "--- build ---",
    "   Compiling kernel v0.1.0 (/home/runner/work/grenOS/grenOS/kernel)",
    "    Finished `release` profile [optimized] target(s) in 0.10s",
    "--- clippy ---",
    "error: empty `loop {}` wastes CPU cycles",
    " --> src/main.rs:8:5",
    ...Array.from({ length: 300 }, () => "   noise from a long build"),
  ].join("\n");
  const run = {
    branch: "agent/4b7d4930",
    status: "failed",
    failure: "compile_error",
    verdicts: [],
    log_excerpt: log,
  };

  const digest = ciDigest(run);
  // The build passed and clippy failed: exactly what the Master must see.
  assert.match(String(digest.errors), /Finished `release`/);
  assert.match(String(digest.errors), /empty `loop \{\}`/);
  assert.match(String(digest.errors), /--> src\/main\.rs:8:5/);
  assert.doesNotMatch(String(digest.errors), /noise|Compiling/);
  assert.ok(String(digest.errors).length <= 1500);
  assert.equal(ciDigest({ ...run, log_excerpt: null }).errors, null);
});
