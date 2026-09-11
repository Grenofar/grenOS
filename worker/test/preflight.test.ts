import { test } from "node:test";
import assert from "node:assert/strict";
import { preflight, renderPreflight, pinnedBeforeRust185 } from "../src/preflight.ts";

// Every rule here was paid for with a real CI run on mission 1. The first test
// matters as much as the others: a check that cries wolf teaches agents to
// ignore the list.

const w = (path: string, content: string | null = "x") => ({ path, content });

test("a sound kernel skeleton passes untouched", () => {
  const main = [
    "#![no_std]",
    "#![no_main]",
    "",
    "#[panic_handler]",
    "fn panic(_info: &core::panic::PanicInfo) -> ! {",
    "    halt()",
    "}",
    "",
    "fn halt() -> ! {",
    "    loop {",
    '        unsafe { core::arch::asm!("hlt") }',
    "    }",
    "}",
    "",
  ].join("\n");
  assert.deepEqual(
    preflight(
      [
        w("kernel/Cargo.toml", '[package]\nname = "kernel"\nversion = "0.1.0"\nedition = "2021"\n'),
        w(
          "kernel/rust-toolchain.toml",
          '[toolchain]\nchannel = "nightly-2026-09-01"\ncomponents = ["rust-src", "clippy"]\ntargets = ["x86_64-unknown-none"]\n',
        ),
        w("kernel/.cargo/config.toml", '[build]\ntarget = "x86_64-unknown-none"\n'),
        w("kernel/src/main.rs", main),
        w("kernel/scripts/make-iso.sh", "#!/bin/sh\nset -e\nxorriso -as mkisofs \"$1\"\n"),
        w("kernel/build.sh"),
        w("kernel/linker.lds"),
        w("kernel/x86_64-grenos.json", null),
      ],
      [],
    ),
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

test("functions core::arch::x86_64 does not have", () => {
  // Mission 1's plan, copied by the Coder: a halt loop and a serial driver
  // written against functions that do not exist.
  const main = '#![no_std]\npub extern "C" fn _start() -> ! {\n    loop { unsafe { core::arch::x86_64::hlt(); } }\n}\n';
  assert.match(preflight([w("kernel/src/main.rs", main)])[0]!, /core::arch::x86_64 has no hlt/);

  const serial =
    "use core::arch::x86_64;\n" +
    "unsafe fn outb(port: u16, val: u8) { x86_64::outb(port, val); }\n" +
    "unsafe fn inb(port: u16) -> u8 { x86_64::inb(port) }\n";
  assert.match(preflight([w("kernel/src/serial.rs", serial)])[0]!, /has no outb, inb:/);

  assert.match(preflight([w("kernel/src/io.rs", "use core::arch::x86_64::{_rdtsc, outb};\n")])[0]!, /has no outb:/);

  // Real intrinsics, the x86_64 crate's own paths and inline assembly pass.
  const fine = [
    "use core::arch::asm;",
    "use core::arch::x86_64::{__cpuid, _rdtsc};",
    "fn t() -> u64 { unsafe { core::arch::x86_64::_rdtsc() } }",
    "fn h() { x86_64::instructions::hlt(); }",
    'fn o(port: u16, byte: u8) { unsafe { asm!("out dx, al", in("dx") port, in("al") byte) } }',
    "fn hlt_loop() -> ! { loop { unsafe { asm!(\"hlt\") } } }",
  ].join("\n");
  assert.deepEqual(preflight([w("kernel/src/io.rs", fine)]), []);
});

test("a no_std binary without a panic handler anywhere in the crate", () => {
  const main =
    '#![no_std]\n#![no_main]\nmod serial;\n\n#[unsafe(no_mangle)]\nextern "C" fn kmain() -> ! {\n    loop { unsafe { core::arch::asm!("hlt") } }\n}\n';
  const handler =
    '#[panic_handler]\nfn panic(_info: &core::panic::PanicInfo) -> ! {\n    loop { unsafe { core::arch::asm!("hlt") } }\n}\n';

  // A known branch holding nothing else: a new crate, and no handler in it.
  assert.match(preflight([w("kernel/src/main.rs", main)], [])[0]!, /must define a #\[panic_handler\]/);

  // A handler elsewhere in the crate, on the branch or in the answer, is enough.
  assert.deepEqual(preflight([w("kernel/src/main.rs", main)], [w("kernel/src/panic.rs", handler)]), []);
  assert.deepEqual(preflight([w("kernel/src/main.rs", main), w("kernel/src/panic.rs", handler)], []), []);
  // So is a crate that provides one.
  const withCrate = '[package]\nname = "k"\n\n[dependencies]\npanic-halt = "1"\n';
  assert.deepEqual(preflight([w("kernel/src/main.rs", main)], [w("kernel/Cargo.toml", withCrate)]), []);

  // Deleting the file that held it brings the problem back.
  assert.equal(
    preflight([w("kernel/src/panic.rs", null)], [w("kernel/src/main.rs", main), w("kernel/src/panic.rs", handler)]).length,
    1,
  );

  // A branch that could not be read proves nothing.
  assert.deepEqual(preflight([w("kernel/src/main.rs", main)]), []);
});

test("edition 2024 under a toolchain pinned before Rust 1.85", () => {
  const manifest = '[package]\nname = "kernel"\nversion = "0.1.0"\nedition = "2024"\n';
  const pin = (channel: string) =>
    w("kernel/rust-toolchain.toml", `[toolchain]\nchannel = "${channel}"\ntargets = ["x86_64-unknown-none"]\n`);

  // Mission 1's pin, read from the branch while the answer changes the manifest.
  assert.match(
    preflight([w("kernel/Cargo.toml", manifest)], [pin("nightly-2024-11-15")])[0]!,
    /edition = "2024" needs Rust 1\.85, and kernel\/rust-toolchain\.toml pins nightly-2024-11-15/,
  );
  // nightly-2024-11-22 is still 1.84; nightly-2024-11-23 is 1.85.
  assert.equal(preflight([w("kernel/Cargo.toml", manifest)], [pin("nightly-2024-11-22")]).length, 1);
  assert.deepEqual(preflight([w("kernel/Cargo.toml", manifest)], [pin("nightly-2024-11-23")]), []);
  assert.deepEqual(preflight([w("kernel/Cargo.toml", manifest), pin("nightly-2026-09-01")], []), []);
  assert.equal(preflight([w("kernel/Cargo.toml", manifest)], [pin("1.84.1")]).length, 1);
  assert.deepEqual(preflight([w("kernel/Cargo.toml", manifest)], [pin("nightly")]), []);

  assert.equal(pinnedBeforeRust185('channel = "nightly-2024-11-15"'), "nightly-2024-11-15");
  assert.equal(pinnedBeforeRust185('channel = "1.85.0"'), null);
  assert.equal(pinnedBeforeRust185('channel = "stable"'), null);
});

test("the agent is told what is left of its corrections", () => {
  assert.match(renderPreflight(["a"], 1), /1 correction\(s\) remain/);
  assert.match(renderPreflight(["a"], 0), /last correction/);
  assert.match(renderPreflight(["a", "b"], 1), /- a\n- b/);
});
