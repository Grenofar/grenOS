import { config, log } from "./config.ts";

/**
 * GitHub access over the REST API, with no local clone.
 *
 * The obvious design would be `git clone` plus shell commands, but the brain
 * runs on a 512 MB disk (D-002) and a Pterodactyl container with no git binary
 * and no root. Committing through the API keeps the worker stateless: it holds
 * no working copy, so it can be killed at any moment and lose nothing.
 *
 * Reads are cached per (path, ref) for the lifetime of a task, because the
 * Coder typically reads a file and then patches it moments later.
 */

const API = "https://api.github.com";

interface Change {
  path: string;
  /** null deletes the file. */
  content: string | null;
}

/** One entry of a tree the Git Data API builds; a null sha deletes the path. */
interface TreeEntry {
  path: string;
  mode: "100644";
  type: "blob";
  sha: string | null;
}

export class GitHub {
  readonly repo: string;
  private readonly token: string;
  private readonly blobCache = new Map<string, string | null>();

  constructor(repo = config.githubRepo, token = config.githubToken) {
    this.repo = repo;
    this.token = token;
  }

  private headers(): Record<string, string> {
    return {
      accept: "application/vnd.github+json",
      authorization: `Bearer ${this.token}`,
      "x-github-api-version": "2022-11-28",
      "content-type": "application/json",
    };
  }

  private async call<T>(
    path: string,
    init: RequestInit = {},
    okStatuses: number[] = [200, 201],
  ): Promise<T> {
    const res = await fetch(`${API}${path}`, {
      ...init,
      headers: { ...this.headers(), ...(init.headers ?? {}) },
    });

    if (!okStatuses.includes(res.status)) {
      const body = (await res.text()).slice(0, 300);
      // 401/403 here almost always means the fine-grained PAT is missing a
      // permission or has expired, so say that rather than echoing raw JSON.
      const hint =
        res.status === 401 || res.status === 403
          ? " — vérifie que le PAT a Contents:write sur ce dépôt et n'a pas expiré"
          : "";
      throw new Error(`GitHub ${init.method ?? "GET"} ${path} → ${res.status}${hint}: ${body}`);
    }

    return res.status === 204 ? (undefined as T) : ((await res.json()) as T);
  }

  /** Head commit SHA of a branch, or null if the branch does not exist. */
  async branchSha(branch: string): Promise<string | null> {
    try {
      const ref = await this.call<{ object: { sha: string } }>(
        `/repos/${this.repo}/git/ref/heads/${encodeURIComponent(branch)}`,
      );
      return ref.object.sha;
    } catch (err) {
      if (err instanceof Error && err.message.includes("→ 404")) return null;
      throw err;
    }
  }

  async defaultBranch(): Promise<string> {
    const repo = await this.call<{ default_branch: string }>(`/repos/${this.repo}`);
    return repo.default_branch;
  }

  /**
   * The ref an agent reads from: its own branch if it exists, then the branch
   * its task builds on (lineage.ts), then the default branch.
   *
   * Deliberately creates nothing. Creating the branch up front — which this
   * used to do — is a push of the default branch's content, and every push to
   * agent/** runs CI. That produced a verdict on a branch holding none of the
   * agent's work, charged to the task as a failed attempt before it had
   * written a line. The branch now comes into existence with its first commit
   * (see commit()), so the first CI run it triggers judges real work.
   */
  async resolveRef(branch: string, base: string | null = null): Promise<string> {
    if (await this.branchSha(branch)) return branch;
    if (base && (await this.branchSha(base))) return base;
    return this.defaultBranch();
  }

  /** Every file at `ref`, with its size. One request, no clone. */
  async listTree(ref: string): Promise<Array<{ path: string; size: number }>> {
    const tree = await this.call<{
      tree: Array<{ path: string; type: string; size?: number }>;
    }>(`/repos/${this.repo}/git/trees/${encodeURIComponent(ref)}?recursive=1`);
    return tree.tree
      .filter((e) => e.type === "blob")
      .map((e) => ({ path: e.path, size: e.size ?? 0 }));
  }

  /** File content at a ref, or null when the file does not exist. */
  async readFile(path: string, ref: string): Promise<string | null> {
    const key = `${ref}:${path}`;
    if (this.blobCache.has(key)) return this.blobCache.get(key)!;

    let content: string | null = null;
    try {
      const file = await this.call<{ content?: string; encoding?: string }>(
        `/repos/${this.repo}/contents/${encodeURI(path)}?ref=${encodeURIComponent(ref)}`,
      );
      content =
        file.content && file.encoding === "base64"
          ? Buffer.from(file.content, "base64").toString("utf8")
          : null;
    } catch (err) {
      if (!(err instanceof Error && err.message.includes("→ 404"))) throw err;
    }

    this.blobCache.set(key, content);
    return content;
  }

  /**
   * Commit a set of changes to a branch in one commit.
   *
   * One commit per task, never one per file: a task is the unit of work that
   * CI verifies and that a human reviews, so it should also be the unit that
   * can be reverted.
   *
   * A missing branch is created by this commit, on the default branch, and
   * carries the changes of `base` when the task builds on another agent
   * branch: born with the agent's content and the work it continues.
   */
  async commit(opts: {
    branch: string;
    message: string;
    changes: Change[];
    base?: string | null;
  }): Promise<{ sha: string } | null> {
    if (opts.changes.length === 0) return null;

    const existing = await this.branchSha(opts.branch);
    let from = opts.branch;
    let headSha = existing;
    // A new branch is born on the default branch; when it continues another
    // agent branch, that branch's own changes are replayed onto it (D-029).
    // GitHub runs the workflow of the pushed commit, and a branch cut from an
    // agent branch of the day before ran that day's CI: no step verdicts, and
    // blind to what make-iso.sh printed.
    let carried: TreeEntry[] = [];
    if (!headSha) {
      from = await this.defaultBranch();
      headSha = await this.branchSha(from);
      if (opts.base && opts.base !== from && (await this.branchSha(opts.base))) {
        carried = await this.changesSince(from, opts.base);
        from = `${from} + ${opts.base}`;
      }
    }
    if (!headSha) throw new Error(`Branche de base introuvable : ${from}`);

    const head = await this.call<{ tree: { sha: string } }>(
      `/repos/${this.repo}/git/commits/${headSha}`,
    );

    const written = await Promise.all(
      opts.changes.map(async (change): Promise<TreeEntry> => {
        if (change.content === null) {
          // A null sha in a tree entry is how the API expresses a deletion.
          return { path: change.path, mode: "100644", type: "blob", sha: null };
        }
        const blob = await this.call<{ sha: string }>(`/repos/${this.repo}/git/blobs`, {
          method: "POST",
          body: JSON.stringify({
            content: Buffer.from(change.content, "utf8").toString("base64"),
            encoding: "base64",
          }),
        });
        return { path: change.path, mode: "100644", type: "blob", sha: blob.sha };
      }),
    );
    // The agent's own version of a file wins over the one it continues.
    const own = new Set(written.map((e) => e.path));
    const tree = [...carried.filter((e) => !own.has(e.path)), ...written];

    const newTree = await this.call<{ sha: string }>(`/repos/${this.repo}/git/trees`, {
      method: "POST",
      body: JSON.stringify({ base_tree: head.tree.sha, tree }),
    });

    const commit = await this.call<{ sha: string }>(`/repos/${this.repo}/git/commits`, {
      method: "POST",
      body: JSON.stringify({
        message: opts.message,
        tree: newTree.sha,
        parents: [headSha],
      }),
    });

    if (existing) {
      await this.call(
        `/repos/${this.repo}/git/refs/heads/${encodeURIComponent(opts.branch)}`,
        { method: "PATCH", body: JSON.stringify({ sha: commit.sha, force: false }) },
      );
    } else {
      await this.call(`/repos/${this.repo}/git/refs`, {
        method: "POST",
        body: JSON.stringify({ ref: `refs/heads/${opts.branch}`, sha: commit.sha }),
      });
      log.info(`branche ${opts.branch} créée depuis ${from}, avec son premier commit`);
    }

    this.blobCache.clear();
    log.info(`commit ${commit.sha.slice(0, 7)} sur ${opts.branch} (${tree.length} fichiers)`);
    return { sha: commit.sha };
  }

  /**
   * The changes `head` made since it left `base`, as tree entries that replay
   * them: added and modified files by their blob, removed and renamed-away
   * ones as deletions of paths `base` still has. Never a workflow file: those
   * are the default branch's to define, and changing one takes a permission
   * the worker's token does not hold.
   */
  private async changesSince(base: string, head: string): Promise<TreeEntry[]> {
    const compare = await this.call<{
      files?: Array<{ filename: string; status: string; sha: string; previous_filename?: string }>;
    }>(`/repos/${this.repo}/compare/${base}...${head}`);
    const present = new Set((await this.listTree(base)).map((f) => f.path));

    const entries: TreeEntry[] = [];
    const remove = (path: string) => {
      if (present.has(path)) entries.push({ path, mode: "100644", type: "blob", sha: null });
    };
    for (const f of compare.files ?? []) {
      if (f.status === "removed") {
        remove(f.filename);
        continue;
      }
      entries.push({ path: f.filename, mode: "100644", type: "blob", sha: f.sha });
      if (f.status === "renamed" && f.previous_filename) remove(f.previous_filename);
    }
    return entries.filter((e) => !e.path.startsWith(".github/"));
  }

  /**
   * Merge a branch into the default branch (merge.ts). GitHub reports the
   * outcome through the status code, so it is returned rather than thrown:
   * a conflict is information for the Master, not a crash of the loop.
   */
  async merge(head: string, message: string): Promise<"merged" | "up_to_date" | "conflict" | "missing"> {
    const base = await this.defaultBranch();
    const res = await fetch(`${API}/repos/${this.repo}/merges`, {
      method: "POST",
      headers: this.headers(),
      body: JSON.stringify({ base, head, commit_message: message }),
    });
    if (res.status === 201) return "merged";
    if (res.status === 204) return "up_to_date";
    if (res.status === 409) return "conflict";
    if (res.status === 404) return "missing";
    throw new Error(`GitHub merge ${head} → ${res.status}: ${(await res.text()).slice(0, 300)}`);
  }

  clearCache(): void {
    this.blobCache.clear();
  }
}
