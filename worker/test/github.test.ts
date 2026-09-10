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
