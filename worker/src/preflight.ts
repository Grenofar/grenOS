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

    // A menu entry is what Limine boots, and it opens with a line starting
    // with "/" (CONFIG.md). Option names are not case sensitive, so only the
    // missing entry is certain: fe2cc7ed wrote options and no entry at all.
    if (name === "limine.conf" && !/^\s*\/+[^\s/]/m.test(content)) {
      problems.push(
        `${path}: no menu entry, so Limine has nothing to boot. An entry opens with a line starting with "/": \`/grenOS\`, then, indented, \`protocol: limine\` and \`kernel_path: boot():/boot/kernel\` (Limine's CONFIG.md).`,
      );
    }

    // A Limine path is resource(argument):/path (CONFIG.md, Paths); a bare
    // /boot/kernel names nothing. fe2cc7ed wrote `KERNEL_PATH : /boot/kernel.el`.
    const kernelPath = name === "limine.conf" ? content.match(/^\s*kernel_path\s*:\s*(\S*)/im)?.[1] : undefined;
    if (kernelPath !== undefined && !/^[a-z]+\([^)]*\):\//i.test(kernelPath)) {
      problems.push(
        `${path}: kernel_path "${kernelPath}" is not a Limine path. A path is resource(argument):/path — e.g. \`kernel_path: boot():/boot/kernel\` for the partition holding limine.conf (Limine's CONFIG.md, Paths).`,
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
      const outside = asmOutsideUnsafe(content);
      if (outside.length > 0) {
        problems.push(
          `${path}: asm! outside an unsafe block (line ${outside.join(", ")}). Inline assembly is unsafe and rustc refuses it anywhere else (E0133): ` +
            "wrap each one in unsafe { … } with its // SAFETY: comment.",
        );
      }
      const delimiter = unbalancedDelimiter(content);
      if (delimiter) {
        problems.push(
          `${path}: ${delimiter}. rustc stops at the first delimiter that does not match, and every error behind it stays hidden until it is fixed.`,
        );
      }
      const casts = castBeforeLessThan(content);
      if (casts.length > 0) {
        problems.push(
          `${path}: a cast followed by < or << (line ${casts.join(", ")}): rustc reads the < as the start of generic arguments. ` +
            "Put the cast in parentheses: (x as u64) << 16.",
        );
      }
      const binary = binaryAsmLabels(content);
      if (binary.length > 0) {
        problems.push(
          `${path}: an asm! label made only of the digits 0 and 1 (line ${binary.join(", ")}). rustc refuses it (binary_asm_labels, deny by default): ` +
            "in Intel syntax `1f` reads as a binary number. Use another label, e.g. `2:` and `2f`.",
        );
      }

      // What clippy -D warnings rejects once rustc is satisfied: mission 2's
      // IDT had 25 such lines behind its type errors, one CI run each layer.
      for (const { name, lines } of staticMutReferences(content)) {
        problems.push(
          `${path}: a reference to static mut ${name} (line ${lines.join(", ")}): &${name}, &mut ${name}, &${name}.field, or a method that borrows it. ` +
            "rustc's static_mut_refs lint rejects it under clippy -D warnings. " +
            `Take the address instead, core::ptr::addr_of!(${name}) or addr_of_mut!(${name}), and call methods through it: (*core::ptr::addr_of_mut!(${name})).iter_mut().`,
        );
      }
      const fnCasts = fnPointerCasts(content);
      if (fnCasts.length > 0) {
        const names = [...new Set(fnCasts.map((c) => c.name))].join(", ");
        problems.push(
          `${path}: a function cast to an integer other than usize (${names}; line ${fnCasts.map((c) => c.line).join(", ")}): ` +
            "clippy::fn_to_numeric_cast rejects it under -D warnings. Cast to usize first: handler as usize as u64.",
        );
      }
      const literals = literalCasts(content);
      if (literals.length > 0) {
        problems.push(
          `${path}: a number literal cast with as (line ${literals.join(", ")}): clippy::unnecessary_cast rejects it under -D warnings. ` +
            "Give the literal its type as a suffix instead: 0x89_u64, not 0x89 as u64.",
        );
      }
      const unused = unusedParameters(content);
      if (unused.length > 0) {
        problems.push(
          `${path}: parameters never used (${unused.map((u) => `${u.name} of ${u.fn}, line ${u.line}`).join("; ")}): ` +
            "rustc's unused_variables lint rejects them under clippy -D warnings. Use each one, or start its name with _.",
        );
      }
      const exposed = privateInterfaces(path, content);
      if (exposed.length > 0) {
        problems.push(
          `${path}: public functions that take or return a type private to this module (${exposed.map((x) => `${x.fn} with ${x.type}, line ${x.line}`).join("; ")}): ` +
            "rustc's private_interfaces lint rejects them under clippy -D warnings. Make the type pub, or the function private.",
        );
      }

      // What drawing code trips on (mission 3, the desktop): a font drawn row
      // by row, a function that ends on return, a draw call with a parameter
      // per coordinate and colour.
      const loops = indexedLoops(content);
      if (loops.length > 0) {
        problems.push(
          `${path}: loops that index one array by their counter (${loops.map((l) => `${l.array}[${l.index}], line ${l.line}`).join("; ")}): ` +
            "clippy::needless_range_loop rejects them under -D warnings. Iterate instead: for (row, bits) in glyph.iter().enumerate().",
        );
      }
      const returns = needlessReturns(content);
      if (returns.length > 0) {
        problems.push(
          `${path}: a return as the last statement of a function (line ${returns.join(", ")}): clippy::needless_return rejects it under -D warnings. ` +
            "End the function on the value itself, with no return and no semicolon.",
        );
      }
      const crowded = crowdedFunctions(content);
      if (crowded.length > 0) {
        problems.push(
          `${path}: functions with more than 7 parameters, self included (${crowded.map((f) => `${f.fn} has ${f.count}, line ${f.line}`).join("; ")}): ` +
            "clippy::too_many_arguments rejects them under -D warnings. Group parameters in a struct, such as a Rect or a Colour.",
        );
      }
      const nestedIfs = collapsibleIfs(content);
      if (nestedIfs.length > 0) {
        problems.push(
          `${path}: an if whose whole body is another if (line ${nestedIfs.join(", ")}): clippy::collapsible_if rejects it under -D warnings. ` +
            "Join the conditions: if a && b { … }.",
        );
      }
      const patterns = redundantPatterns(content);
      if (patterns.length > 0) {
        problems.push(
          `${path}: if let Some(_), None, Ok(_) or Err(_), which only tests (line ${patterns.join(", ")}): clippy::redundant_pattern_matching rejects it under -D warnings. ` +
            "Call is_some(), is_none(), is_ok() or is_err() instead.",
        );
      }
    }
  }

  problems.push(...crateProblems(changes, branch));
  return problems;
}

/** The module name a file under src/ answers to: `gdt` for gdt.rs and gdt/mod.rs. */
function moduleStem(path: string, prefix: string): string {
  const rel = path.slice(`${prefix}src/`.length);
  return (rel.endsWith("/mod.rs") ? rel.slice(0, -"/mod.rs".length) : rel.slice(0, -".rs".length)).split("/").pop()!;
}

/** Whether cargo compiles a file under src/: a crate root, src/bin/, or a module some `mod` declares. */
function compiled(path: string, prefix: string, files: Map<string, string>): boolean {
  const rel = path.slice(`${prefix}src/`.length);
  if (rel === "main.rs" || rel === "lib.rs" || rel.startsWith("bin/")) return true;
  const stem = moduleStem(path, prefix);
  const declaration = new RegExp(`\\bmod\\s+(?:r#)?${stem}\\s*[;{]`);
  return [...files].some(
    ([other, content]) =>
      other !== path &&
      other.startsWith(`${prefix}src/`) &&
      other.endsWith(".rs") &&
      (declaration.test(stripComments(content)) || content.includes(`${stem}.rs"`)),
  );
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

    // A module file that no `mod` declares is never compiled, and CI turns
    // green without it. Mission 2's first GDT task wrote kernel/src/gdt.rs and
    // no `mod gdt;`: the kernel CI booted was the one before, and neither the
    // GDT nor the bugs in that file ever reached it.
    for (const { path } of changes) {
      if (!path.startsWith(`${prefix}src/`) || !path.endsWith(".rs") || !files.has(path)) continue;
      if (!compiled(path, prefix, files)) {
        const stem = moduleStem(path, prefix);
        problems.push(
          `${path}: no \`mod ${stem};\` declares this file, so cargo never compiles it and CI would pass without it. ` +
            `Declare it in ${prefix}src/main.rs (or in its parent module) and call what it provides.`,
        );
      }
    }

    // The build judges the whole crate, not only what the answer changed.
    // Mission 2's last GDT/IDT attempt rewrote idt.rs and serial.rs; nine
    // clippy errors in gdt.rs, untouched, waited behind a type error, and
    // nothing had ever reported them. A file nothing compiles is left alone.
    const changed = new Set(changes.map((c) => c.path));
    const untouched = branch.filter(
      (c) =>
        !changed.has(c.path) &&
        c.path.startsWith(`${prefix}src/`) &&
        c.path.endsWith(".rs") &&
        compiled(c.path, prefix, files),
    );
    for (const problem of preflight(untouched, null)) {
      problems.push(
        `${problem} (You did not change this file, but the build judges the whole crate: fix it, or answer spec_gap if it is outside your allowed paths.)`,
      );
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

/**
 * Lines holding inline assembly outside any unsafe context. `asm!` is unsafe
 * (E0133), and a snippet without its `unsafe` block is the easiest thing to
 * copy: mission 1's second plan showed `loop { core::arch::asm!("hlt") }` in a
 * safe function, and the Coder's next attempt reproduced it twice.
 *
 * A brace scanner over the source, comments and literals blanked out. A block
 * is unsafe when it follows `unsafe` or is the body of an `unsafe fn`; other
 * blocks inherit from the one around them, except a `fn` body, which starts
 * afresh. Nothing inside a macro_rules! body is judged: where the macro
 * expands is what counts.
 */
export function asmOutsideUnsafe(source: string): number[] {
  const code = blankLiterals(source);
  const stack: Array<{ unsafe: boolean; macro: boolean }> = [];
  const lines = new Set<number>();
  let boundary = 0;

  for (let i = 0; i < code.length; i++) {
    const c = code[i];
    if (c === "{") {
      const header = code.slice(boundary, i);
      const parent = stack[stack.length - 1] ?? { unsafe: false, macro: false };
      let unsafe = parent.unsafe;
      if (/\bfn\s+[A-Za-z_]/.test(header)) {
        unsafe = /\bunsafe\s+(?:extern\s*(?:"[^"]*")?\s*)?fn\s+[A-Za-z_]/.test(header);
      }
      if (/\bunsafe\s*$/.test(header)) unsafe = true;
      stack.push({ unsafe, macro: parent.macro || /\bmacro_rules!/.test(header) });
      boundary = i + 1;
    } else if (c === "}") {
      stack.pop();
      boundary = i + 1;
    } else if (c === ";") {
      boundary = i + 1;
    } else if (c === "a" && !/\w/.test(code[i - 1] ?? "") && /^asm!\s*[([{]/.test(code.slice(i, i + 12))) {
      const frame = stack[stack.length - 1];
      if (frame && !frame.unsafe && !frame.macro) lines.add(code.slice(0, i).split("\n").length);
    }
  }
  return [...lines];
}

/**
 * The first delimiter that does not match, described; null when they all do.
 * rustc stops there, so every error behind it stays hidden: mission 2's
 * gdt.rs lost two CI runs to `size_of::<[u8; 0x4000>>()`, a bracket closed
 * by a parenthesis, while the rest of the file waited unseen.
 */
export function unbalancedDelimiter(source: string): string | null {
  const code = blankLiterals(source);
  const opener: Record<string, string> = { ")": "(", "]": "[", "}": "{" };
  const stack: Array<{ ch: string; line: number }> = [];
  let line = 1;
  for (const ch of code) {
    if (ch === "\n") line++;
    else if (ch === "(" || ch === "[" || ch === "{") stack.push({ ch, line });
    else if (ch === ")" || ch === "]" || ch === "}") {
      const top = stack.pop();
      if (!top) return `\`${ch}\` on line ${line} closes nothing`;
      if (top.ch !== opener[ch]) {
        return `\`${top.ch}\` opened on line ${top.line} is closed by \`${ch}\` on line ${line}`;
      }
    }
  }
  const last = stack.pop();
  return last ? `\`${last.ch}\` opened on line ${last.line} is never closed` : null;
}

/**
 * Lines where an integer cast is followed by `<` or `<<`: rustc reads the `<`
 * as the start of generic arguments (`x as u64 << 16`). Mission 2's gdt.rs
 * had eight of them behind its bracket error.
 */
export function castBeforeLessThan(source: string): number[] {
  const code = blankLiterals(source);
  const lines = new Set<number>();
  for (const m of code.matchAll(/\bas\s+(?:[iu](?:8|16|32|64|128|size)|f32|f64)\s*<(?!=)/g)) {
    lines.add(code.slice(0, m.index!).split("\n").length);
  }
  return [...lines];
}

// The clippy checks below were each held against nightly-2024-11-15's clippy
// with -D warnings: what they report failed there, and their negative cases
// passed (worker/test/preflight.test.ts keeps both).

const INTEGER = "(?:[iu](?:8|16|32|64|128|size))";

/**
 * A token that starts here, not in the middle of a word or after a field dot:
 * `t.0` and `self.IDT` are fields, but `0..IDT.len()` is a range, whose second
 * dot hid mission 2's last reference to IDT from the first version of this.
 */
const OWN_NAME = "(?<!\\w)(?<!(?<!\\.)\\.)";

/** Array methods that borrow the array, and so the static that holds it. */
const BORROWING_METHODS =
  "len|is_empty|iter|iter_mut|as_ptr|as_mut_ptr|as_slice|as_mut_slice|fill|first|first_mut|last|last_mut|" +
  "get|get_mut|copy_from_slice|clone_from_slice|swap|split_at|split_at_mut|chunks|chunks_mut|windows|contains|clone";

/**
 * References to each `static mut` of the file: `&NAME`, `&mut NAME`,
 * `&NAME.field`, and a borrowing method called on an array static
 * (`GDT.len()`, `IDT.iter_mut()`). rustc's static_mut_refs lint rejects them
 * under clippy -D warnings. A reference to one element, `&NAME[i]`, and a
 * method that takes the value, `COUNT.wrapping_add(1)`, pass.
 */
export function staticMutReferences(source: string): Array<{ name: string; lines: number[] }> {
  const code = blankLiterals(source);
  const found: Array<{ name: string; lines: number[] }> = [];
  for (const decl of code.matchAll(/\bstatic\s+mut\s+([A-Za-z_]\w*)\s*:\s*(\[)?/g)) {
    const name = decl[1]!;
    // `a && NAME` and `a & NAME` are not references: the & must touch the name.
    const patterns = [new RegExp(`(?<![\\w)\\]&])&(?:mut\\s+)?${name}\\b(?!\\s*\\[)`, "g")];
    if (decl[2]) patterns.push(new RegExp(`${OWN_NAME}${name}\\s*\\.\\s*(?:${BORROWING_METHODS})\\s*\\(`, "g"));
    const lines = new Set<number>();
    for (const pattern of patterns) for (const m of code.matchAll(pattern)) lines.add(lineAt(code, m.index!));
    if (lines.size > 0) found.push({ name, lines: [...lines].sort((a, b) => a - b) });
  }
  return found;
}

/**
 * Functions of the file cast to an integer other than usize
 * (`breakpoint_handler as u64`): clippy's fn_to_numeric_cast lints reject
 * them under -D warnings. Only functions declared at the top of the file
 * count, so a method or a local of the same name is not taken for one.
 */
export function fnPointerCasts(source: string): Array<{ name: string; line: number }> {
  const code = blankLiterals(source);
  const names = [
    ...code.matchAll(/^(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:unsafe\s+)?(?:extern\s+(?:"[^"]*"\s+)?)?fn\s+([A-Za-z_]\w*)/gm),
  ].map((m) => m[1]!);
  if (names.length === 0) return [];
  const cast = new RegExp(`${OWN_NAME}(${names.join("|")})\\s+as\\s+(?!usize\\b)${INTEGER}\\b`, "g");
  return [...code.matchAll(cast)].map((m) => ({ name: m[1]!, line: lineAt(code, m.index!) }));
}

/**
 * Lines where a number literal without a suffix is cast (`0x89 as u64`):
 * clippy's unnecessary_cast rejects it under -D warnings, in a const too. A
 * literal with a suffix (`5u32 as u64`) is a real conversion and passes.
 */
export function literalCasts(source: string): number[] {
  const code = blankLiterals(source);
  const literal = new RegExp(
    `${OWN_NAME}(?:0x[0-9a-fA-F_]+|0o[0-7_]+|0b[01_]+|\\d[\\d_]*)\\s+as\\s+(?:${INTEGER}|f32|f64)\\b`,
    "g",
  );
  return [...new Set([...code.matchAll(literal)].map((m) => lineAt(code, m.index!)))];
}

/**
 * Parameters a function body never mentions (`stack_frame` in a handler that
 * only prints a message): rustc's unused_variables lint rejects them under
 * clippy -D warnings. A name that starts with _ is exempt, and so is one the
 * body mentions anywhere, a format string or a comment included.
 */
export function unusedParameters(source: string): Array<{ fn: string; name: string; line: number }> {
  if (/allow\(\s*unused/.test(source)) return [];
  const code = blankLiterals(source);
  const found: Array<{ fn: string; name: string; line: number }> = [];
  for (const m of code.matchAll(/\bfn\s+([A-Za-z_]\w*)[^(;{]*\(/g)) {
    const open = m.index! + m[0].length - 1;
    const close = closingDelimiter(code, open);
    if (close === -1) continue;
    // The body is the first { after the signature, unless a ; ends it first.
    let brace = close + 1;
    while (brace < code.length && code[brace] !== "{" && code[brace] !== ";") brace++;
    if (code[brace] !== "{") continue;
    const end = closingDelimiter(code, brace);
    if (end === -1) continue;
    const body = source.slice(brace, end + 1);
    for (const param of topLevelParts(code.slice(open + 1, close))) {
      const name = param.match(/^\s*(?:mut\s+)?([A-Za-z_]\w*)\s*:(?!:)/)?.[1];
      if (!name || name.startsWith("_") || name === "self") continue;
      if (!new RegExp(`\\b${name}\\b`).test(body)) found.push({ fn: m[1]!, name, line: lineAt(code, m.index!) });
    }
  }
  return found;
}

/**
 * Public functions of a module file that take or return a type private to it
 * (`pub unsafe fn load_gdt(ptr: &DescriptorTablePointer)` beside a plain
 * `struct DescriptorTablePointer`): rustc's private_interfaces lint rejects
 * them under clippy -D warnings. At a crate root a private type is visible to
 * the whole crate, so roots are skipped.
 */
export function privateInterfaces(path: string, source: string): Array<{ fn: string; type: string; line: number }> {
  if (/(^|\/)(main|lib|build)\.rs$/.test(path) || /(^|\/)src\/bin\//.test(path)) return [];
  const code = blankLiterals(source);
  const hidden = [...code.matchAll(/^(?:struct|enum|union)\s+([A-Za-z_]\w*)/gm)].map((m) => m[1]!);
  if (hidden.length === 0) return [];
  const found: Array<{ fn: string; type: string; line: number }> = [];
  const pubFn =
    /^pub(?:\((?:crate|super)\))?\s+(?:const\s+)?(?:unsafe\s+)?(?:extern\s+(?:"[^"]*"\s+)?)?fn\s+([A-Za-z_]\w*)([^{;]*)/gm;
  for (const m of code.matchAll(pubFn)) {
    const signature = m[2]!;
    // A generic parameter of the same name is not the private type.
    const generics = signature.slice(0, Math.max(0, signature.indexOf("(")));
    const type = hidden.find((t) => new RegExp(`\\b${t}\\b`).test(signature) && !new RegExp(`\\b${t}\\b`).test(generics));
    if (type) found.push({ fn: m[1]!, type, line: lineAt(code, m.index!) });
  }
  return found;
}

/**
 * `for i in 0..n` loops whose body indexes one array by `i` alone: clippy's
 * needless_range_loop rejects them under -D warnings, statics included
 * (`FONT[c]`). It lets a loop be when the counter also indexes another
 * array, reaches one through a field or a pointer (`self.buf[i]`,
 * `(*p)[i]`), sits in arithmetic inside the brackets (`g[i + 1]`), or when
 * the body uses the array some other way.
 */
export function indexedLoops(source: string): Array<{ index: string; array: string; line: number }> {
  const code = blankLiterals(source);
  const found: Array<{ index: string; array: string; line: number }> = [];
  for (const m of code.matchAll(/\bfor\s+([A-Za-z_]\w*)\s+in\s+0\s*\.\.=?[^{;]*\{/g)) {
    const index = m[1]!;
    const open = m.index! + m[0].length - 1;
    const close = closingDelimiter(code, open);
    if (close === -1) continue;
    const body = code.slice(open + 1, close);
    const counter = new RegExp(`\\b${index}\\b`);
    const direct = new Set<string>();
    let indirect = false;
    for (let i = body.indexOf("["); i !== -1; i = body.indexOf("[", i + 1)) {
      const end = closingDelimiter(body, i);
      if (end === -1) break;
      const inside = body.slice(i + 1, end);
      if (!counter.test(inside)) continue;
      const name = body.slice(0, i).match(/(?<![\w.)\]])([A-Za-z_]\w*)\s*$/)?.[1];
      if (inside.trim() === index && name) direct.add(name);
      else indirect = true;
    }
    if (indirect || direct.size !== 1) continue;
    const array = [...direct][0]!;
    const uses = body.match(new RegExp(`\\b${array}\\b`, "g"))?.length ?? 0;
    const indexed = body.match(new RegExp(`\\b${array}\\s*\\[`, "g"))?.length ?? 0;
    if (uses === indexed) found.push({ index, array, line: lineAt(code, m.index!) });
  }
  return found;
}

/**
 * Lines of a `return` that ends a function body, `return x;` or `return;`:
 * clippy's needless_return rejects it under -D warnings. An early return
 * followed by a tail value passes.
 */
export function needlessReturns(source: string): number[] {
  const code = blankLiterals(source);
  const lines: number[] = [];
  for (const m of code.matchAll(/\bfn\s+[A-Za-z_]\w*[^(;{]*\(/g)) {
    const open = m.index! + m[0].length - 1;
    const close = closingDelimiter(code, open);
    if (close === -1) continue;
    let brace = close + 1;
    while (brace < code.length && code[brace] !== "{" && code[brace] !== ";") brace++;
    if (code[brace] !== "{") continue;
    const end = closingDelimiter(code, brace);
    if (end === -1) continue;
    // The last statement at the top level of the body.
    const body = code.slice(brace + 1, end).trimEnd();
    let depth = 0;
    let start = 0;
    for (let i = 0; i < body.length; i++) {
      const c = body[i]!;
      if ("([{".includes(c)) depth++;
      else if (")]}".includes(c)) {
        depth--;
        if (depth === 0 && c === "}") start = i + 1;
      } else if (c === ";" && depth === 0 && i < body.length - 1) start = i + 1;
    }
    const last = body.slice(start).trim();
    if (/^return\b/.test(last)) lines.push(lineAt(code, brace + 1 + body.lastIndexOf(last)));
  }
  return lines;
}

/** Functions with more than 7 parameters, self included: clippy::too_many_arguments. */
export function crowdedFunctions(source: string): Array<{ fn: string; count: number; line: number }> {
  if (/too_many_arguments/.test(source)) return [];
  const code = blankLiterals(source);
  const found: Array<{ fn: string; count: number; line: number }> = [];
  for (const m of code.matchAll(/\bfn\s+([A-Za-z_]\w*)[^(;{]*\(/g)) {
    const open = m.index! + m[0].length - 1;
    const close = closingDelimiter(code, open);
    if (close === -1) continue;
    const count = topLevelParts(code.slice(open + 1, close)).filter((p) => p.trim() !== "").length;
    if (count > 7) found.push({ fn: m[1]!, count, line: lineAt(code, m.index!) });
  }
  return found;
}

/**
 * Lines of an `if` whose whole body is another `if`, neither with an else:
 * clippy's collapsible_if rejects it under -D warnings, an `else if` branch
 * included. `if let`, an else on either, another statement, or a comment
 * between the two keeps them apart, as clippy does.
 */
export function collapsibleIfs(source: string): number[] {
  const code = blankLiterals(source);
  const lines: number[] = [];
  for (const m of code.matchAll(/\bif\s+(?!let\b)/g)) {
    const open = blockAfterCondition(code, m.index! + m[0].length);
    if (open === -1) continue;
    const close = closingDelimiter(code, open);
    if (close === -1 || /^\s*else\b/.test(code.slice(close + 1))) continue;
    // The raw source: a comment inside the outer block keeps the ifs apart.
    const inner = source.slice(open + 1, close).match(/^\s*if\s+(?!let\b)/);
    if (!inner) continue;
    const innerOpen = blockAfterCondition(code, open + 1 + inner[0].length);
    if (innerOpen === -1 || innerOpen > close) continue;
    const innerClose = closingDelimiter(code, innerOpen);
    if (innerClose === -1 || innerClose > close) continue;
    if (source.slice(innerClose + 1, close).trim() === "") lines.push(lineAt(code, m.index!));
  }
  return lines;
}

/** Lines of `if let` or `while let` on Some(_), None, Ok(_) or Err(_): clippy::redundant_pattern_matching. */
export function redundantPatterns(source: string): number[] {
  const code = blankLiterals(source);
  const test = /\b(?:if|while)\s+let\s+(?:(?:Some|Ok|Err)\s*\(\s*_\s*\)|None)\s*=[^=]/g;
  return [...new Set([...code.matchAll(test)].map((m) => lineAt(code, m.index!)))];
}

/** The `{` opening the block of an if whose condition starts at `from`; -1 if none. */
function blockAfterCondition(code: string, from: number): number {
  let depth = 0;
  for (let i = from; i < code.length; i++) {
    const c = code[i];
    if (c === "(" || c === "[") depth++;
    else if (c === ")" || c === "]") depth--;
    else if (c === "{" && depth === 0) return i;
    else if (c === ";" && depth === 0) return -1;
  }
  return -1;
}

/** The 1-based line of an offset. */
function lineAt(text: string, index: number): number {
  return text.slice(0, index).split("\n").length;
}

/** The offset of the delimiter closing the one at `open`, in blanked code; -1 if none. */
function closingDelimiter(code: string, open: number): number {
  let depth = 0;
  for (let i = open; i < code.length; i++) {
    const c = code[i];
    if (c === "(" || c === "[" || c === "{") depth++;
    else if ((c === ")" || c === "]" || c === "}") && --depth === 0) return i;
  }
  return -1;
}

/** A parameter list split at its top-level commas; `->` closes no generic. */
function topLevelParts(list: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < list.length; i++) {
    const c = list[i]!;
    if ("([{<".includes(c)) depth++;
    else if (")]}".includes(c) || (c === ">" && list[i - 1] !== "-")) depth--;
    else if (c === "," && depth === 0) {
      parts.push(list.slice(start, i));
      start = i + 1;
    }
  }
  parts.push(list.slice(start));
  return parts;
}

/**
 * Lines of asm! templates that use a label made only of the digits 0 and 1.
 * rustc refuses them (the binary_asm_labels lint, deny by default): in Intel
 * syntax `1f` reads as a binary number. Mission 2's plan reloaded CS with
 * `lea {2}, [rip + 1f]` … `1:`, the pattern most examples still show.
 */
export function binaryAsmLabels(source: string): number[] {
  const code = blankLiterals(source);
  const lines = new Set<number>();
  for (const call of code.matchAll(/(?<!\w)asm!\s*\(/g)) {
    // The macro's arguments, up to the parenthesis that closes them; literals
    // are blanked in `code`, so no parenthesis inside a string counts.
    let depth = 0;
    let end = code.length;
    for (let i = call.index! + call[0].length - 1; i < code.length; i++) {
      if (code[i] === "(") depth++;
      else if (code[i] === ")" && --depth === 0) {
        end = i;
        break;
      }
    }
    const args = source.slice(call.index!, end);
    for (const literal of args.matchAll(/"((?:[^"\\]|\\.)*)"/g)) {
      const template = literal[1]!;
      if (/(^|[\s;])[01]+:/.test(template) || /(?<![\w.])[01]+[fb]\b/.test(template)) {
        lines.add(source.slice(0, call.index! + literal.index!).split("\n").length);
      }
    }
  }
  return [...lines];
}

/**
 * The source with comments and string, raw-string and char literals turned
 * into spaces, newlines kept: offsets and line numbers stay the original's,
 * and no brace or keyword inside a literal is read as code.
 */
function blankLiterals(source: string): string {
  const out = source.split("");
  const blank = (from: number, to: number) => {
    for (let k = from; k < to && k < out.length; k++) if (out[k] !== "\n") out[k] = " ";
  };

  let i = 0;
  while (i < source.length) {
    const c = source[i];
    const next = source[i + 1];

    if (c === "/" && next === "/") {
      const end = source.indexOf("\n", i);
      const stop = end === -1 ? source.length : end;
      blank(i, stop);
      i = stop;
    } else if (c === "/" && next === "*") {
      // Rust block comments nest.
      let depth = 1;
      let j = i + 2;
      while (j < source.length && depth > 0) {
        if (source[j] === "/" && source[j + 1] === "*") (depth++, (j += 2));
        else if (source[j] === "*" && source[j + 1] === "/") (depth--, (j += 2));
        else j++;
      }
      blank(i, j);
      i = j;
    } else if ((c === "r" || (c === "b" && next === "r")) && !/\w/.test(source[i - 1] ?? "")) {
      const raw = source.slice(i, i + 300).match(/^b?r(#*)"/);
      if (!raw) {
        i++;
        continue;
      }
      const start = i + raw[0].length;
      const end = source.indexOf(`"${raw[1]}`, start);
      const stop = end === -1 ? source.length : end;
      blank(start, stop);
      i = stop + 1 + raw[1]!.length;
    } else if (c === '"') {
      let j = i + 1;
      while (j < source.length && source[j] !== '"') j += source[j] === "\\" ? 2 : 1;
      blank(i + 1, j);
      i = j + 1;
    } else if (c === "'") {
      // A char literal, or a lifetime or label, which is left alone.
      if (next === "\\") {
        let j = i + 2;
        j = source[j] === "u" && source[j + 1] === "{" ? source.indexOf("}", j) + 1 : j + 1;
        if (source[j] === "'") {
          blank(i + 1, j);
          i = j + 1;
          continue;
        }
      } else if (source[i + 2] === "'") {
        blank(i + 1, i + 2);
        i += 3;
        continue;
      }
      i++;
    } else {
      i++;
    }
  }
  return out.join("");
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

/** What the agent reads when its answer could not be parsed at all. */
export function renderUnreadable(reason: string, left: number): string {
  return [
    "# Your answer could not be read",
    "",
    `The runtime could not parse it: ${reason}`,
    "Nothing was committed.",
    "",
    "Return your complete answer again as exactly one JSON object, as the protocol defines it — every action, not only a fix. " +
      'Inside a JSON string, write a quote as \\" and a line break as \\n; no comment, no trailing comma, no code fence.',
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
