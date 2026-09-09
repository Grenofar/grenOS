import { test } from "node:test";
import assert from "node:assert/strict";
import {
  authorize,
  globToRegExp,
  normalizePath,
  pathsOverlap,
} from "../src/sandbox.ts";

// The sandbox is the only thing standing between a misbehaving model and the
// repository, so it gets tested rather than assumed.

test("normalizePath rejects escapes", () => {
  assert.equal(normalizePath("kernel/src/main.rs"), "kernel/src/main.rs");
  assert.equal(normalizePath("./kernel/src/main.rs"), "kernel/src/main.rs");
  assert.equal(normalizePath("kernel\\src\\main.rs"), "kernel/src/main.rs");

  assert.equal(normalizePath("../outside.txt"), null);
  assert.equal(normalizePath("kernel/../../etc/passwd"), null);
  assert.equal(normalizePath("/etc/passwd"), null);
  assert.equal(normalizePath("C:/Windows/system32"), null);
  assert.equal(normalizePath("kernel//src/main.rs"), null);
  assert.equal(normalizePath("kernel/./main.rs"), null);
});

test("glob semantics", () => {
  assert.ok(globToRegExp("kernel/**").test("kernel/src/main.rs"));
  assert.ok(!globToRegExp("kernel/**").test("docs/main.rs"));

  assert.ok(globToRegExp("kernel/src/*.rs").test("kernel/src/main.rs"));
  assert.ok(!globToRegExp("kernel/src/*.rs").test("kernel/src/mm/frame.rs"));

  // `**/` must match zero directories too, or a root-level .env slips through.
  assert.ok(globToRegExp("**/.env*").test(".env.local"));
  assert.ok(globToRegExp("**/.env*").test("apps/web/.env"));
});

test("forbidden beats allowed", () => {
  const rules = {
    canWrite: true,
    allowedPaths: ["**"],
    forbiddenPaths: ["agents/**", "**/.env*"],
  };
  assert.equal(authorize("agents/00-master.md", rules).ok, false);
  assert.equal(authorize(".env.local", rules).ok, false);
  assert.equal(authorize("kernel/src/main.rs", rules).ok, true);
});

test("tester cannot touch kernel sources", () => {
  const tester = {
    canWrite: true,
    allowedPaths: ["kernel/tests/**", "tests/**"],
    forbiddenPaths: ["kernel/src/**", "agents/**", "**/.env*"],
  };
  assert.equal(authorize("kernel/tests/boot.rs", tester).ok, true);
  assert.equal(authorize("kernel/src/main.rs", tester).ok, false);
  // Escaping via traversal must fail before any glob is consulted.
  assert.equal(authorize("kernel/tests/../src/main.rs", tester).ok, false);
});

test("review agent can write nothing", () => {
  const review = { canWrite: false, allowedPaths: [], forbiddenPaths: ["**"] };
  assert.equal(authorize("docs/notes.md", review).ok, false);
});

test("overlap detection", () => {
  assert.ok(pathsOverlap(["kernel/src/**"], ["kernel/src/mm/**"]));
  assert.ok(pathsOverlap(["kernel/src/main.rs"], ["kernel/src/**"]));
  assert.ok(!pathsOverlap(["docs/**"], ["kernel/src/**"]));
});
