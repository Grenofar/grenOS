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
  // 9 agents d'exécution + l'agent de cadrage, qui mène l'entretien de
  // création d'une mission (agents/04-intake.md).
  assert.equal(agents.length, 10);

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

test("the whole team is active and correctly wired", () => {
  const byId = new Map(loadAgents().map((a) => [a.id, a]));

  for (const id of [
    "master", "architect", "coder", "tester",
    "kernel", "filesystem", "drivers", "security", "review", "intake",
  ]) {
    assert.equal(byId.get(id)?.status, "active", `${id} devrait être actif`);
  }

  assert.equal(byId.get("master")!.roleClass, "orchestrator");
  assert.equal(byId.get("coder")!.reportsTo, "master");
});

test("specialist paths are closed to the Coder by the sandbox, not by a prompt", async () => {
  const { authorize } = await import("../src/sandbox.ts");
  const byId = new Map(loadAgents().map((a) => [a.id, a]));
  const coder = byId.get("coder")!;
  const rules = {
    allowedPaths: coder.allowedPaths,
    forbiddenPaths: coder.forbiddenPaths,
    canWrite: coder.canWrite,
  };

  // The Coder holds kernel/** so these overlap on purpose. A wrong MMIO
  // register or a bad IDT entry produces a silent reset, not an error, so the
  // precedence has to be enforced rather than requested.
  for (const path of [
    "kernel/src/arch/gdt.rs",
    "kernel/src/mm/frame_alloc.rs",
    "kernel/src/interrupts/idt.rs",
    "kernel/src/drivers/serial.rs",
    "kernel/src/fs/vfs.rs",
    "kernel/src/pci/enumerate.rs",
  ]) {
    assert.equal(authorize(path, rules).ok, false, `le Codeur ne doit pas écrire ${path}`);
  }

  // Everything else under kernel/ stays his.
  assert.equal(authorize("kernel/src/main.rs", rules).ok, true);
  assert.equal(authorize("kernel/Cargo.toml", rules).ok, true);
});

test("each specialist can write its own domain", async () => {
  const { authorize } = await import("../src/sandbox.ts");
  const byId = new Map(loadAgents().map((a) => [a.id, a]));

  const cases: Array<[string, string]> = [
    ["kernel", "kernel/src/mm/frame_alloc.rs"],
    ["filesystem", "kernel/src/fs/vfs.rs"],
    ["drivers", "kernel/src/drivers/serial.rs"],
  ];

  for (const [id, path] of cases) {
    const a = byId.get(id)!;
    const verdict = authorize(path, {
      allowedPaths: a.allowedPaths,
      forbiddenPaths: a.forbiddenPaths,
      canWrite: a.canWrite,
    });
    assert.equal(verdict.ok, true, `${id} devrait pouvoir écrire ${path}`);
  }
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

test("the intake agent can write nothing at all", async () => {
  const { authorize } = await import("../src/sandbox.ts");
  const intake = loadAgents().find((a) => a.id === "intake")!;

  // Il conduit une conversation et lance une mission ; il ne touche jamais au
  // dépôt. Le sandbox l'applique, la prudence du prompt ne suffit pas.
  assert.equal(intake.canWrite, false);
  for (const path of ["docs/notes.md", "kernel/src/main.rs", "agents/00-master.md"]) {
    assert.equal(
      authorize(path, {
        allowedPaths: intake.allowedPaths,
        forbiddenPaths: intake.forbiddenPaths,
        canWrite: intake.canWrite,
      }).ok,
      false,
    );
  }
});
