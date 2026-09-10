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

const BUDGET_CHARS = 60_000;
const MAX_FILE_BYTES = 30_000;
// Four backticks: design documents contain their own triple-backtick blocks,
// and a three-backtick fence would end at the first one.
const FENCE = "````";

export async function repoContext(
  gh: GitHub,
  agent: AgentDefinition,
  allowedPaths: string[],
  readRef: string,
): Promise<string> {
  const base = await gh.defaultBranch();
  const [branchTree, baseTree] = await Promise.all([
    gh.listTree(readRef),
    readRef === base ? Promise.resolve(null) : gh.listTree(base),
  ]);

  const code = selectContext(branchTree, [...allowedPaths, "kernel/**"], MAX_FILE_BYTES);
  const codePaths = new Set(code.map((f) => f.path));
  const docs = selectContext(baseTree ?? branchTree, ["docs/**"], MAX_FILE_BYTES).filter(
    (f) => !codePaths.has(f.path),
  );

  const writable = (path: string) =>
    authorize(path, {
      allowedPaths,
      forbiddenPaths: agent.forbiddenPaths,
      canWrite: agent.canWrite,
    }).ok;

  let used = 0;
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
    for (const f of code) {
      if (used + f.size > BUDGET_CHARS) continue;
      const content = await gh.readFile(f.path, readRef);
      if (content === null) continue;
      used += content.length;
      parts.push(
        `\n## ${f.path}${writable(f.path) ? " (writable)" : ""}\n${FENCE}\n${content}\n${FENCE}`,
      );
    }
  }

  const shownDocs: string[] = [];
  for (const f of docs) {
    if (used + f.size > BUDGET_CHARS) continue;
    const content = await gh.readFile(f.path, baseTree ? base : readRef);
    if (content === null) continue;
    used += content.length;
    shownDocs.push(`\n## ${f.path}\n${FENCE}\n${content}\n${FENCE}`);
  }
  if (shownDocs.length > 0) {
    parts.push(`\n# Design documents — \`${baseTree ? base : readRef}\`\n`);
    parts.push(...shownDocs);
  }

  return parts.join("\n");
}
