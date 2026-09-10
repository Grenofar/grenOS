/**
 * Mechanical mistakes, caught before anything is committed (D-025).
 *
 * Each of these used to cost a CI run and an attempt, and each is visible in
 * the files themselves: mission 1 lost attempts to `Cargo.tompl`, to a custom
 * target spec, and to an empty `loop {}` that clippy rejects. The agent now
 * gets the list back within seconds, in the same attempt, and fixes it.
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

export function preflight(changes: Change[]): string[] {
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

    if (name.endsWith(".rs") && /\bloop\s*\{\s*\}/.test(stripComments(content))) {
      problems.push(
        `${path}: an empty \`loop {}\`. CI runs clippy with -D warnings and clippy::empty_loop rejects it: halt inside the loop instead, e.g. with core::arch::asm!("hlt").`,
      );
    }
  }

  return problems;
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
