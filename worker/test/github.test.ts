import { test, beforeEach } from "node:test";
import assert from "node:assert/strict";

process.env.GRENOS_ROOT = new URL("../..", import.meta.url).pathname.replace(
  /^\/([A-Za-z]:)/,
  "$1",
);
process.env.SUPABASE_URL ??= "https://test.supabase.co";
process.env.SUPABASE_SERVICE_ROLE_KEY ??= "test";
process.env.GEMINI_API_KEY ??= "test";
process.env.GITHUB_TOKEN ??= "test";

const { GitHub } = await import("../src/github.ts");

/**
 * Every push to agent/** runs CI, so *when* a branch comes into existence is
 * part of the contract with CI. Creating it before the agent's first commit
 * pushed the default branch's content, CI judged a branch holding none of the
 * agent's work, and the verdict was charged to the task as a failed attempt.
 */

interface Call {
  method: string;
  path: string;
  body: { ref?: string; sha?: string; parents?: string[] } | null;
}

const realFetch = globalThis.fetch;

function stub(route: (c: Call) => { status: number; json?: unknown }): Call[] {
  const calls: Call[] = [];
  globalThis.fetch = (async (url: string | URL | Request, init?: RequestInit) => {
    const c: Call = {
      method: init?.method ?? "GET",
      path: String(url).replace("https://api.github.com", ""),
      body: init?.body ? JSON.parse(String(init.body)) : null,
    };
    calls.push(c);
    const r = route(c);
    return new Response(r.json === undefined ? "" : JSON.stringify(r.json), { status: r.status });
  }) as typeof fetch;
  return calls;
}

beforeEach(() => {
  globalThis.fetch = realFetch;
});

const repoWhere = (branchExists: boolean) => (c: Call) => {
  if (c.method === "GET" && c.path.includes("/git/ref/heads/agent")) {
    return branchExists
      ? { status: 200, json: { object: { sha: "branchhead" } } }
      : { status: 404, json: { message: "Not Found" } };
  }
  if (c.method === "GET" && c.path === "/repos/o/r") return { status: 200, json: { default_branch: "main" } };
  if (c.method === "GET" && c.path.includes("/git/ref/heads/main")) {
    return { status: 200, json: { object: { sha: "mainhead" } } };
  }
  if (c.method === "GET" && c.path.includes("/git/commits/")) return { status: 200, json: { tree: { sha: "tree0" } } };
  if (c.method === "POST" && c.path.endsWith("/git/blobs")) return { status: 201, json: { sha: "blob1" } };
  if (c.method === "POST" && c.path.endsWith("/git/trees")) return { status: 201, json: { sha: "tree1" } };
  if (c.method === "POST" && c.path.endsWith("/git/commits")) return { status: 201, json: { sha: "commit1" } };
  if (c.method === "POST" && c.path.endsWith("/git/refs")) return { status: 201, json: {} };
  if (c.method === "PATCH") return { status: 200, json: {} };
  return { status: 500, json: { message: `route non prévue ${c.method} ${c.path}` } };
};

test("resolveRef reads from the default branch and creates nothing", async () => {
  const calls = stub(repoWhere(false));
  const ref = await new GitHub("o/r", "t").resolveRef("agent/abc12345");

  assert.equal(ref, "main");
  assert.ok(
    calls.every((c) => c.method === "GET"),
    "aucune écriture : créer la branche ici, c'est pousser main et déclencher un verdict fantôme",
  );
});

test("a new branch is born with its first commit, not as a copy of main", async () => {
  const calls = stub(repoWhere(false));
  const out = await new GitHub("o/r", "t").commit({
    branch: "agent/abc12345",
    message: "coder: add manifest",
    changes: [{ path: "kernel/Cargo.toml", content: "[package]" }],
  });

  assert.equal(out?.sha, "commit1");

  const create = calls.find((c) => c.method === "POST" && c.path.endsWith("/git/refs"));
  assert.ok(create, "la branche doit être créée");
  assert.equal(create.body?.ref, "refs/heads/agent/abc12345");
  assert.equal(create.body?.sha, "commit1", "elle pointe sur le travail de l'agent, pas sur main");

  const iCommit = calls.findIndex((c) => c.method === "POST" && c.path.endsWith("/git/commits"));
  assert.ok(iCommit < calls.indexOf(create), "le commit existe avant la branche");
  assert.equal(calls[iCommit]!.body?.parents?.[0], "mainhead", "le premier commit a main pour parent");
  assert.ok(!calls.some((c) => c.method === "PATCH"));
});

test("a branch that continues another agent branch is born on main, carrying that branch's changes", async () => {
  // Mission 1's branches descended from a commit of 2026-09-10 and ran its
  // CI: a branch now runs the workflow main has today.
  const calls = stub((c) => {
    if (c.method === "GET" && c.path.includes("/git/ref/heads/agent%2Fbase0000")) {
      return { status: 200, json: { object: { sha: "basehead" } } };
    }
    if (c.method === "GET" && c.path === "/repos/o/r/compare/main...agent/base0000") {
      return {
        status: 200,
        json: {
          files: [
            { filename: "kernel/Cargo.toml", status: "added", sha: "b-cargo" },
            { filename: "kernel/src/main.rs", status: "modified", sha: "b-main" },
            { filename: "kernel/old.rs", status: "removed", sha: "b-old" },
            { filename: "kernel/gone.rs", status: "removed", sha: "b-gone" },
            { filename: "kernel/new.rs", status: "renamed", sha: "b-new", previous_filename: "kernel/prev.rs" },
            { filename: ".github/workflows/verify.yml", status: "modified", sha: "b-wf" },
          ],
        },
      };
    }
    if (c.method === "GET" && c.path.startsWith("/repos/o/r/git/trees/main")) {
      return {
        status: 200,
        json: { tree: [{ path: "kernel/old.rs", type: "blob", size: 1 }, { path: "kernel/prev.rs", type: "blob", size: 1 }] },
      };
    }
    return repoWhere(false)(c);
  });

  await new GitHub("o/r", "t").commit({
    branch: "agent/abc12345",
    message: "coder: finish the boot",
    changes: [{ path: "kernel/src/main.rs", content: "#![no_std]" }],
    base: "agent/base0000",
  });

  const commit = calls.find((c) => c.method === "POST" && c.path.endsWith("/git/commits"));
  assert.equal(commit?.body?.parents?.[0], "mainhead", "born on main, so it runs main's CI");

  const posted = calls.find((c) => c.method === "POST" && c.path.endsWith("/git/trees"));
  const entries = (posted?.body as unknown as { tree: Array<{ path: string; sha: string | null }> }).tree;
  const sha = new Map(entries.map((e) => [e.path, e.sha]));
  assert.equal(sha.get("kernel/Cargo.toml"), "b-cargo", "the work it continues comes along");
  assert.equal(sha.get("kernel/src/main.rs"), "blob1", "the agent's own version wins");
  assert.equal(sha.get("kernel/new.rs"), "b-new");
  assert.equal(sha.get("kernel/old.rs"), null);
  assert.equal(sha.get("kernel/prev.rs"), null);
  assert.ok(!sha.has("kernel/gone.rs"), "no deletion of a path main does not have");
  assert.ok(!sha.has(".github/workflows/verify.yml"), "never a workflow file");
});

test("an existing branch is fast-forwarded, never recreated", async () => {
  const calls = stub(repoWhere(true));
  await new GitHub("o/r", "t").commit({
    branch: "agent/abc12345",
    message: "coder: add entry point",
    changes: [{ path: "kernel/src/main.rs", content: "#![no_std]" }],
  });

  assert.ok(calls.some((c) => c.method === "PATCH"), "mise à jour de la ref existante");
  assert.ok(!calls.some((c) => c.method === "POST" && c.path.endsWith("/git/refs")));
  const commit = calls.find((c) => c.method === "POST" && c.path.endsWith("/git/commits"));
  assert.equal(commit?.body?.parents?.[0], "branchhead");
});
