import { test, beforeEach } from "node:test";
import assert from "node:assert/strict";
import { Router, RouterExhaustedError, type UsageStore } from "../src/index.ts";

/**
 * The cascade is the part of the router that only misbehaves under conditions
 * that are inconvenient to reproduce by hand: an exhausted quota at 6pm, a
 * model retired by Google overnight. Stubbing fetch makes those the normal
 * case for a test run.
 */

const realFetch = globalThis.fetch;

function stubFetch(handler: (model: string) => Response | Promise<Response>): string[] {
  const calls: string[] = [];
  globalThis.fetch = (async (url: string | URL | Request) => {
    const href = String(url);
    const model = href.slice(href.lastIndexOf("/") + 1).replace(":generateContent", "");
    calls.push(model);
    return handler(model);
  }) as typeof fetch;
  return calls;
}

const okResponse = (text: string) =>
  new Response(
    JSON.stringify({
      candidates: [{ content: { parts: [{ text }] }, finishReason: "STOP" }],
      usageMetadata: { promptTokenCount: 10, candidatesTokenCount: 5 },
    }),
    { status: 200, headers: { "content-type": "application/json" } },
  );

class FakeUsage implements UsageStore {
  counts = new Map<string, number>();
  recorded: Array<{ model: string; error?: string }> = [];

  async requestsToday(model: string): Promise<number> {
    return this.counts.get(model) ?? 0;
  }
  async record(e: { model: string; error?: string }): Promise<void> {
    this.recorded.push(e);
    this.counts.set(e.model, (this.counts.get(e.model) ?? 0) + 1);
  }
}

let usage: FakeUsage;
beforeEach(() => {
  usage = new FakeUsage();
  globalThis.fetch = realFetch;
});

test("uses the first model in the role cascade", async () => {
  const calls = stubFetch(() => okResponse('{"ok":true}'));
  const router = new Router({ geminiApiKey: "k", usage });

  const res = await router.complete({ role: "coder", system: "s", messages: [] });

  assert.equal(res.model, "gemini-3.8-flash");
  assert.deepEqual(calls, ["gemini-3.8-flash"]);
  assert.equal(res.tokensIn, 10);
});

test("the tester starts on 3.6, leaving 3.8 quota to the others", async () => {
  stubFetch(() => okResponse("{}"));
  const router = new Router({ geminiApiKey: "k", usage });

  const res = await router.complete({ role: "tester", system: "s", messages: [] });
  assert.equal(res.model, "gemini-3.6-flash");
});

const dailyQuota429 = () =>
  new Response(
    JSON.stringify({
      error: {
        code: 429,
        message:
          "You exceeded your current quota. Quota exceeded for metric: " +
          "generativelanguage.googleapis.com/generate_content_free_tier_requests, limit: 20",
      },
    }),
    { status: 429 },
  );

test("a daily quota retires the model for the rest of the day", async () => {
  // Learned from the server, never guessed: published free-tier figures were
  // wrong by two orders of magnitude.
  let calls = stubFetch((model) =>
    model === "gemini-3.8-flash" ? dailyQuota429() : okResponse("{}"),
  );
  const router = new Router({ geminiApiKey: "k", usage });

  const first = await router.complete({ role: "coder", system: "s", messages: [] });
  assert.equal(first.model, "gemini-3.7-flash");
  assert.equal(first.attempts[0]?.outcome, "quota_exhausted");

  // The next call must not spend a round trip re-discovering the same limit.
  calls.length = 0;
  const second = await router.complete({ role: "coder", system: "s", messages: [] });
  assert.equal(second.model, "gemini-3.7-flash");
  assert.ok(!calls.includes("gemini-3.8-flash"), "3.8 est épuisé jusqu'à demain");
});

test("a per-minute 429 is a cooldown, not a retirement", async () => {
  const perMinute = () =>
    new Response(
      JSON.stringify({
        error: { code: 429, message: "Quota exceeded for metric: ...generate_requests_per_minute" },
      }),
      { status: 429, headers: { "retry-after": "1" } },
    );
  stubFetch((model) => (model === "gemini-3.8-flash" ? perMinute() : okResponse("{}")));
  const router = new Router({ geminiApiKey: "k", usage });

  const res = await router.complete({ role: "coder", system: "s", messages: [] });
  assert.equal(res.model, "gemini-3.7-flash");
  assert.equal(res.attempts[0]?.outcome, "rate_limited", "pas quota_exhausted");
});

test("a 429 falls through and puts that model in cooldown", async () => {
  const calls = stubFetch((model) =>
    model === "gemini-3.8-flash"
      ? new Response("rate limited", { status: 429, headers: { "retry-after": "30" } })
      : okResponse("{}"),
  );
  const router = new Router({ geminiApiKey: "k", usage });

  const first = await router.complete({ role: "coder", system: "s", messages: [] });
  assert.equal(first.model, "gemini-3.7-flash");

  // The second call must not re-try the model that just said 429.
  calls.length = 0;
  const second = await router.complete({ role: "coder", system: "s", messages: [] });
  assert.equal(second.model, "gemini-3.7-flash");
  assert.ok(!calls.includes("gemini-3.8-flash"), "3.8 devrait être en cooldown");
});

test("a model that vanished (404) does not stop the system", async () => {
  stubFetch((model) =>
    model === "gemini-3.8-flash"
      ? new Response("model not found", { status: 404 })
      : okResponse("{}"),
  );
  const router = new Router({ geminiApiKey: "k", usage });

  const res = await router.complete({ role: "coder", system: "s", messages: [] });
  assert.equal(res.model, "gemini-3.7-flash");
});

test("a truncated answer is a failure, not a half-parsed envelope", async () => {
  stubFetch((model) =>
    model === "gemini-3.8-flash"
      ? new Response(
          JSON.stringify({
            candidates: [{ content: { parts: [{ text: '{"status":"do' }] }, finishReason: "MAX_TOKENS" }],
          }),
          { status: 200 },
        )
      : okResponse("{}"),
  );
  const router = new Router({ geminiApiKey: "k", usage });

  const res = await router.complete({ role: "coder", system: "s", messages: [] });
  assert.equal(res.model, "gemini-3.7-flash", "la troncature doit faire basculer de modèle");
});

test("only fails once every model is unusable", async () => {
  const calls = stubFetch(() => dailyQuota429());
  const router = new Router({ geminiApiKey: "k", usage });

  await assert.rejects(
    () => router.complete({ role: "coder", system: "s", messages: [] }),
    RouterExhaustedError,
  );
  assert.equal(calls.length, 3, "un appel par modèle pour apprendre chaque limite");

  // Second time round, nothing is left to try and nothing is spent finding out.
  calls.length = 0;
  await assert.rejects(
    () => router.complete({ role: "coder", system: "s", messages: [] }),
    RouterExhaustedError,
  );
  assert.equal(calls.length, 0, "les limites apprises évitent tout appel");
});

test("a model that thinks past its budget gets more room, not a different model", async () => {
  // Gemini 3.x spends reasoning tokens from the same allowance as the answer,
  // so a tight budget returns MAX_TOKENS with an empty body. Switching models
  // would hit the same wall.
  const budgets: number[] = [];
  globalThis.fetch = (async (_url: string | URL | Request, init?: RequestInit) => {
    const body = JSON.parse(String(init?.body));
    const max = body.generationConfig.maxOutputTokens;
    budgets.push(max);
    if (max < 20_000) {
      return new Response(
        JSON.stringify({
          candidates: [{ content: { parts: [] }, finishReason: "MAX_TOKENS" }],
          usageMetadata: { thoughtsTokenCount: max },
        }),
        { status: 200 },
      );
    }
    return okResponse("{}");
  }) as typeof fetch;

  const router = new Router({ geminiApiKey: "k", usage });
  const res = await router.complete({ role: "coder", system: "s", messages: [] });

  assert.equal(res.model, "gemini-3.8-flash", "même modèle, pas un repli");
  assert.equal(budgets.length, 2);
  assert.ok(budgets[1]! > budgets[0]!, "le second essai doit avoir plus de place");
});

test("reasoning tokens are counted, not hidden", async () => {
  stubFetch(
    () =>
      new Response(
        JSON.stringify({
          candidates: [{ content: { parts: [{ text: "{}" }] }, finishReason: "STOP" }],
          usageMetadata: {
            promptTokenCount: 100,
            candidatesTokenCount: 40,
            thoughtsTokenCount: 900,
          },
        }),
        { status: 200 },
      ),
  );
  const router = new Router({ geminiApiKey: "k", usage });
  const res = await router.complete({ role: "coder", system: "s", messages: [] });

  // 940, not 40: reasoning is billed, and a mission budget that ignores it
  // would let the Master overspend by an order of magnitude.
  assert.equal(res.tokensOut, 940);
});

test("failures are recorded so the dashboard can show them", async () => {
  stubFetch((model) =>
    model === "gemini-3.8-flash" ? new Response("boom", { status: 500 }) : okResponse("{}"),
  );
  const router = new Router({ geminiApiKey: "k", usage });

  await router.complete({ role: "coder", system: "s", messages: [] });
  assert.ok(usage.recorded.some((r) => r.model === "gemini-3.8-flash" && r.error));
});

test("a missing key fails loudly at construction", () => {
  assert.throws(() => new Router({ geminiApiKey: "", usage }), /GEMINI_API_KEY/);
});

test("a refused project stops the cascade instead of repeating the 403", async () => {
  const calls = stubFetch(
    () =>
      new Response(
        JSON.stringify({
          error: { code: 403, message: "Your project has been denied access." },
        }),
        { status: 403 },
      ),
  );
  const router = new Router({ geminiApiKey: "k", usage });

  await assert.rejects(
    () => router.complete({ role: "coder", system: "s", messages: [] }),
    // The message must name the real cause, not "no model available": every
    // model shares one key and one project, so this is not a quota problem.
    /refuse la clé ou son projet/,
  );
  assert.equal(calls.length, 1, "un seul appel — inutile de répéter le même refus");
});
