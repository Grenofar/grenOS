import { authorize, matchesAny } from "./sandbox.ts";
import type { GitHub } from "./github.ts";
import type { AgentDefinition } from "./prompts.ts";

/**
 * What an agent is shown of the repository.
 *
 * Until this existed, agents wrote blind. Nothing ever set context_refs, so the
 * Coder rewrote kernel/Cargo.toml from scratch on every attempt without seeing
 * the one it had written the attempt before, and the Architect answered a
 * review task with "I have no way to read the file". A model cannot patch code
 * it has never been shown — patch_file even requires quoting it exactly.
 *
 * Two sources: the agent's own branch for code (files it may change, then the
 * rest of kernel/ as read-only context), and the design documents on the
 * default branch, where the Architect's plans land. The file list is always
 * complete; contents stop at the budget, in priority order.
 */

export interface RepoFile {
  path: string;
  size: number;
}

const TEXT = /\.(rs|toml|ld|md|cfg|conf|sh|json|ya?ml|txt|asm|s|S|c|h)$|(^|\/)(Makefile|README)$/;

// A model reasoning about a Rust kernel needs the manifest, the toolchain and
// the entry point before anything else — and budgets cut from the end.
const FIRST = [
  /(^|\/)Cargo\.toml$/,
  /(^|\/)rust-toolchain(\.toml)?$/,
  /(^|\/)\.cargo\/config(\.toml)?$/,
  /(^|\/)[^/]+\.ld$/,
  /(^|\/)limine\.(cfg|conf)$/,
  /(^|\/)src\/(main|lib)\.rs$/,
];

const rank = (path: string): number => {
  const i = FIRST.findIndex((re) => re.test(path));
  return i === -1 ? FIRST.length : i;
};

// These describe the agent system, not the OS. For an agent writing a kernel
// they are tokens spent on nothing.
const FACTORY_DOCS = /^docs\/(ARCHITECTURE|DECISIONS|STATE|ROADMAP)\.md$/;

/** Pure: which files are shown, in which order. */
export function selectContext(files: RepoFile[], include: string[], maxBytesPerFile: number): RepoFile[] {
  return files
    .filter(
      (f) =>
        matchesAny(f.path, include) &&
        TEXT.test(f.path) &&
        f.size <= maxBytesPerFile &&
        !FACTORY_DOCS.test(f.path),
    )
    .sort((a, b) => rank(a.path) - rank(b.path) || a.path.localeCompare(b.path));
}

// The kernel outgrew the first budget (60 000 characters, files of 30 KiB at
// most): by 2026-09-16 it was 377 KiB, desktop.rs alone 150, and the files a
// task most needed to patch were the ones never shown — patch_file has to
// quote the file exactly. So a task's writable files always come whole, up to
// 200 KiB each, whatever the budget.
//
// The rest is kept small, because size is latency: with about 50 000 tokens
// per call the same night, DeepSeek V4 Flash timed out at 240 s three times
// out of eleven. The design documents the task names come first, then the
// Master's notebook, then the others, within DOCS_BUDGET_CHARS; small
// read-only files whole within BUDGET_CHARS; every other Rust file is summed
// up by its signatures, within OUTLINE_BUDGET_CHARS.
const BUDGET_CHARS = 50_000;
const MAX_FILE_BYTES = 200_000;
const READ_ONLY_WHOLE_BYTES = 12_000;
const DOCS_BUDGET_CHARS = 45_000;
const OUTLINE_BUDGET_CHARS = 30_000;
// Four backticks: design documents contain their own triple-backtick blocks,
// and a three-backtick fence would end at the first one.
const FENCE = "````";

export async function repoContext(
  gh: GitHub,
  agent: AgentDefinition,
  allowedPaths: string[],
  readRef: string,
  /** The task's goal and criteria: documents they name come first. */
  mentioned = "",
): Promise<string> {
  const base = await gh.defaultBranch();
  const [branchTree, baseTree] = await Promise.all([
    gh.listTree(readRef),
    readRef === base ? Promise.resolve(null) : gh.listTree(base),
  ]);

  const code = selectContext(branchTree, [...allowedPaths, "kernel/**"], MAX_FILE_BYTES);
  const codePaths = new Set(code.map((f) => f.path));
  const docs = rankDocs(
    selectContext(baseTree ?? branchTree, ["docs/**"], MAX_FILE_BYTES).filter((f) => !codePaths.has(f.path)),
    mentioned,
  );

  const writable = (path: string) =>
    authorize(path, {
      allowedPaths,
      forbiddenPaths: agent.forbiddenPaths,
      canWrite: agent.canWrite,
    }).ok;

  // The design documents are read first, within a share of their own: the
  // specifications a task is written against must never be the part that a
  // large source file pushes out of the budget.
  let used = 0;
  const shownDocs: string[] = [];
  for (const f of docs) {
    if (used + f.size > DOCS_BUDGET_CHARS) continue;
    const content = await gh.readFile(f.path, baseTree ? base : readRef);
    if (content === null) continue;
    used += content.length;
    shownDocs.push(`\n## ${f.path}\n${FENCE}\n${content}\n${FENCE}`);
  }

  const parts: string[] = [];

  parts.push(`\n# Repository — what exists on \`${readRef}\`\n`);
  if (code.length === 0) {
    parts.push(
      "Nothing under kernel/ or your writable paths yet: you are creating these files from scratch.",
    );
  } else {
    parts.push(
      "Every relevant file is listed. Contents follow in priority order until the budget " +
        "runs out. Files marked (writable) are yours to change; the rest is read-only " +
        "context. If you need a file that is listed without contents, say so rather than " +
        "guessing what it contains.\n",
    );
    for (const f of code) {
      parts.push(`- ${f.path}${writable(f.path) ? " (writable)" : ""} — ${f.size} B`);
    }
    // Writable files first, whole; then small read-only ones, whole; what is
    // left is outlined.
    const whole = new Set<string>();
    const passes: Array<(f: RepoFile) => boolean> = [
      (f) => writable(f.path),
      (f) => !writable(f.path) && f.size <= READ_ONLY_WHOLE_BYTES,
    ];
    for (const [index, pass] of passes.entries()) {
      for (const f of code.filter(pass)) {
        // Writable files are never left out: a patch needs the file.
        if (index > 0 && used + f.size > BUDGET_CHARS + DOCS_BUDGET_CHARS) continue;
        const content = await gh.readFile(f.path, readRef);
        if (content === null) continue;
        used += content.length;
        whole.add(f.path);
        parts.push(
          `\n## ${f.path}${writable(f.path) ? " (writable)" : ""}\n${FENCE}\n${content}\n${FENCE}`,
        );
      }
    }
    const outlined: string[] = [];
    let outlinedChars = 0;
    for (const f of code) {
      if (whole.has(f.path) || !f.path.endsWith(".rs")) continue;
      const content = await gh.readFile(f.path, readRef);
      if (content === null) continue;
      const summary = outline(content);
      if (outlinedChars + summary.length > OUTLINE_BUDGET_CHARS) continue;
      outlinedChars += summary.length;
      used += summary.length;
      outlined.push(`\n## ${f.path} — outline only (${f.size} B)\n${FENCE}\n${summary}\n${FENCE}`);
    }
    if (outlined.length > 0) {
      parts.push(
        "\n# Outlines\n\nThese files are too large to show whole next to your writable files: " +
          "their items and signatures follow, bodies left out. Do not patch a file shown only " +
          "as an outline; if the task needs one, return failed and name it, so the Master can " +
          "give it to you.",
      );
      parts.push(...outlined);
    }
  }

  if (shownDocs.length > 0) {
    parts.push(`\n# Design documents — \`${baseTree ? base : readRef}\`\n`);
    parts.push(...shownDocs);
  }

  return parts.join("\n");
}

/**
 * A Rust file reduced to what another file can use: module docs, attributes,
 * and the first line of every item — functions, types, traits, constants,
 * impls and modules — with bodies left out. Pure.
 */
export function outline(source: string): string {
  const keep = /^\s*(\/\/!|#\[|(pub(\([^)]*\))?\s+)?(unsafe\s+|const\s+|async\s+|extern\s+"[^"]*"\s+)*(fn|struct|enum|trait|type|const|static|mod|impl|macro_rules!|use)\b)/;
  return source
    .split(/\r?\n/)
    .filter((line) => keep.test(line))
    .map((line) => line.replace(/\s*\{\s*$/, "").replace(/\s+$/, ""))
    .join("\n");
}

/**
 * Design documents in the order they are worth reading for a task. Pure.
 * The ones its goal or criteria name come first, then the Master's notebook
 * (it binds every agent), then the rest in their usual order.
 */
export function rankDocs(docs: RepoFile[], mentioned: string): RepoFile[] {
  const score = (f: RepoFile): number => {
    if (mentioned.includes(f.path) || mentioned.includes(f.path.split("/").pop() ?? "\u0000")) return 0;
    if (f.path === "docs/MASTER.md") return 1;
    return 2;
  };
  return docs
    .map((f, index) => ({ f, index }))
    .sort((a, b) => score(a.f) - score(b.f) || a.index - b.index)
    .map(({ f }) => f);
}
