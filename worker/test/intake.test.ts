import { test } from "node:test";
import assert from "node:assert/strict";

process.env.GRENOS_ROOT = new URL("../..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
process.env.SUPABASE_URL ??= "https://test.supabase.co";
process.env.SUPABASE_SERVICE_ROLE_KEY ??= "test";
process.env.GEMINI_API_KEY ??= "test";
process.env.GITHUB_TOKEN ??= "test";

const { canClaimStep, renderRoadmap } = await import("../src/intake.ts");

const step = (over: Record<string, unknown> = {}) => ({
  position: 1,
  key: "boot-serial",
  title: "Boot et port série",
  goal: "g",
  state: "todo",
  done_when: ["x"],
  mission_id: null,
  ...over,
});

// Mission 1 ran all day while the map showed its step as "todo": nothing ever
// linked a mission to the roadmap. These are the rules for doing it.

test("a free step is claimed by the mission that implements it", () => {
  assert.deepEqual(canClaimStep([step()], "boot-serial"), { ok: true });
});

test("a step carried by a live or finished mission is never taken over", () => {
  // Re-linking would hide that mission's progress, or its success, from the map.
  assert.equal(canClaimStep([step({ mission_id: "m1", state: "active" })], "boot-serial").ok, false);
  assert.equal(canClaimStep([step({ mission_id: "m1", state: "done" })], "boot-serial").ok, false);
});

test("an aborted mission frees its step for the next attempt", () => {
  assert.deepEqual(canClaimStep([step({ mission_id: "m1", state: "aborted" })], "boot-serial"), { ok: true });
});

test("an invented key is refused, never created", () => {
  assert.equal(canClaimStep([step()], "boot").ok, false);
});

test("the Master sees every step with its state and what CI must observe", () => {
  const text = renderRoadmap([step(), step({ position: 2, key: "gdt-idt" })]);
  assert.match(text, /1\. `boot-serial` \[todo\] Boot et port série/);
  assert.match(text, /2\. `gdt-idt`/);
  assert.match(text, /done when: \["x"\]/);
});

test("no roadmap, no section", () => {
  assert.equal(renderRoadmap([]), "");
});
