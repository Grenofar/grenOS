/**
 * Mechanical mistakes, caught before anything is committed (D-025, D-027).
 *
 * Each of these used to cost a CI run and an attempt, and each is visible in
 * the files themselves: mission 1 lost attempts to `Cargo.tompl`, to a custom
 * target spec, to an empty `loop {}` that clippy rejects, and to a serial
 * driver written against functions `core::arch::x86_64` does not have. The
 * agent now gets the list back within seconds, in the same attempt, and fixes
 * it.
 *
 * Only mistakes that are certain. Anything that needs judgement is CI's job,
 * and a false alarm here would teach agents to ignore the list.
 */

type Change = { path: string; content: string | null };

/** Corrections offered per attempt before a pre-flight problem costs it. */
export const PREFLIGHT_ROUNDS = 2;

/** Names a tool looks up exactly; a near miss is silently ignored. */
const EXACT_NAMES = [
  "Cargo.toml",
  "Cargo.lock",
  "config.toml",
  "rust-toolchain.toml",
  "build.rs",
  "main.rs",
  "lib.rs",
  "limine.conf",
  "make-iso.sh",
];

const TARGET = "x86_64-unknown-none";

/**
 * Instructions models call as if `core::arch::x86_64` exported them. It holds
 * CPU intrinsics — `_rdtsc`, `__cpuid`, `ud2` — and none of these: mission 1's
 * plan prescribed `core::arch::x86_64::hlt()` and `outb`/`inb`, and the kernel
 * the Coder wrote from it could never have compiled.
 */
const NOT_IN_CORE_ARCH = [
  "hlt",
  "outb",
  "inb",
  "outw",
  "inw",
  "outl",
  "inl",
  "cli",
  "sti",
  "lgdt",
  "lidt",
  "invlpg",
  "rdmsr",
  "wrmsr",
];

/**
 * Edition 2024 begins with Rust 1.85. nightly-2024-11-22 is the last nightly
 * that still reports 1.84, nightly-2024-11-23 the first that reports 1.85
 * (static.rust-lang.org manifests, read on 2026-09-11).
 */
const LAST_NIGHTLY_BEFORE_1_85 = "2024-11-22";

/** A pin checked to exist with clippy (static.rust-lang.org, 2026-09-11): Rust 1.100. */
export const RECENT_NIGHTLY = "nightly-2026-09-01";

/**
 * @param branch The files of each crate the answer touches that it leaves
 *   alone, as they stand on the branch (executor.ts). `null` when they could
 *   not be read: the checks that need the whole crate are skipped, never
 *   guessed.
 */
export function preflight(changes: Change[], branch: Change[] | null = null): string[] {
  const problems: string[] = [];

  for (const { path, content } of changes) {
    if (content === null) continue;
    const name = path.split("/").pop() ?? path;

    const near = EXACT_NAMES.includes(name)
      ? undefined
      : EXACT_NAMES.find(
          (known) =>
            known.toLowerCase() === name.toLowerCase() ||
            distance(known.toLowerCase(), name.toLowerCase()) <= 1,
        );
    if (near) {
      problems.push(
        `${path}: looks like a misspelling of ${near}. Tools read the exact name only, so this file would be ignored.`,
      );
    }

    if (/(^|\/)\.cargo\/config$/.test(path)) {
      problems.push(`${path}: cargo's configuration file is .cargo/config.toml.`);
    }
    if (name === "limine.cfg") {
      problems.push(
        `${path}: Limine reads limine.conf, which has its own syntax (see Limine's CONFIG.md). A limine.cfg is not read.`,
      );
    }
    // The old limine.cfg keys, which models still write from memory. Limine's
    // CONFIG.md no longer mentions them at all (checked 2026-09-10); the
    // current form is `protocol: limine` and `kernel_path: boot():/...`.
    if (name === "limine.conf" && /^\s*(PROTOCOL|KERNEL_PATH|TIMEOUT)\s*=/m.test(content)) {
      problems.push(
        `${path}: old limine.cfg syntax (KEY=value). limine.conf uses \`timeout: 3\`, then an entry \`/grenOS\` with \`protocol: limine\` and \`kernel_path: boot():/boot/kernel\` — see Limine's CONFIG.md.`,
      );
    }

    // A script that writes the configuration builds an image Limine cannot
    // read: mission 1's first make-iso.sh wrote a limine.cfg from a heredoc.
    if (/\.sh$|(^|\/)(GNU)?[Mm]akefile$/.test(path) && /limine\.cfg\b/.test(content)) {
      problems.push(
        `${path}: produces a limine.cfg. Limine reads limine.conf, in the syntax of its CONFIG.md (\`protocol: limine\`, \`kernel_path: boot():/boot/kernel\`).`,
      );
    }

    if (!content.trim() && name !== ".gitkeep") {
      problems.push(`${path}: the file is empty.`);
    }

    if (name.endsWith(".json")) {
      let parsed: unknown = null;
      try {
        parsed = JSON.parse(content);
      } catch (err) {
        problems.push(`${path}: invalid JSON (${err instanceof Error ? err.message : err}).`);
      }
      if (parsed && typeof parsed === "object" && "llvm-target" in parsed) {
        problems.push(
          `${path}: a custom target spec. CI builds for the built-in ${TARGET} (protocol §6); a custom spec needs unstable flags and build-std, and was refused on mission 1.`,
        );
      }
    }

    if (name === "rust-toolchain.toml" && !content.includes(TARGET)) {
      problems.push(
        `${path}: CI installs only what this file lists. Add targets = ["${TARGET}"], or core is missing for the target.`,
      );
    }

    if (/(^|\/)\.cargo\/config\.toml$/.test(path)) {
      const target = content.match(/^\s*target\s*=\s*"([^"]+)"/m)?.[1];
      if (target && target !== TARGET) {
        problems.push(`${path}: target = "${target}". CI builds for ${TARGET} (protocol §6).`);
      }
    }

    if (name === "Cargo.toml" && !/^\s*\[(package|workspace)\]/m.test(content)) {
      problems.push(`${path}: no [package] section, so cargo cannot build it.`);
    }

    if (name.endsWith(".rs")) {
      const code = stripComments(content);
      if (/\bloop\s*\{\s*\}/.test(code)) {
        problems.push(
          `${path}: an empty \`loop {}\`. CI runs clippy with -D warnings and clippy::empty_loop rejects it: halt inside the loop instead, e.g. with core::arch::asm!("hlt").`,
        );
      }
      const invented = inventedIntrinsics(code);
      if (invented.length > 0) {
        problems.push(
          `${path}: core::arch::x86_64 has no ${invented.join(", ")}: it holds CPU intrinsics such as _rdtsc and __cpuid. ` +
            "Write these as inline assembly, each in an unsafe block with its // SAFETY: comment: " +
            'core::arch::asm!("hlt"), asm!("out dx, al", in("dx") port, in("al") byte), asm!("in al, dx", out("al") byte, in("dx") port).',
        );
      }
    }
  }

  problems.push(...crateProblems(changes, branch));
  return problems;
}

/**
 * Checks that need the whole crate, not only the files the answer changed: a
 * panic handler may live in any file of the crate, and the toolchain that
 * builds a manifest is another file.
 */
function crateProblems(changes: Change[], branch: Change[] | null): string[] {
  if (branch === null) return [];
  const files = effectiveFiles(changes, branch);
  const problems: string[] = [];

  for (const root of crateRoots(changes)) {
    const prefix = root ? `${root}/` : "";

    // Mission 1's last attempt had none: seven errors were waiting behind its
    // syntax error, and this was one of them.
    const main = files.get(`${prefix}src/main.rs`);
    if (main !== undefined) {
      const code = stripComments(main);
      const handled = [...files].some(
        ([path, content]) =>
          path.startsWith(`${prefix}src/`) &&
          path.endsWith(".rs") &&
          /#\[panic_handler\]/.test(stripComments(content)),
      );
      // A crate such as panic-halt provides the handler instead.
      const provided = /^\s*panic[-_][\w-]*\s*=/m.test(files.get(`${prefix}Cargo.toml`) ?? "");
      if (/#!\[no_std\]/.test(code) && /#!\[no_main\]/.test(code) && !handled && !provided) {
        problems.push(
          `${prefix}src/main.rs: a #![no_std] binary must define a #[panic_handler], and no file under ${prefix}src/ does, so cargo refuses to build it. ` +
            'Add one, e.g. #[panic_handler] fn panic(_info: &core::panic::PanicInfo) -> ! { loop { unsafe { core::arch::asm!("hlt") } } }',
        );
      }
    }

    const manifest = files.get(`${prefix}Cargo.toml`);
    const toolchain = files.get(`${prefix}rust-toolchain.toml`);
    const pinned = toolchain ? pinnedBeforeRust185(toolchain) : null;
    if (manifest && pinned && /^\s*edition\s*=\s*"2024"/m.test(manifest)) {
      problems.push(
        `${prefix}Cargo.toml: edition = "2024" needs Rust 1.85, and ${prefix}rust-toolchain.toml pins ${pinned}, which is older. ` +
          `Pin a recent dated nightly (${RECENT_NIGHTLY} exists, with clippy) or use edition = "2021".`,
      );
    }
  }

  return problems;
}

/** The files of a crate as the answer would leave them: its changes over the branch. */
export function effectiveFiles(changes: Change[], branch: Change[]): Map<string, string> {
  const files = new Map<string, string>();
  for (const f of branch) if (f.content !== null) files.set(f.path, f.content);
  for (const c of changes) {
    if (c.content === null) files.delete(c.path);
    else files.set(c.path, c.content);
  }
  return files;
}

/** The crate directories an answer touches: `kernel` for kernel/src/serial.rs. */
export function crateRoots(changes: Array<{ path: string }>): string[] {
  const roots = new Set<string>();
  for (const { path } of changes) {
    const m = path.match(/^(?:(.+?)\/)?(?:src\/.+\.rs|Cargo\.toml|rust-toolchain\.toml)$/);
    if (m) roots.add(m[1] ?? "");
  }
  return [...roots];
}

/**
 * The channel a toolchain file pins when it is certainly older than Rust
 * 1.85; null when it is newer, floating (plain "nightly"), or not certain.
 */
export function pinnedBeforeRust185(toolchain: string): string | null {
  const channel = toolchain.match(/^\s*channel\s*=\s*"([^"]+)"/m)?.[1]?.trim();
  if (!channel) return null;
  const nightly = channel.match(/^nightly-(\d{4}-\d{2}-\d{2})$/);
  if (nightly) return nightly[1]! <= LAST_NIGHTLY_BEFORE_1_85 ? channel : null;
  const stable = channel.match(/^1\.(\d+)(?:\.\d+)?$/);
  if (stable) return Number(stable[1]) < 85 ? channel : null;
  return null;
}

/** Names from NOT_IN_CORE_ARCH that a source reaches through core::arch::x86_64. */
export function inventedIntrinsics(source: string): string[] {
  const names = new Set<string>();
  const list = NOT_IN_CORE_ARCH.join("|");

  for (const m of source.matchAll(new RegExp(`\\bcore::arch::x86_64::(${list})\\b`, "g"))) {
    names.add(m[1]!);
  }
  for (const m of source.matchAll(/\buse\s+core::arch::x86_64::\{([^}]*)\}/g)) {
    for (const item of m[1]!.split(",")) {
      const imported = item.trim().split(/\s+as\s+/)[0]!;
      if (NOT_IN_CORE_ARCH.includes(imported)) names.add(imported);
    }
  }
  // `use core::arch::x86_64;` makes `x86_64::` mean that module in this file.
  if (/\buse\s+core::arch::x86_64\s*;/.test(source)) {
    for (const m of source.matchAll(new RegExp(`(?<![\\w:])x86_64::(${list})\\b`, "g"))) {
      names.add(m[1]!);
    }
  }
  return [...names];
}

/** What the agent reads when its answer is sent back. */
export function renderPreflight(problems: string[], left: number): string {
  return [
    "# Pre-flight: nothing was committed",
    "",
    "Your answer was stopped before the commit. Each of these is certain to fail:",
    "",
    ...problems.map((p) => `- ${p}`),
    "",
    "Fix every one and return your complete answer again — every action, not only the fixes.",
    left > 0
      ? `After this one, ${left} correction(s) remain in this attempt.`
      : "This is the last correction in this attempt: after it, the attempt is spent.",
  ].join("\n");
}

/** Comments do not make a loop non-empty for clippy, so they are removed first. */
function stripComments(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
}

/** Levenshtein distance, for names that are one keystroke away. */
function distance(a: string, b: string): number {
  const row = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    let diagonal = row[0]!;
    row[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const above = row[j]!;
      row[j] = Math.min(above + 1, row[j - 1]! + 1, diagonal + (a[i - 1] === b[j - 1] ? 0 : 1));
      diagonal = above;
    }
  }
  return row[b.length]!;
}
