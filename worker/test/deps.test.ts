import { test } from "node:test";
import assert from "node:assert/strict";
import { checkDependencies, checkEditions, requirements, satisfies } from "../src/deps.ts";

// Mission 1 pinned limine = "0.11" when the newest release was 0.6.5, and
// cargo gave up before compiling a line. These rules decide, before any
// commit, whether a requirement can be met by anything published.

test("cargo's caret, tilde and exact requirements", () => {
  assert.equal(satisfies("0.6.5", "0.6"), true);
  assert.equal(satisfies("0.6.5", "0.11"), false);
  assert.equal(satisfies("0.7.0", "0.6"), false); // ^0.6 stops before 0.7
  assert.equal(satisfies("1.9.0", "1.2"), true); // ^1.2 reaches up to 2.0
  assert.equal(satisfies("2.0.0", "1.2"), false);
  assert.equal(satisfies("0.0.3", "0.0.3"), true);
  assert.equal(satisfies("0.0.4", "0.0.3"), false); // ^0.0.3 is exact
  assert.equal(satisfies("1.2.9", "~1.2"), true);
  assert.equal(satisfies("1.3.0", "~1.2"), false);
  assert.equal(satisfies("0.6.5", "=0.6.5"), true);
  assert.equal(satisfies("0.6.4", "=0.6.5"), false);
  assert.equal(satisfies("1.0.0-beta.1", "1"), false); // pre-releases are opt-in
  assert.equal(satisfies("1.0.0", "*"), true);
  // Not modelled here, so not judged here.
  assert.equal(satisfies("1.0.0", ">=1, <2"), undefined);
});

test("the registry requirements of a manifest, and only those", () => {
  const manifest = [
    "[package]",
    'name = "kernel"',
    'version = "0.1.0"',
    "",
    "[dependencies]",
    'limine = "0.11"',
    'spin = { version = "0.9", default-features = false }',
    'local = { path = "../local" }',
    'fork = { git = "https://example.org/fork" }',
    'alias = { package = "bitflags", version = "2" }',
    "",
    "[target.'cfg(target_os = \"none\")'.dependencies]",
    'uart = "0.1"',
    "",
    "[profile.release]",
    'panic = "abort"',
  ].join("\n");

  assert.deepEqual(requirements(manifest), [
    { name: "limine", req: "0.11" },
    { name: "spin", req: "0.9" },
    { name: "bitflags", req: "2" },
    { name: "uart", req: "0.1" },
  ]);
});

test("an invented version or crate comes back with what exists", async () => {
  const lookup = async (name: string) => {
    if (name === "limine") return ["0.6.5", "0.6.4", "0.5.0", "0.1.12"];
    if (name === "spin") return ["0.10.0", "0.9.8"];
    if (name === "nope") return null;
    return undefined; // crates.io did not answer: not our call
  };
  const manifest = '[package]\nname = "k"\n\n[dependencies]\nlimine = "0.11"\nspin = "0.9"\nnope = "1"\nunknown = "1"\n';
  const problems = await checkDependencies([{ path: "kernel/Cargo.toml", content: manifest }], lookup);

  assert.equal(problems.length, 2);
  assert.match(problems[0]!, /limine matches "0\.11" — the newest is 0\.6\.5/);
  assert.match(problems[1]!, /"nope" does not exist/);
});

test("only manifests are read", async () => {
  const lookup = async () => null;
  assert.deepEqual(await checkDependencies([{ path: "kernel/src/main.rs", content: 'x = "1"' }], lookup), []);
});

test("an edition-2024 release under a toolchain older than Rust 1.85", async () => {
  // As crates.io lists them on 2026-09-11: limine is edition 2024 from 0.6.
  const releases = async (name: string) =>
    name === "limine"
      ? [
          { num: "0.6.5", edition: "2024" },
          { num: "0.6.4", edition: "2024" },
          { num: "0.5.0", edition: "2021" },
          { num: "0.4.0", edition: "2021" },
        ]
      : undefined;
  const manifest = (req: string) =>
    `[package]\nname = "kernel"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\nlimine = "${req}"\n`;
  const toolchain = (channel: string) => ({
    path: "kernel/rust-toolchain.toml",
    content: `[toolchain]\nchannel = "${channel}"\ntargets = ["x86_64-unknown-none"]\n`,
  });
  const cargo = (req: string) => ({ path: "kernel/Cargo.toml", content: manifest(req) });

  // Mission 1's pin, on the branch, while the answer adds limine 0.6.
  const problems = await checkEditions([cargo("0.6")], [toolchain("nightly-2024-11-15")], releases);
  assert.equal(problems.length, 1);
  assert.match(problems[0]!, /limine 0\.6\.5, which cargo selects for "0\.6", is built with edition 2024/);
  assert.match(problems[0]!, /require limine = "0\.5\.0", the newest release built with edition 2021/);

  // Moving the pin back is judged too, against the manifest on the branch.
  assert.equal((await checkEditions([toolchain("nightly-2024-11-15")], [cargo("0.6")], releases)).length, 1);

  // Edition 2021 under the old pin, or anything under a recent one: fine.
  assert.deepEqual(await checkEditions([cargo("0.5")], [toolchain("nightly-2024-11-15")], releases), []);
  assert.deepEqual(await checkEditions([cargo("0.6")], [toolchain("nightly-2026-09-01")], releases), []);

  // A lock file may select another release, and an unread branch proves nothing.
  const lock = { path: "kernel/Cargo.lock", content: "# locked" };
  assert.deepEqual(await checkEditions([cargo("0.6")], [toolchain("nightly-2024-11-15"), lock], releases), []);
  assert.deepEqual(await checkEditions([cargo("0.6")], null, releases), []);
});
