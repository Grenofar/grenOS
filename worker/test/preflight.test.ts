import { test } from "node:test";
import assert from "node:assert/strict";
import { preflight, renderPreflight } from "../src/preflight.ts";

// Every rule here was paid for with a real CI run on mission 1. The first test
// matters as much as the others: a check that cries wolf teaches agents to
// ignore the list.

const w = (path: string, content: string | null = "x") => ({ path, content });

test("a sound kernel skeleton passes untouched", () => {
  assert.deepEqual(
    preflight([
      w("kernel/Cargo.toml", '[package]\nname = "kernel"\nversion = "0.1.0"\nedition = "2021"\n'),
      w(
        "kernel/rust-toolchain.toml",
        '[toolchain]\nchannel = "nightly-2026-09-01"\ncomponents = ["rust-src", "clippy"]\ntargets = ["x86_64-unknown-none"]\n',
      ),
      w("kernel/.cargo/config.toml", '[build]\ntarget = "x86_64-unknown-none"\n'),
      w("kernel/src/main.rs", '#![no_std]\nfn halt() -> ! {\n    loop {\n        unsafe { core::arch::asm!("hlt") }\n    }\n}\n'),
      w("kernel/scripts/make-iso.sh", "#!/bin/sh\nset -e\nxorriso -as mkisofs \"$1\"\n"),
      w("kernel/build.sh"),
      w("kernel/linker.lds"),
      w("kernel/x86_64-grenos.json", null),
    ]),
    [],
  );
});

test("mission 1's Cargo.tompl is caught before it costs a CI run", () => {
  const [problem] = preflight([w("kernel/Cargo.tompl", "[package]")]);
  assert.match(problem!, /misspelling of Cargo\.toml/);
});

test("the case of a file name matters", () => {
  assert.match(preflight([w("kernel/cargo.toml", "[package]")])[0]!, /misspelling of Cargo\.toml/);
});

test("names that tools do not read", () => {
  assert.match(preflight([w("kernel/.cargo/config", "[build]")])[0]!, /config\.toml/);
  assert.match(preflight([w("kernel/limine.cfg", ":grenOS")])[0]!, /limine\.conf/);
  // CI runs kernel/scripts/make-iso.sh and nothing else.
  assert.match(preflight([w("kernel/scripts/make_iso.sh", "#!/bin/sh")])[0]!, /make-iso\.sh/);
});

test("mission 1's custom target spec is refused", () => {
  const spec = JSON.stringify({
    "llvm-target": "x86_64-unknown-none",
    arch: "x86_64",
    "target-pointer-width": "64",
  });
  assert.match(preflight([w("kernel/x86_64-grenos.json", spec)])[0]!, /custom target spec/);
});

test("invalid JSON is reported with the parser's reason", () => {
  assert.match(preflight([w("kernel/x.json", "{ nope")])[0]!, /invalid JSON/);
});

test("a toolchain file that forgets the target", () => {
  assert.match(
    preflight([w("kernel/rust-toolchain.toml", '[toolchain]\nchannel = "nightly-2026-09-01"\n')])[0]!,
    /targets = \["x86_64-unknown-none"\]/,
  );
});

test("a cargo config aiming at another target", () => {
  assert.match(
    preflight([w("kernel/.cargo/config.toml", '[build]\ntarget = "x86_64-grenos"\n')])[0]!,
    /target = "x86_64-grenos"/,
  );
});

test("a manifest cargo cannot build", () => {
  assert.match(preflight([w("kernel/Cargo.toml", "[dependencies]\n")])[0]!, /no \[package\]/);
});

test("mission 1's empty loop, which clippy rejects", () => {
  assert.match(preflight([w("kernel/src/main.rs", "fn f() -> ! { loop {} }")])[0]!, /empty `loop \{\}`/);
  // A comment does not make it any less empty.
  assert.equal(preflight([w("kernel/src/main.rs", "fn f() -> ! { loop { /* wait */ } }")]).length, 1);
});

test("limine.conf in the syntax Limine reads today, not the one models remember", () => {
  const current = "timeout: 3\n\n/grenOS\n    protocol: limine\n    kernel_path: boot():/boot/kernel\n";
  assert.deepEqual(preflight([w("kernel/limine.conf", current)]), []);

  const remembered = "TIMEOUT=3\n:grenOS\nPROTOCOL=limine\nKERNEL_PATH=boot:///kernel.elf\n";
  assert.match(preflight([w("kernel/limine.conf", remembered)])[0]!, /old limine\.cfg syntax/);
});

test("a build script that writes limine.cfg", () => {
  // Mission 1's first make-iso.sh wrote one from a heredoc.
  const script = '#!/bin/sh\ncat > "$ISO_ROOT/boot/limine.cfg" <<EOF\nTIMEOUT 20\nEOF\n';
  assert.match(preflight([w("kernel/scripts/make-iso.sh", script)])[0]!, /produces a limine\.cfg/);
  const good = "#!/bin/sh\ncp limine.conf iso_root/boot/limine/\n";
  assert.deepEqual(preflight([w("kernel/scripts/make-iso.sh", good)]), []);
});

test("an empty file is never what was meant", () => {
  assert.match(preflight([w("kernel/src/serial.rs", "  \n")])[0]!, /empty/);
});

test("the agent is told what is left of its corrections", () => {
  assert.match(renderPreflight(["a"], 1), /1 correction\(s\) remain/);
  assert.match(renderPreflight(["a"], 0), /last correction/);
  assert.match(renderPreflight(["a", "b"], 1), /- a\n- b/);
});
