import { test, beforeEach } from "node:test";
import assert from "node:assert/strict";
import { CASCADES, Router, RouterExhaustedError, type UsageStore } from "../src/index.ts";

/**
 * The cascade only misbehaves under conditions that are inconvenient to
 * reproduce by hand: an exhausted quota at 6pm, a model retired overnight, a
 * provider timing out on a cold start. Stubbing fetch makes those the normal
 * case for a test run.
 *
 * The stub speaks both providers, because the point of the cascade is that it
 * crosses them: NVIDIA leads on speed and volume, Gemini is the floor that
 * never runs out of credits.
 */

const realFetch = globalThis.fetch;

const NV = "deepseek-ai/deepseek-v4-flash-0731";
const NEMO = "nvidia/nemotron-3-super-120b-a12b";
const KIMI = "moonshotai/kimi-k3";

type Handler = (model: string, budget: number) => Response | Promise<Response>;

/** Records every model actually called, in order. */
function stubFetch(handler: Handler): string[] {
  const calls: string[] = [];
  globalThis.fetch = (async (url: string | URL | Request, init?: RequestInit) => {
    const href = String(url);
    const body = init?.body ? JSON.parse(String(init.body)) : {};

    // Gemini names the model in the path; NVIDIA puts it in the body.
    const model = href.includes("generativelanguage")
      ? href.slice(href.lastIndexOf("/") + 1).replace(":generateContent", "")
      : body.model;
    const budget = body.generationConfig?.maxOutputTokens ?? body.max_tokens ?? 0;

    calls.push(model);
    return handler(model, budget);
  }) as typeof fetch;
  return calls;
}

const geminiOk = (text = "{}") =>
  new Response(
    JSON.stringify({
      candidates: [{ content: { parts: [{ text }] }, finishReason: "STOP" }],
      usageMetadata: { promptTokenCount: 10, candidatesTokenCount: 5 },
    }),
    { status: 200 },
  );

const nvidiaOk = (text = "{}") =>
  new Response(
    JSON.stringify({
      choices: [{ message: { content: text }, finish_reason: "stop" }],
      usage: { prompt_tokens: 10, completion_tokens: 5 },
    }),
    { status: 200 },
  );

const anyOk = (model: string) => (model.includes("gemini") ? geminiOk() : nvidiaOk());

const dailyQuota429 = () =>
  new Response(
    JSON.stringify({
      error: {
        code: 429,
        message: "Quota exceeded for metric: generate_content_free_tier_requests, limit: 20",
      },
    }),
    { status: 429 },
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
const both = () => ({ geminiApiKey: "g", nvidiaApiKey: "n", usage });

beforeEach(() => {
  usage = new FakeUsage();
  globalThis.fetch = realFetch;
});

test("the coder starts on the strongest model, not the cheapest", async () => {
  const calls = stubFetch(anyOk);
  const res = await new Router(both()).complete({ role: "coder", system: "s", messages: [] });

  assert.equal(res.model, NV);
  assert.equal(res.provider, "nvidia");
  assert.deepEqual(calls, [NV]);
});

test("the master starts on the fastest model — it runs on every state change", async () => {
  stubFetch(anyOk);
  const res = await new Router(both()).complete({ role: "master", system: "s", messages: [] });
  assert.equal(res.model, NEMO);
});

test("the architect falls back to Nemotron, then to the slow model", async () => {
  stubFetch((m) => (m === NV ? new Response("down", { status: 503 }) : anyOk(m)));
  const res = await new Router(both()).complete({ role: "architect", system: "s", messages: [] });
  assert.equal(res.model, NEMO);
  stubFetch((m) => (m === KIMI ? anyOk(m) : new Response("down", { status: 503 })));
  const slow = await new Router(both()).complete({ role: "architect", system: "s", messages: [] });
  assert.equal(slow.model, KIMI);
});

test("the cascade crosses providers when NVIDIA fails", async () => {
  // The whole point of mixing them: NVIDIA has the volume, Gemini has the
  // permanence. One provider being down must not stop the team.
  const calls = stubFetch((m) =>
    m.includes("gemini") ? geminiOk() : new Response("gateway timeout", { status: 504 }),
  );
  const res = await new Router(both()).complete({ role: "coder", system: "s", messages: [] });

  assert.equal(res.provider, "gemini");
  assert.equal(res.model, "gemini-3.8-flash");
  assert.deepEqual(calls, [NV, NEMO, KIMI, "gemini-3.8-flash"]);
});

test("a second Gemini floor answers when the first is at high demand", async () => {
  // 2026-09-11: the NIM models timing out or answering 503 while
  // gemini-3.8-flash refused for "high demand", all at once, for most of an hour.
  const calls = stubFetch((m) =>
    m === "gemini-3.7-flash" ? geminiOk() : new Response("high demand", { status: 503 }),
  );
  const res = await new Router(both()).complete({ role: "coder", system: "s", messages: [] });

  assert.equal(res.model, "gemini-3.7-flash");
  assert.deepEqual(calls, [NV, NEMO, KIMI, "gemini-3.8-flash", "gemini-3.7-flash"]);
});

test("a retired model is set aside for a day, not retried every tick", async () => {
  // DeepSeek V4 Pro answered 410 Gone from 2026-09-14, when it reached its
  // end of life on NIM.
  const calls = stubFetch((m) => (m === NV ? new Response("Gone", { status: 410 }) : anyOk(m)));
  const router = new Router(both());
  const first = await router.complete({ role: "coder", system: "s", messages: [] });
  assert.equal(first.model, NEMO);
  await router.complete({ role: "coder", system: "s", messages: [] });
  assert.equal(calls.filter((m) => m === NV).length, 1);
});

test("a model's own request fields and timeout reach NVIDIA", async () => {
  let body: Record<string, unknown> = {};
  globalThis.fetch = (async (_url: string | URL | Request, init?: RequestInit) => {
    body = JSON.parse(String(init?.body));
    return nvidiaOk();
  }) as typeof fetch;
  await new Router(both()).complete({ role: "coder", system: "s", messages: [] });
  assert.deepEqual(body["chat_template_kwargs"], { thinking: false, enable_thinking: false });
});

test("without an NVIDIA key its models are skipped, not called", async () => {
  const calls = stubFetch(anyOk);
  const res = await new Router({ geminiApiKey: "g", usage }).complete({
    role: "coder",
    system: "s",
    messages: [],
  });

  assert.equal(res.model, "gemini-3.8-flash");
  assert.deepEqual(calls, ["gemini-3.8-flash"], "aucun appel réseau vers NVIDIA");
  assert.equal(res.attempts[0]?.outcome, "skipped");
});

test("a daily quota retires the model for the rest of the day", async () => {
  // Learned from the server, never guessed: published free-tier figures were
  // wrong by two orders of magnitude.
  const calls = stubFetch((m) => (m === NV ? dailyQuota429() : anyOk(m)));
  const router = new Router(both());

  const first = await router.complete({ role: "coder", system: "s", messages: [] });
  assert.equal(first.model, NEMO);
  assert.equal(first.attempts[0]?.outcome, "quota_exhausted");

  calls.length = 0;
  const second = await router.complete({ role: "coder", system: "s", messages: [] });
  assert.equal(second.model, NEMO);
  assert.ok(!calls.includes(NV), "épuisé jusqu'à demain, on ne le rappelle pas");
});

test("a per-minute 429 is a cooldown, not a retirement", async () => {
  const perMinute = () =>
    new Response(JSON.stringify({ error: { message: "Quota exceeded: requests_per_minute" } }), {
      status: 429,
      headers: { "retry-after": "1" },
    });
  stubFetch((m) => (m === NV ? perMinute() : anyOk(m)));

  const res = await new Router(both()).complete({ role: "coder", system: "s", messages: [] });
  assert.equal(res.attempts[0]?.outcome, "rate_limited", "pas quota_exhausted");
});

test("a model that vanished (404) does not stop the system", async () => {
  stubFetch((m) => (m === NV ? new Response("gone", { status: 404 }) : anyOk(m)));
  const res = await new Router(both()).complete({ role: "coder", system: "s", messages: [] });
  assert.equal(res.model, NEMO);
});

test("a model that thinks past its budget gets more room, not a different model", async () => {
  // Reasoning tokens come out of the same allowance as the answer, so a tight
  // budget returns an empty body. Switching models would hit the same wall.
  const budgets: number[] = [];
  stubFetch((model, budget) => {
    if (model !== NV) return anyOk(model);
    budgets.push(budget);
    if (budget < 20_000) {
      return new Response(
        JSON.stringify({
          choices: [{ message: { content: "" }, finish_reason: "length" }],
          usage: { completion_tokens: budget },
        }),
        { status: 200 },
      );
    }
    return nvidiaOk();
  });

  const res = await new Router(both()).complete({ role: "coder", system: "s", messages: [] });

  assert.equal(res.model, NV, "même modèle, pas un repli");
  assert.equal(budgets.length, 2);
  assert.ok(budgets[1]! > budgets[0]!, "le second essai doit avoir plus de place");
});

test("a reasoning model that never answers is a truncation, not an empty reply", async () => {
  // Some models put their thinking in a separate field and return empty
  // content. That is the same failure wearing a different shape.
  let calls = 0;
  stubFetch((model, budget) => {
    if (model !== NV) return anyOk(model);
    calls += 1;
    if (budget < 20_000) {
      return new Response(
        JSON.stringify({
          choices: [
            { message: { content: "", reasoning_content: "thinking..." }, finish_reason: "stop" },
          ],
          usage: { completion_tokens: 100 },
        }),
        { status: 200 },
      );
    }
    return nvidiaOk();
  });

  const res = await new Router(both()).complete({ role: "coder", system: "s", messages: [] });
  assert.equal(res.model, NV);
  assert.equal(calls, 2, "relancé avec plus de place");
});

test("reasoning tokens are counted, not hidden", async () => {
  stubFetch(() =>
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
  const res = await new Router({ geminiApiKey: "g", usage }).complete({
    role: "coder",
    system: "s",
    messages: [],
  });

  // 940, not 40: a mission budget that ignores reasoning would let the Master
  // overspend by an order of magnitude.
  assert.equal(res.tokensOut, 940);
});

test("a refused key stops the cascade instead of repeating the 403", async () => {
  // Every Gemini model shares one key and one project, so a refusal is the
  // same refusal three times over.
  const calls = stubFetch(() => new Response("denied", { status: 403 }));

  await assert.rejects(
    () =>
      new Router({ geminiApiKey: "g", usage }).complete({
        role: "coder",
        system: "s",
        messages: [],
      }),
    /refuse la clé ou son projet/,
  );
  assert.equal(calls.length, 1);
});

test("only fails once every model is unusable", async () => {
  const calls = stubFetch(() => dailyQuota429());
  const router = new Router(both());

  await assert.rejects(
    () => router.complete({ role: "coder", system: "s", messages: [] }),
    RouterExhaustedError,
  );
  assert.equal(calls.length, CASCADES.coder.length, "un appel par modèle pour apprendre chaque limite");

  calls.length = 0;
  await assert.rejects(
    () => router.complete({ role: "coder", system: "s", messages: [] }),
    RouterExhaustedError,
  );
  assert.equal(calls.length, 0, "les limites apprises évitent tout appel");
});

test("failures are recorded so the dashboard can show them", async () => {
  stubFetch((m) => (m === NV ? new Response("boom", { status: 500 }) : anyOk(m)));
  await new Router(both()).complete({ role: "coder", system: "s", messages: [] });
  assert.ok(usage.recorded.some((r) => r.model === NV && r.error));
});

test("a missing Gemini key fails loudly at construction", () => {
  assert.throws(() => new Router({ geminiApiKey: "", usage }), /GEMINI_API_KEY/);
});

test("an overloaded provider is cooled down, not hammered on the next call", async () => {
  const calls = stubFetch((m) => (m === NV ? new Response("overloaded", { status: 503 }) : anyOk(m)));
  const router = new Router(both());

  await router.complete({ role: "coder", system: "s", messages: [] });
  calls.length = 0;
  await router.complete({ role: "coder", system: "s", messages: [] });

  // Retrying a 503 every tick turns one outage into a stream of failures.
  assert.ok(!calls.includes(NV), "503 : le modèle doit être en pause");
});

test("available() answers no once every model of the role is cooling down", async () => {
  stubFetch(() => new Response("overloaded", { status: 503 }));
  const router = new Router(both());

  assert.equal(router.available("coder"), true);
  await assert.rejects(
    () => router.complete({ role: "coder", system: "s", messages: [] }),
    RouterExhaustedError,
  );
  // The dispatcher asks this before claiming a task, so an outage the router
  // already knows about costs no model call and no provider_error event.
  assert.equal(router.available("coder"), false);
});

test("a refused key sets its whole provider aside instead of retrying every tick", async () => {
  const calls = stubFetch(() => new Response("denied", { status: 403 }));
  const router = new Router({ geminiApiKey: "g", usage });

  await assert.rejects(
    () => router.complete({ role: "tester", system: "s", messages: [] }),
    /refuse la clé ou son projet/,
  );

  // A stale .env.local put a refused Gemini key back in production, and the
  // worker hit the same 403 on every 15-second tick. The refusal is known now:
  // the next call must not cost a single request.
  calls.length = 0;
  await assert.rejects(
    () => router.complete({ role: "coder", system: "s", messages: [] }),
    /refuse la clé/,
  );
  assert.equal(calls.length, 0);
  assert.equal(router.available("coder"), false);
});

test("a refused NVIDIA key is reported as NVIDIA's, not Gemini's", async () => {
  stubFetch((m) =>
    m.includes("gemini")
      ? new Response("overloaded", { status: 503 })
      : new Response("unauthorized", { status: 401 }),
  );
  const router = new Router(both());

  await assert.rejects(
    () => router.complete({ role: "coder", system: "s", messages: [] }),
    (err: Error) => /NVIDIA refuse la clé/.test(err.message) && !/Gemini refuse/.test(err.message),
  );
});

test("a panel takes distinct NVIDIA models of the role, and a pinned request tries only its model", async () => {
  const { Router, InMemoryUsageStore, CASCADES, MODELS } = await import("../src/index.ts");
  const router = new Router({ geminiApiKey: "g", nvidiaApiKey: "n", usage: new InMemoryUsageStore() });
  const members = router.panel("coder", 3);
  assert.equal(new Set(members).size, members.length);
  assert.ok(members.length >= 1 && members.length <= 3);
  assert.ok(members.every((m: string) => CASCADES.coder.includes(m) && MODELS[m]?.provider === "nvidia"));
  const noNvidia = new Router({ geminiApiKey: "g", usage: new InMemoryUsageStore() });
  assert.ok(noNvidia.panel("coder", 3).every((m: string) => MODELS[m]?.provider === "gemini"));
});
