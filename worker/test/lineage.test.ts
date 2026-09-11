import { test } from "node:test";
import assert from "node:assert/strict";

process.env.GRENOS_ROOT = new URL("../..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
process.env.SUPABASE_URL ??= "https://test.supabase.co";
process.env.SUPABASE_SERVICE_ROLE_KEY ??= "test";
process.env.GEMINI_API_KEY ??= "test";
process.env.GITHUB_TOKEN ??= "test";

const { pickBase, progress } = await import("../src/lineage.ts");

// Mission 1's second Coder task was cut from a main with no kernel, and lost
// everything its predecessor had got to compile; its third would have started
// from a branch that did not even parse. Which branch a task builds on is
// decided here.

const tasks = [
  { id: "4b7d4930-aaaa", assigned_to: "coder", branch: "agent/4b7d4930", created_at: "2026-09-10T07:10:00Z" },
  { id: "4002c138-bbbb", assigned_to: "architect", branch: "agent/4002c138", created_at: "2026-09-10T07:05:00Z" },
  { id: "4ed99f62-cccc", assigned_to: "coder", branch: "agent/4ed99f62", created_at: "2026-09-10T19:56:00Z" },
  { id: "9f000000-dddd", assigned_to: "coder", branch: null, created_at: "2026-09-10T20:30:00Z" },
];

// Mission 1's last runs, as the database holds them: no step list yet, only
// the log sections of the steps that ran.
const runs = [
  {
    branch: "agent/4ed99f62",
    status: "failed",
    verdicts: [],
    log_excerpt: "--- build ---\nerror: expected item after attributes",
    started_at: "2026-09-10T20:16:16Z",
  },
  {
    branch: "agent/4b7d4930",
    status: "failed",
    verdicts: [],
    log_excerpt:
      "--- build ---\n    Finished `release` profile\n--- clippy ---\nerror: empty `loop {}` wastes CPU cycles\n--- qemu ---\nAucune image bootable produite",
    started_at: "2026-09-10T19:29:41Z",
  },
  {
    branch: "agent/4b7d4930",
    status: "failed",
    verdicts: [],
    log_excerpt: "--- build ---\nerror: failed to parse manifest",
    started_at: "2026-09-10T15:07:24Z",
  },
];

test("without a CI run to go by, a writer continues from the latest writer branch", () => {
  assert.equal(pickBase(tasks, "coder"), "agent/4ed99f62");
  assert.equal(pickBase(tasks, "drivers"), "agent/4ed99f62");
});

test("the branch whose last run got furthest wins over the latest", () => {
  // 4b7d4930 built and failed on clippy; 4ed99f62, newer, did not parse.
  assert.equal(pickBase(tasks, "coder", undefined, runs), "agent/4b7d4930");
});

test("a branch is judged by its last run, not by its best", () => {
  const broken = [
    ...runs,
    { branch: "agent/4b7d4930", status: "failed", verdicts: [], log_excerpt: "--- build ---\nerror[E0425]", started_at: "2026-09-10T21:00:00Z" },
  ];
  // Neither builds any more: the most recent wins again.
  assert.equal(pickBase(tasks, "coder", undefined, broken), "agent/4ed99f62");
});

test("the Master can name the work to build on", () => {
  assert.equal(pickBase(tasks, "coder", "4b7d4930"), "agent/4b7d4930");
  assert.equal(pickBase(tasks, "coder", "agent/4b7d4930"), "agent/4b7d4930");
  // Its choice comes before the ranking.
  assert.equal(pickBase(tasks, "coder", "4ed99f62", runs), "agent/4ed99f62");
});

test("or start clean from main", () => {
  assert.equal(pickBase(tasks, "coder", "main", runs), null);
});

test("a name that matches nothing falls back to the default choice, never to an arbitrary task", () => {
  assert.equal(pickBase(tasks, "coder", "zzzzzzzz"), "agent/4ed99f62");
  assert.equal(pickBase(tasks, "coder", ""), "agent/4ed99f62");
  assert.equal(pickBase(tasks, "coder", "zzzzzzzz", runs), "agent/4b7d4930");
});

test("only writers inherit a branch, and only from writers", () => {
  // Design documents go to main; a review reads the branch it is given.
  assert.equal(pickBase(tasks, "architect"), null);
  assert.equal(pickBase(tasks, "review"), null);
  assert.equal(pickBase([tasks[1]!], "coder"), null);
});

test("how far a run got, from the steps CI lists or, before that, from its log", () => {
  const step = (name: string, verdict: string) => ({ step: name, criterion: name, verdict, evidence: "" });
  assert.equal(progress({ status: "passed", verdicts: [], log_excerpt: null }), 4);
  assert.equal(
    progress({ status: "failed", verdicts: [step("build", "PASS"), step("clippy", "PASS"), step("boot", "FAIL")] }),
    3,
  );
  assert.equal(
    progress({ status: "failed", verdicts: [step("build", "PASS"), step("clippy", "FAIL"), step("boot", "FAIL")] }),
    2,
  );
  assert.equal(
    progress({
      status: "failed",
      verdicts: [step("build", "FAIL"), step("clippy", "UNVERIFIABLE"), step("boot", "UNVERIFIABLE")],
    }),
    1,
  );
  assert.equal(progress(runs[1]!), 2);
  assert.equal(progress(runs[0]!), 1);
  // No kernel to judge, or a runner problem: nothing learned about the code.
  assert.equal(progress({ status: "error", verdicts: [], log_excerpt: null }), 0);
});
