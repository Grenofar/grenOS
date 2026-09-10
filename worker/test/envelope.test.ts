import { test } from "node:test";
import assert from "node:assert/strict";
import { parseEnvelope, EnvelopeError } from "../src/envelope.ts";

const ok = JSON.stringify({
  task_id: "t_1",
  status: "done",
  summary: "Added serial output",
  actions: [{ type: "write_file", path: "kernel/src/serial.rs", content: "// code" }],
  tokens_used: 1200,
});

test("parses a clean envelope", () => {
  const env = parseEnvelope(ok);
  assert.equal(env.status, "done");
  assert.equal(env.actions.length, 1);
});

test("recovers from a code fence", () => {
  assert.equal(parseEnvelope("```json\n" + ok + "\n```").status, "done");
});

test("recovers from a leading sentence", () => {
  assert.equal(parseEnvelope("Sure, here it is:\n" + ok).status, "done");
});

test("braces inside strings do not confuse the scanner", () => {
  const tricky = JSON.stringify({
    status: "done",
    summary: "rust code with braces",
    actions: [
      {
        type: "write_file",
        path: "kernel/src/main.rs",
        content: 'fn main() { println!("}"); }',
      },
    ],
  });
  const env = parseEnvelope("noise " + tricky + " trailing");
  const action = env.actions[0]!;
  assert.equal(action.type, "write_file");
  assert.ok(action.type === "write_file" && action.content.includes('println!("}")'));
});

test("rejects rather than repairs", () => {
  assert.throws(() => parseEnvelope("not json at all"), EnvelopeError);
  assert.throws(
    () => parseEnvelope('{"status":"finished","summary":"x","actions":[]}'),
    EnvelopeError,
    "un status inventé doit être refusé",
  );
  assert.throws(
    () => parseEnvelope('{"status":"done","actions":[]}'),
    EnvelopeError,
    "summary manquant",
  );
  assert.throws(
    () => parseEnvelope('{"status":"done","summary":"x","actions":[{"type":"rm_rf"}]}'),
    EnvelopeError,
    "type d'action inconnu",
  );
});

test("guards that mirror database constraints", () => {
  // Empty old_str would silently rewrite the whole file.
  assert.throws(
    () =>
      parseEnvelope(
        '{"status":"done","summary":"x","actions":[{"type":"patch_file","path":"a.rs","old_str":"","new_str":"y"}]}',
      ),
    EnvelopeError,
  );

  // A task with no acceptance criteria is not a task.
  assert.throws(
    () =>
      parseEnvelope(
        '{"status":"done","summary":"x","actions":[{"type":"propose_task","assigned_to":"coder","goal":"g","acceptance_criteria":[]}]}',
      ),
    EnvelopeError,
  );
});

test("finalize_mission carries the criteria that will grade the work", () => {
  const env = parseEnvelope(
    JSON.stringify({
      status: "done",
      summary: "C'est parti, je lance la mission.",
      actions: [
        {
          type: "finalize_mission",
          title: "Boot minimal en QEMU",
          description: "Full brief for the Architect.",
          acceptance_criteria: ["cargo build --release succeeds", "serial prints grenOS"],
        },
      ],
    }),
  );

  const action = env.actions[0]!;
  assert.equal(action.type, "finalize_mission");
  assert.ok(action.type === "finalize_mission" && action.acceptance_criteria.length === 2);
});

test("a mission cannot be launched without checkable criteria", () => {
  // Launching a mission nobody can grade is precisely what the intake
  // conversation exists to prevent, so the envelope refuses it outright.
  assert.throws(
    () =>
      parseEnvelope(
        JSON.stringify({
          status: "done",
          summary: "lancée",
          actions: [
            {
              type: "finalize_mission",
              title: "T",
              description: "D",
              acceptance_criteria: [],
            },
          ],
        }),
      ),
    EnvelopeError,
  );

  assert.throws(
    () =>
      parseEnvelope(
        JSON.stringify({
          status: "done",
          summary: "lancée",
          actions: [{ type: "finalize_mission", title: "T", description: "D" }],
        }),
      ),
    EnvelopeError,
    "acceptance_criteria absent",
  );
});

test("finalize_mission names the roadmap step it implements, when there is one", () => {
  const base = { type: "finalize_mission", title: "T", description: "D", acceptance_criteria: ["c"] };
  const parse = (action: object) =>
    parseEnvelope(JSON.stringify({ status: "done", summary: "s", actions: [action] })).actions[0]!;

  const linked = parse({ ...base, roadmap_key: " gdt-idt " });
  assert.ok(linked.type === "finalize_mission" && linked.roadmap_key === "gdt-idt");

  // A mission outside the roadmap is legitimate: no key, no link.
  const free = parse(base);
  assert.ok(free.type === "finalize_mission" && free.roadmap_key === undefined);
  const blank = parse({ ...base, roadmap_key: "  " });
  assert.ok(blank.type === "finalize_mission" && blank.roadmap_key === undefined);
});

test("consult asks to read a document before writing", () => {
  const env = parseEnvelope(
    JSON.stringify({
      status: "needs_input",
      summary: "reading first",
      actions: [{ type: "consult", url: " https://docs.rs/limine/latest/limine/ ", why: "current API" }],
    }),
  );
  const action = env.actions[0]!;
  assert.ok(
    action.type === "consult" &&
      action.url === "https://docs.rs/limine/latest/limine/" &&
      action.why === "current API",
  );

  // The host allowlist is not checked here — a refused URL is answered, not
  // punished — but a consult with no URL at all is not an action.
  assert.throws(
    () => parseEnvelope(JSON.stringify({ status: "needs_input", summary: "s", actions: [{ type: "consult" }] })),
    EnvelopeError,
  );
});

test("propose_task can name the work it builds on", () => {
  const base = { type: "propose_task", assigned_to: "coder", goal: "g", acceptance_criteria: ["c"] };
  const parse = (action: object) =>
    parseEnvelope(JSON.stringify({ status: "done", summary: "s", actions: [action] })).actions[0]!;

  const named = parse({ ...base, continue_from: " 4b7d4930 " });
  assert.ok(named.type === "propose_task" && named.continue_from === "4b7d4930");
  const plain = parse(base);
  assert.ok(plain.type === "propose_task" && plain.continue_from === undefined);
});
