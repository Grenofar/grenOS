/**
 * Path authorisation. This is the law referred to throughout agents/README.md:
 * a prompt asks, the sandbox decides.
 *
 * Every write an agent proposes passes through `authorize()` before it reaches
 * GitHub. Nothing here trusts the model — an agent that ignores its
 * instructions and targets `.env.local` is stopped by this file, not by having
 * been told not to.
 */

export type Denial =
  | { ok: true; path: string }
  | { ok: false; path: string; reason: string };

/**
 * Normalise a repo-relative path, rejecting anything that could escape the
 * repository or dodge a glob check.
 */
export function normalizePath(input: string): string | null {
  if (!input) return null;

  const path = input.replace(/\\/g, "/").replace(/^\.\//, "").trim();

  if (path.startsWith("/")) return null; // absolute
  if (/^[A-Za-z]:/.test(path)) return null; // windows drive
  if (path.includes("\0")) return null;

  const segments = path.split("/");
  // `..` is the classic escape, and a bare `.` segment lets the same file be
  // spelled two ways, which would let a second spelling dodge a lease.
  if (segments.some((s) => s === ".." || s === "." || s === "")) return null;

  return segments.join("/");
}

/** Glob supporting `**`, `*` and `?`, anchored to the whole path. */
export function globToRegExp(glob: string): RegExp {
  let out = "";
  let i = 0;

  while (i < glob.length) {
    const rest = glob.slice(i);

    // `**/` matches zero or more directories, so `**/.env*` catches a file at
    // the repository root as well as one nested ten levels down.
    if (rest.startsWith("**/")) {
      out += "(?:[^/]+/)*";
      i += 3;
      continue;
    }
    if (rest.startsWith("**")) {
      out += ".*";
      i += 2;
      continue;
    }

    const ch = glob[i]!;
    if (ch === "*") out += "[^/]*";
    else if (ch === "?") out += "[^/]";
    else out += ch.replace(/[.+^${}()|[\]\\]/g, "\\$&");
    i += 1;
  }

  return new RegExp(`^${out}$`);
}

export function matchesAny(path: string, globs: string[]): boolean {
  return globs.some((g) => globToRegExp(g).test(path));
}

/**
 * Decide whether `rawPath` may be written by an agent with these rules.
 *
 * Deny wins over allow, always. An overlap between the two lists is not an
 * ambiguity to resolve cleverly — it is a signal that the safest reading is
 * the restrictive one.
 */
export function authorize(
  rawPath: string,
  rules: { allowedPaths: string[]; forbiddenPaths: string[]; canWrite: boolean },
): Denial {
  const path = normalizePath(rawPath);
  if (!path) {
    return {
      ok: false,
      path: rawPath,
      reason: `Chemin invalide ou tentative de sortie du dépôt : "${rawPath}"`,
    };
  }

  if (!rules.canWrite) {
    return { ok: false, path, reason: "Cet agent n'a aucun droit d'écriture" };
  }

  if (matchesAny(path, rules.forbiddenPaths)) {
    return {
      ok: false,
      path,
      reason: `Chemin interdit à cet agent : ${path}`,
    };
  }

  if (!matchesAny(path, rules.allowedPaths)) {
    return {
      ok: false,
      path,
      reason:
        `Hors du périmètre autorisé : ${path}. ` +
        `Autorisé : ${rules.allowedPaths.join(", ") || "(rien)"}`,
    };
  }

  return { ok: true, path };
}

/**
 * Do two path sets overlap? The Master uses this to avoid dispatching two
 * tasks that would fight over the same file, before the lease table has to
 * reject one of them.
 */
export function pathsOverlap(a: string[], b: string[]): boolean {
  for (const left of a) {
    for (const right of b) {
      if (globToRegExp(left).test(right) || globToRegExp(right).test(left)) {
        return true;
      }
      // Two globs can overlap without either matching the other literally
      // (`kernel/src/**` vs `kernel/**/mm.rs`). Compare their fixed prefixes.
      const pa = literalPrefix(left);
      const pb = literalPrefix(right);
      if (pa.startsWith(pb) || pb.startsWith(pa)) return true;
    }
  }
  return false;
}

function literalPrefix(glob: string): string {
  const star = glob.indexOf("*");
  const head = star === -1 ? glob : glob.slice(0, star);
  return head.slice(0, head.lastIndexOf("/") + 1);
}
