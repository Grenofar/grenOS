import { test } from "node:test";
import assert from "node:assert/strict";

process.env.GRENOS_ROOT = new URL("../..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
process.env.SUPABASE_URL ??= "https://test.supabase.co";
process.env.SUPABASE_SERVICE_ROLE_KEY ??= "test";
process.env.GEMINI_API_KEY ??= "test";
process.env.GITHUB_TOKEN ??= "test";

const {
  humanLanguage,
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
  assert.match(first, /# Carnet du Maître/);
  assert.match(first, /## Journal des échanges\n\n- a\n$/);

  // The Master rewrites its instructions and forgets the journal: kept anyway.
  const second = withJournal("# Carnet du Maître\n\n## Consignes en vigueur\n\n- pas de crate x86_64", first, "- b");
  assert.match(second, /- pas de crate x86_64/);
  assert.match(second, /- a\n- b\n$/);

  // The Master does not write this time: its instructions stay as they were.
  const third = withJournal(second, second, "- c");
  assert.match(third, /- pas de crate x86_64/);
  assert.match(third, /- a\n- b\n- c\n$/);
});

test("the journal keeps the most recent exchanges only", () => {
  let text = "";
  for (let i = 0; i < 50; i++) text = withJournal(text, text, `- e${i}`, 40);
  assert.doesNotMatch(text, /- e9\n/);
  assert.match(text, /- e10\n/);
  assert.match(text, /- e49\n$/);
});

test("a journal entry is one bounded line", () => {
  const entry = journalEntry([{ content: "ligne 1\nligne 2", created_at: "2026-09-10T21:40:12Z" }], "ok\nnoté");
  assert.equal(entry, "- 2026-09-10 21:40 UTC · Humain : « ligne 1 ligne 2 » → Maître : « ok noté »");
  const long = journalEntry([{ content: "y".repeat(5000), created_at: "2026-09-10T21:40:12Z" }], "z".repeat(5000));
  assert.ok(long.length < 1200);
  assert.equal(long.split("\n").length, 1);
});

test("the Master is told which language to answer in", () => {
  // The first real exchange: a French message, answered in English.
  assert.equal(
    humanLanguage(["Le build passe. Continue : corrige clippy avec un hlt dans la boucle, puis produis l'image."]),
    "French",
  );
  assert.equal(humanLanguage(["The build passes. Now fix clippy and then make the image."]), "English");
  assert.equal(humanLanguage(["ok"]), "the human's language");
});

test("a key pasted into the chat never reaches the public repository", () => {
  // Built at runtime so that no credential-shaped literal sits in the source.
  const nvidia = "nvapi-" + "a".repeat(30);
  const google = "AIza" + "S".repeat(35);
  const text = redactSecrets(`voici ${nvidia} et ${google}, merci`);
  assert.equal(text, "voici [secret masqué] et [secret masqué], merci");
  assert.equal(redactSecrets("rien de secret ici"), "rien de secret ici");
});
