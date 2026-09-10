import { test } from "node:test";
import assert from "node:assert/strict";

process.env.GRENOS_ROOT = new URL("../..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
process.env.SUPABASE_URL ??= "https://test.supabase.co";
process.env.SUPABASE_SERVICE_ROLE_KEY ??= "test";
process.env.GEMINI_API_KEY ??= "test";
process.env.GITHUB_TOKEN ??= "test";

const { pickBase } = await import("../src/lineage.ts");

// Mission 1's second Coder task was cut from a main with no kernel, and lost
// everything its predecessor had got to compile. Which branch a task builds on
// is decided here.

const tasks = [
  { id: "4b7d4930-aaaa", assigned_to: "coder", branch: "agent/4b7d4930", created_at: "2026-09-10T07:10:00Z" },
  { id: "4002c138-bbbb", assigned_to: "architect", branch: "agent/4002c138", created_at: "2026-09-10T07:05:00Z" },
  { id: "4ed99f62-cccc", assigned_to: "coder", branch: "agent/4ed99f62", created_at: "2026-09-10T19:56:00Z" },
  { id: "9f000000-dddd", assigned_to: "coder", branch: null, created_at: "2026-09-10T20:30:00Z" },
];

test("a writer continues from the mission's latest writer branch", () => {
  assert.equal(pickBase(tasks, "coder"), "agent/4ed99f62");
  assert.equal(pickBase(tasks, "drivers"), "agent/4ed99f62");
});

test("the Master can name the work to build on", () => {
  // The branch where the build last passed, rather than the latest one.
  assert.equal(pickBase(tasks, "coder", "4b7d4930"), "agent/4b7d4930");
  assert.equal(pickBase(tasks, "coder", "agent/4b7d4930"), "agent/4b7d4930");
});

test("or start clean from main", () => {
  assert.equal(pickBase(tasks, "coder", "main"), null);
});

test("a name that matches nothing falls back to the latest, never to an arbitrary task", () => {
  assert.equal(pickBase(tasks, "coder", "zzzzzzzz"), "agent/4ed99f62");
  assert.equal(pickBase(tasks, "coder", ""), "agent/4ed99f62");
});

test("only writers inherit a branch, and only from writers", () => {
  // Design documents go to main; a review reads the branch it is given.
  assert.equal(pickBase(tasks, "architect"), null);
  assert.equal(pickBase(tasks, "review"), null);
  assert.equal(pickBase([tasks[1]!], "coder"), null);
});
