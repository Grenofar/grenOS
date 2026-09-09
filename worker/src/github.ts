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

export class GitHub {
  readonly repo: string;
  private readonly token: string;
  private readonly blobCache = new Map<string, string | null>();

  constructor(repo = config.githubRepo, token = config.githubToken) {
    this.repo = repo;
    this.token = token;
  }

  private async call<T>(
    path: string,
    init: RequestInit = {},
    okStatuses: number[] = [200, 201],
  ): Promise<T> {
    const res = await fetch(`${API}${path}`, {
      ...init,
      headers: {
        accept: "application/vnd.github+json",
        authorization: `Bearer ${this.token}`,
        "x-github-api-version": "2022-11-28",
        "content-type": "application/json",
        ...(init.headers ?? {}),
      },
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

  /** Create `branch` off `from` if it does not already exist. */
  async ensureBranch(branch: string, from?: string): Promise<string> {
    const existing = await this.branchSha(branch);
    if (existing) return existing;

    const base = from ?? (await this.defaultBranch());
    const baseSha = await this.branchSha(base);
    if (!baseSha) throw new Error(`Branche de base introuvable : ${base}`);

    await this.call(`/repos/${this.repo}/git/refs`, {
      method: "POST",
      body: JSON.stringify({ ref: `refs/heads/${branch}`, sha: baseSha }),
    });
    log.info(`branche créée ${branch} depuis ${base}`);
    return baseSha;
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
   */
  async commit(opts: {
    branch: string;
    message: string;
    changes: Change[];
  }): Promise<{ sha: string } | null> {
    if (opts.changes.length === 0) return null;

    const headSha = await this.branchSha(opts.branch);
    if (!headSha) throw new Error(`Branche inconnue : ${opts.branch}`);

    const head = await this.call<{ tree: { sha: string } }>(
      `/repos/${this.repo}/git/commits/${headSha}`,
    );

    const tree = await Promise.all(
      opts.changes.map(async (change) => {
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

    await this.call(
      `/repos/${this.repo}/git/refs/heads/${encodeURIComponent(opts.branch)}`,
      { method: "PATCH", body: JSON.stringify({ sha: commit.sha, force: false }) },
    );

    this.blobCache.clear();
    log.info(`commit ${commit.sha.slice(0, 7)} sur ${opts.branch} (${tree.length} fichiers)`);
    return { sha: commit.sha };
  }

  clearCache(): void {
    this.blobCache.clear();
  }
}
