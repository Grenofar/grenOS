import { test } from "node:test";
import assert from "node:assert/strict";
import { allowedUrl, clip, htmlToText, summarizeCrate } from "../src/consult.ts";

// An agent may now read the web before it writes. These tests pin down how
// narrow "the web" is, and what it is shown of a page.

test("documentation hosts only, and only over https", () => {
  assert.ok(allowedUrl("https://docs.rs/limine/latest/limine/"));
  assert.ok(allowedUrl("https://crates.io/api/v1/crates/limine"));
  assert.ok(allowedUrl("https://raw.githubusercontent.com/limine-bootloader/limine/trunk/CONFIG.md"));

  assert.equal(allowedUrl("http://docs.rs/limine"), null);
  assert.equal(allowedUrl("https://evil.example/docs.rs"), null);
  assert.equal(allowedUrl("https://docs.rs.evil.example/"), null);
  assert.equal(allowedUrl("https://user:pw@docs.rs/"), null);
  assert.equal(allowedUrl("not a url"), null);
  // Never the services the worker itself talks to with credentials.
  assert.equal(allowedUrl("https://api.github.com/repos/Grenofar/grenOS"), null);
  assert.equal(allowedUrl("https://example.supabase.co/rest/v1/tasks"), null);
});

test("a page becomes text an agent can read", () => {
  const text = htmlToText(
    "<html><head><style>p{color:red}</style><script>alert(1)</script></head>" +
      "<body><nav>menu</nav><h1>limine</h1><p>Version 0.6.5 &amp; later &lt;3</p></body></html>",
  );
  assert.match(text, /limine/);
  assert.match(text, /Version 0\.6\.5 & later <3/);
  assert.doesNotMatch(text, /alert|color:red|menu/);
});

test("a document is clipped, and says so", () => {
  const clipped = clip("x".repeat(100), 10);
  assert.ok(clipped.startsWith("x".repeat(10)));
  assert.match(clipped, /90 more characters/);
  assert.equal(clip("short", 10), "short");
});

test("a crates.io answer is reduced to the versions that matter", () => {
  const summary = summarizeCrate(
    JSON.stringify({
      crate: {
        name: "limine",
        max_version: "0.6.5",
        max_stable_version: "0.6.5",
        newest_version: "0.6.5",
        updated_at: "2026-06-07T13:54:52Z",
      },
      versions: [
        { num: "0.6.5", yanked: false, created_at: "2026-06-07T13:54:52Z" },
        { num: "0.6.4", yanked: true, created_at: "2026-05-01T00:00:00Z" },
      ],
    }),
  );
  assert.match(summary!, /max_stable_version: 0\.6\.5/);
  assert.match(summary!, /0\.6\.4 \(yanked\) — 2026-05-01/);
  assert.equal(summarizeCrate("{}"), null);
  assert.equal(summarizeCrate("not json"), null);
});
