import { test } from "node:test";
import assert from "node:assert/strict";

// The loader reads config.ts, which requires the whole environment. Point it at
// the repo root and give it the minimum so the module can initialise.
process.env.GRENOS_ROOT = new URL("../..", import.meta.url).pathname.replace(
  /^\/([A-Za-z]:)/,
  "$1",
);
process.env.SUPABASE_URL ??= "https://test.supabase.co";
process.env.SUPABASE_SERVICE_ROLE_KEY ??= "test";
process.env.GEMINI_API_KEY ??= "test";
process.env.GITHUB_TOKEN ??= "test";

const { loadAgents } = await import("../src/prompts.ts");

test("every agent file parses", () => {
  const agents = loadAgents();
  assert.equal(agents.length, 9, "4 actifs + 5 dormants");

  for (const a of agents) {
    assert.ok(a.id, "id manquant");
    assert.ok(a.systemPrompt.length > 500, `${a.id}: prompt suspicieusement court`);
    assert.ok(
      a.systemPrompt.includes("grenOS Agent Protocol"),
      `${a.id}: le protocole partagé n'est pas préfixé`,
    );
    assert.equal(a.promptSha.length, 64);
  }
});

test("the four core agents are active and correctly wired", () => {
  const byId = new Map(loadAgents().map((a) => [a.id, a]));

  for (const id of ["master", "architect", "coder", "tester"]) {
    assert.equal(byId.get(id)?.status, "active", `${id} devrait être actif`);
  }
  for (const id of ["kernel", "filesystem", "drivers", "security", "review"]) {
    assert.equal(byId.get(id)?.status, "dormant", `${id} devrait être dormant`);
  }

  assert.equal(byId.get("master")!.roleClass, "orchestrator");
  assert.equal(byId.get("coder")!.reportsTo, "master");
});

test("structural safeguards survive the parse", () => {
  const byId = new Map(loadAgents().map((a) => [a.id, a]));

  // D-009: the verifier must not be able to edit the implementation.
  assert.ok(
    byId.get("tester")!.forbiddenPaths.includes("kernel/src/**"),
    "le Testeur doit rester interdit d'écriture sur kernel/src",
  );

  // The reviewer holds no write capability at all.
  assert.equal(byId.get("review")!.canWrite, false);

  // D-007: no agent may rewrite the prompts, whatever its own file claims.
  for (const agent of byId.values()) {
    assert.ok(
      agent.forbiddenPaths.includes("agents/**"),
      `${agent.id} devrait avoir agents/** interdit`,
    );
    assert.ok(
      agent.forbiddenPaths.includes("**/.env*"),
      `${agent.id} devrait avoir **/.env* interdit`,
    );
  }
});

test("reports_to: human becomes null, not a dangling foreign key", () => {
  const master = loadAgents().find((a) => a.id === "master")!;
  // The .md says "human" because that is who the Master answers to. The
  // agents table has a foreign key on this column, so anything that is not
  // another agent must be null — this failed on the first worker start.
  assert.equal(master.reportsTo, null);

  const coder = loadAgents().find((a) => a.id === "coder")!;
  assert.equal(coder.reportsTo, "master", "une vraie référence reste intacte");
});
