import { test } from "node:test";
import assert from "node:assert/strict";

process.env.GRENOS_ROOT = new URL("../..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
process.env.SUPABASE_URL ??= "https://test.supabase.co";
process.env.SUPABASE_SERVICE_ROLE_KEY ??= "test";
process.env.GEMINI_API_KEY ??= "test";
process.env.GITHUB_TOKEN ??= "test";

const { declineReason, redact, digest } = await import("../src/localcheck.ts");

const main = new Map([
  ["kernel/build.rs", "fn main() {}"],
  ["kernel/Cargo.toml", "[package]"],
]);
const tree = (extra: Record<string, string> = {}) =>
  new Map([
    ["kernel/build.rs", "fn main() {}"],
    ["kernel/Cargo.toml", "[package]"],
    ["kernel/src/main.rs", 'let v = env!("CARGO_PKG_VERSION");'],
    ...Object.entries(extra),
  ]);

test("the kernel as it is may be built here", () => {
  assert.equal(declineReason(tree(), main), null);
});

test("a build script or manifest a model changed is left to CI", () => {
  assert.match(declineReason(tree({ "kernel/build.rs": "fn main() { std::process::Command::new(\"x\"); }" }), main)!, /build\.rs/);
  assert.match(declineReason(tree({ "kernel/Cargo.toml": "[dependencies]\nevil = \"1\"" }), main)!, /Cargo\.toml/);
  assert.match(declineReason(tree({ "kernel/.cargo/config.toml": "[build]" }), main)!, /config\.toml/);
});

test("anything that could print a file of this machine into a compiler message is refused", () => {
  // compile_error!(include_str!("C:/.../.env.local")) would hand the secrets
  // to the model in the error it is sent back.
  assert.ok(declineReason(tree({ "kernel/src/x.rs": 'compile_error!(include_str!("../../.env.local"));' }), main));
  assert.ok(declineReason(tree({ "kernel/src/x.rs": 'static B: &[u8] = include_bytes!("/etc/passwd");' }), main));
  assert.ok(declineReason(tree({ "kernel/src/x.rs": 'include!("../../secret.rs");' }), main));
  assert.ok(declineReason(tree({ "kernel/src/x.rs": '#[path = "../../../worker/src/config.ts"]\nmod c;' }), main));
  assert.ok(declineReason(tree({ "kernel/src/x.rs": 'const K: &str = env!("NVIDIA_API_KEY");' }), main));
  assert.ok(declineReason(tree({ "kernel/src/x.rs": 'const K: Option<&str> = option_env!("GITHUB_TOKEN");' }), main));
});

test("secret values never survive into the output", () => {
  assert.equal(redact("key sk-abcdef123456 leaked twice sk-abcdef123456", ["sk-abcdef123456", ""]), "key [secret] leaked twice [secret]");
});

test("clippy's short output becomes repository paths, errors only", () => {
  const output = [
    "    Checking kernel v0.8.0",
    String.raw`src\update.rs:16:11: error: constant ${"`DOWNLOAD`"} is never used`,
    String.raw`src\chacha.rs:162:22: error: the loop variable ${"`index`"} is only used to index ${"`h`"}`,
    "error: could not compile `kernel` (bin \"kernel\") due to 2 previous errors",
  ].join("\n");
  const lines = digest(output);
  assert.equal(lines.length, 2);
  assert.match(lines[0]!, /kernel\/src\/update\.rs:16:11: error: constant `DOWNLOAD`/);
});
