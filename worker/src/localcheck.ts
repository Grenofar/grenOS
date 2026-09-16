import { spawn } from "node:child_process";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { config, log } from "./config.ts";
import type { GitHub } from "./github.ts";

/**
 * The kernel built on this machine before anything is committed.
 *
 * CI takes about eight minutes a run and a task has three attempts; clippy
 * with the pinned toolchain takes seconds once the dependencies are built. So
 * when the worker's host has that toolchain (LOCAL_CHECK=true), an answer that
 * passed pre-flight is compiled and linted exactly as verify.yml does, and the
 * compiler's own messages go back to the model inside the attempt. A panel
 * (executor.ts) also prefers the answers that built.
 *
 * Compiling code a model wrote runs code on this machine, so this declines —
 * and leaves the answer to CI, which runs in a throwaway VM — whenever it
 * could do more than compile:
 *
 *   - a build script, the manifest (dependencies bring build scripts and
 *     macros of their own), the cargo config or the toolchain file that
 *     differs from main, where a human merged it;
 *   - a file pulled in at compile time (include_str!, include_bytes!,
 *     include!, #[path]) or an environment variable read at compile time
 *     beyond the three the kernel uses. Their contents can surface in a
 *     compiler message, and compiler messages go back to the model: that is
 *     how a secret on this disk would leave it.
 *
 * And cargo itself runs with an environment stripped of every secret, in a
 * directory outside the repository, with any secret value that still shows up
 * in its output replaced before the output goes anywhere.
 */

type Change = { path: string; content: string | null };

export interface BuildResult {
  ok: boolean;
  errors: string[];
}

/** Files that decide what a build may run. They must match main exactly. */
export const GUARDED = [
  "kernel/build.rs",
  "kernel/Cargo.toml",
  "kernel/rust-toolchain.toml",
  "kernel/rust-toolchain",
  "kernel/.cargo/config.toml",
  "kernel/.cargo/config",
];

/** The compile-time environment reads the kernel has, and the only ones allowed. */
const KNOWN_ENV = new Set(['env!("CARGO_PKG_VERSION")', 'env!("GRENOS_BUILD")', 'env!("GRENOS_BUILT_AT")']);

/**
 * Why this tree must not be built here, or null when it may be. Pure.
 * `tree` is the kernel as the answer leaves it; `main` holds the guarded files
 * as they are on the default branch (absent ones missing from the map).
 */
export function declineReason(tree: Map<string, string>, main: Map<string, string>): string | null {
  for (const path of GUARDED) {
    if ((tree.get(path) ?? null) !== (main.get(path) ?? null)) {
      return `${path} differs from main`;
    }
  }
  for (const [path, content] of tree) {
    if (!path.endsWith(".rs") || path === "kernel/build.rs") continue;
    if (/\binclude(_str|_bytes)?\s*!/.test(content)) return `${path} pulls a file in at compile time`;
    if (/#\s*!?\s*\[\s*path\s*=/.test(content)) return `${path} uses #[path]`;
    if (/\boption_env\s*!/.test(content)) return `${path} reads the environment at compile time`;
    for (const read of content.matchAll(/\benv\s*!\s*\([^)]*\)/g)) {
      if (!KNOWN_ENV.has(read[0].replace(/\s+/g, ""))) return `${path} reads ${read[0]} at compile time`;
    }
  }
  return null;
}

/** Every occurrence of a secret value replaced. Pure. */
export function redact(text: string, secrets: string[]): string {
  let out = text;
  for (const secret of secrets) {
    if (secret && secret.length >= 8) out = out.split(secret).join("[secret]");
  }
  return out;
}

/**
 * The lines of clippy's short output worth sending to a model: each error with
 * its place, paths as the repository names them, at most 25. Pure.
 */
export function digest(output: string): string[] {
  const lines = output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => /^(src|build\.rs)[\\/].*:\d+:\d+: (error|warning)/.test(line) || /^error(\[E\d+\])?: /.test(line))
    .filter((line) => !/^error: could not compile/.test(line) && !/^error: aborting due to/.test(line))
    .map((line) => `local build (cargo clippy --release -- -D warnings, CI's toolchain): kernel/${line.replace(/\\/g, "/")}`);
  return [...new Set(lines)].slice(0, 25);
}

// One cargo at a time: the target directory is shared, which is what makes
// every build after the first take seconds.
let queue: Promise<unknown> = Promise.resolve();

export async function localCheck(gh: GitHub, readRef: string, changes: Change[]): Promise<BuildResult | null> {
  if (!config.localCheck) return null;
  if (!changes.some((c) => c.path.startsWith("kernel/"))) return null;
  const run = queue.then(() => check(gh, readRef, changes));
  queue = run.catch(() => undefined);
  return run;
}

const ROOT = join(tmpdir(), "grenos-check");

async function check(gh: GitHub, readRef: string, changes: Change[]): Promise<BuildResult | null> {
  try {
    const tree = await kernelFiles(gh, readRef);
    for (const change of changes) {
      if (!change.path.startsWith("kernel/")) continue;
      if (change.content === null) tree.delete(change.path);
      else tree.set(change.path, change.content);
    }
    const defaultBranch = await gh.defaultBranch();
    const main = new Map<string, string>();
    for (const path of GUARDED) {
      const content = await gh.readFile(path, defaultBranch);
      if (content !== null) main.set(path, content);
    }
    const reason = declineReason(tree, main);
    if (reason) {
      log.info(`  build local refusé, laissé à la CI : ${reason}`);
      return null;
    }

    const work = join(ROOT, "work");
    await rm(work, { recursive: true, force: true });
    for (const [path, content] of tree) {
      const target = join(work, path);
      await mkdir(dirname(target), { recursive: true });
      await writeFile(target, content);
    }

    const started = Date.now();
    const result = await cargo(join(work, "kernel"));
    if (!result) return null;
    const output = redact(result.output, secrets());
    const errors = result.code === 0 ? [] : digest(output);
    log.info(`  build local · ${result.code === 0 ? "vert" : `${errors.length} erreur(s)`} en ${Math.round((Date.now() - started) / 1000)} s`);
    if (result.code !== 0 && errors.length === 0) {
      errors.push(`local build failed: ${output.trim().split(/\r?\n/).slice(-6).join(" | ").slice(0, 600)}`);
    }
    return { ok: result.code === 0, errors };
  } catch (err) {
    log.warn(`build local impossible : ${err instanceof Error ? err.message : err}`);
    return null;
  }
}

/** The kernel's source files at a ref: what cargo needs, nothing built. */
async function kernelFiles(gh: GitHub, ref: string): Promise<Map<string, string>> {
  const wanted = (await gh.listTree(ref)).filter(
    ({ path, size }) =>
      path.startsWith("kernel/") &&
      size < 1_000_000 &&
      !/^kernel\/(target|limine|iso_root)\//.test(path) &&
      /\.(rs|toml|ld|conf|sh)$|(^|\/)rust-toolchain$/.test(path),
  );
  const files = new Map<string, string>();
  for (const { path } of wanted) {
    const content = await gh.readFile(path, ref);
    if (content !== null) files.set(path, content);
  }
  return files;
}

function secrets(): string[] {
  return [
    config.supabaseServiceKey,
    config.geminiApiKey,
    config.nvidiaApiKey,
    config.githubToken,
    process.env.SUPABASE_ANON_KEY ?? "",
    process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY ?? "",
  ];
}

/** What cargo and rustup need to run, and nothing else: no secret reaches the build. */
function cleanEnvironment(): NodeJS.ProcessEnv {
  const keep = [
    "PATH", "Path", "PATHEXT", "SystemRoot", "SYSTEMROOT", "windir", "ComSpec",
    "TEMP", "TMP", "HOME", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "HOMEDRIVE", "HOMEPATH",
    "RUSTUP_HOME", "CARGO_HOME",
  ];
  const env: NodeJS.ProcessEnv = {};
  for (const key of keep) {
    if (process.env[key]) env[key] = process.env[key];
  }
  env["CARGO_TARGET_DIR"] = join(ROOT, "target");
  env["CARGO_TERM_COLOR"] = "never";
  return env;
}

function cargo(dir: string): Promise<{ code: number; output: string } | null> {
  return new Promise((resolve) => {
    let output = "";
    let settled = false;
    const child = spawn("cargo", ["clippy", "--release", "--message-format", "short", "--", "-D", "warnings"], {
      cwd: dir,
      env: cleanEnvironment(),
      windowsHide: true,
    });
    const timer = setTimeout(() => {
      child.kill();
      if (!settled) {
        settled = true;
        log.warn("build local : plus de 8 minutes, abandonné");
        resolve(null);
      }
    }, 8 * 60 * 1000);
    child.stdout.on("data", (chunk) => (output += chunk));
    child.stderr.on("data", (chunk) => (output += chunk));
    child.on("error", (err) => {
      clearTimeout(timer);
      if (!settled) {
        settled = true;
        log.warn(`build local : cargo introuvable (${err.message})`);
        resolve(null);
      }
    });
    child.on("close", (code) => {
      clearTimeout(timer);
      if (!settled) {
        settled = true;
        resolve({ code: code ?? 1, output });
      }
    });
  });
}
