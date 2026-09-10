import { test } from "node:test";
import assert from "node:assert/strict";
import { selectContext } from "../src/context.ts";

// Agents wrote blind for a whole mission: the Coder rewrote Cargo.toml from
// scratch on every attempt without seeing its previous version. These are the
// rules that decide what they are now shown, and in what order.

const f = (path: string, size = 100) => ({ path, size });

test("the manifest, toolchain and entry point come first, whatever the alphabet says", () => {
  const out = selectContext(
    [
      f("kernel/src/serial.rs"),
      f("kernel/src/main.rs"),
      f("kernel/linker.ld"),
      f("kernel/rust-toolchain.toml"),
      f("kernel/Cargo.toml"),
    ],
    ["kernel/**"],
    30_000,
  ).map((x) => x.path);

  assert.deepEqual(out, [
    "kernel/Cargo.toml",
    "kernel/rust-toolchain.toml",
    "kernel/linker.ld",
    "kernel/src/main.rs",
    "kernel/src/serial.rs",
  ]);
});

test("binaries and oversized files are left out", () => {
  const out = selectContext(
    [f("kernel/limine-bios.sys"), f("kernel/grenos.iso"), f("kernel/src/huge.rs", 90_000), f("kernel/src/main.rs")],
    ["kernel/**"],
    30_000,
  ).map((x) => x.path);

  assert.deepEqual(out, ["kernel/src/main.rs"]);
});

test("factory documents are noise for an agent writing a kernel", () => {
  const out = selectContext(
    [f("docs/ARCHITECTURE.md"), f("docs/DECISIONS.md"), f("docs/STATE.md"), f("docs/ROADMAP.md"), f("docs/PLAN.md")],
    ["docs/**"],
    30_000,
  ).map((x) => x.path);

  // The Architect's plan is exactly what the Coder never saw on mission 1.
  assert.deepEqual(out, ["docs/PLAN.md"]);
});

test("only the included paths are shown", () => {
  const out = selectContext([f("apps/web/app/page.tsx"), f("kernel/src/main.rs")], ["kernel/**"], 30_000);
  assert.deepEqual(
    out.map((x) => x.path),
    ["kernel/src/main.rs"],
  );
});
