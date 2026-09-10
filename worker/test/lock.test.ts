import { test } from "node:test";
import assert from "node:assert/strict";

process.env.GRENOS_ROOT = new URL("../..", import.meta.url).pathname.replace(
  /^\/([A-Za-z]:)/,
  "$1",
);
process.env.SUPABASE_URL ??= "https://test.supabase.co";
process.env.SUPABASE_SERVICE_ROLE_KEY ??= "test";
process.env.GEMINI_API_KEY ??= "test";
process.env.GITHUB_TOKEN ??= "test";

const { decide, STALE_AFTER_MS } = await import("../src/lock.ts");

// Six workers ran at once for a day, each planning and committing on its own.
// These are the rules that make a second one refuse to start.

const now = Date.parse("2026-09-10T16:00:00Z");
const holder = (secondsAgo: number) => ({
  instance: "other",
  pid: 3932,
  host: "DESKTOP",
  started_at: "2026-09-10T07:04:16Z",
  heartbeat_at: new Date(now - secondsAgo * 1000).toISOString(),
});

test("an empty lock is taken", () => {
  assert.deepEqual(decide(null, "me", now), { acquire: true, takeover: null });
});

test("our own lock is ours", () => {
  assert.equal(decide({ ...holder(5), instance: "me" }, "me", now).acquire, true);
});

test("a live worker makes the second one refuse", () => {
  const d = decide(holder(10), "me", now);
  assert.equal(d.acquire, false);
  assert.ok(!d.acquire && d.holder.pid === 3932, "le refus doit nommer l'autre worker");
});

test("a worker silent past the threshold is presumed dead and taken over", () => {
  const d = decide(holder(STALE_AFTER_MS / 1000 + 1), "me", now);
  assert.equal(d.acquire, true);
  assert.ok(d.acquire && d.takeover?.pid === 3932);
});

test("an unreadable heartbeat does not lock the system out forever", () => {
  const d = decide({ ...holder(0), heartbeat_at: "not a date" }, "me", now);
  assert.equal(d.acquire, true);
});
