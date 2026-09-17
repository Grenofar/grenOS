import { test } from "node:test";
import assert from "node:assert/strict";
import { parseEnvelope, splitBlocks, EnvelopeError } from "../src/envelope.ts";

const rust = [
  "//! A file with \"quotes\", a backslash \ and braces { }.",
  "pub fn f() -> &'static str { \"\u{1F600}\" }",
].join("\n");

const answer = [
  JSON.stringify({
    status: "done",
    summary: "wrote it",
    actions: [
      { type: "write_file", path: "kernel/src/x.rs", content_block: "x" },
      { type: "patch_file", path: "kernel/src/main.rs", old_block: "old", new_block: "new" },
    ],
  }),
  "-----BEGIN BLOCK x-----",
  rust,
  "-----END BLOCK x-----",
  "-----BEGIN BLOCK old-----",
  "mod acpi;",
  "-----END BLOCK old-----",
  "-----BEGIN BLOCK new-----",
  "mod acpi;",
  "mod x;",
  "-----END BLOCK new-----",
].join("\n");

test("file contents travel unescaped in blocks after the JSON", () => {
  // 2026-09-16: Nemotron lost three answers of one task to JSON escapes.
  const envelope = parseEnvelope(answer);
  const write = envelope.actions[0];
  assert.ok(write?.type === "write_file");
  assert.equal(write.content, `${rust}\n`);
  const patch = envelope.actions[1];
  assert.ok(patch?.type === "patch_file");
  assert.equal(patch.old_str, "mod acpi;\n");
  assert.equal(patch.new_str, "mod acpi;\nmod x;\n");
});

test("braces in a block placed before the JSON do not confuse the envelope", () => {
  const blocksFirst = [
    "-----BEGIN BLOCK x-----",
    "fn main() { let v = { 1 }; }",
    "-----END BLOCK x-----",
    JSON.stringify({ status: "done", summary: "s", actions: [{ type: "write_file", path: "a.rs", content_block: "x" }] }),
  ].join("\r\n");
  const envelope = parseEnvelope(blocksFirst);
  assert.equal(envelope.actions[0]?.type === "write_file" && envelope.actions[0].content, "fn main() { let v = { 1 }; }\n");
});

test("a missing block is an answer the model can fix, and says how", () => {
  const missing = JSON.stringify({ status: "done", summary: "s", actions: [{ type: "write_file", path: "a.rs", content_block: "nope" }] });
  assert.throws(() => parseEnvelope(missing), (err: unknown) => err instanceof EnvelopeError && /BEGIN BLOCK nope/.test(err.message));
});

test("plain string contents still work, and an unterminated block is left as text", () => {
  const plain = JSON.stringify({ status: "done", summary: "s", actions: [{ type: "write_file", path: "a.rs", content: "x\n" }] });
  assert.equal(parseEnvelope(plain).actions.length, 1);
  const { blocks } = splitBlocks("-----BEGIN BLOCK a-----\nnever closed");
  assert.equal(blocks.size, 0);
  assert.equal(splitBlocks("-----BEGIN BLOCK e-----\n-----END BLOCK e-----").blocks.get("e"), "");
});
