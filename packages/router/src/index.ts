import {
  CASCADES,
  MODELS,
  specFor,
  type ModelRole,
  type ModelSpec,
  type ProviderId,
} from "./models.ts";
import {
  AccessDeniedError,
  callGemini,
  ProviderError,
  RateLimitedError,
  TruncatedError,
} from "./gemini.ts";
import { callNvidia } from "./nvidia.ts";
import {
  RouterExhaustedError,
  type AttemptRecord,
  type CompletionRequest,
  type CompletionResult,
  type UsageStore,
} from "./types.ts";

export * from "./models.ts";
export * from "./types.ts";

export interface RouterOptions {
  geminiApiKey: string;
  /** NVIDIA NIM. Optional: without it the cascades fall through to Gemini. */
  nvidiaApiKey?: string;
  usage: UsageStore;
  /**
   * Stop using a model once it has spent this share of its published daily
   * quota, leaving headroom for the roles further down the cascade. 0.9 means
   * a model retires at 1350 of its 1500 daily requests.
   */
  quotaSafetyMargin?: number;
  onEvent?: (e: RouterEvent) => void;
}

export interface RouterEvent {
  level: "info" | "warn";
  model: string;
  role: ModelRole;
  outcome: AttemptRecord["outcome"];
  detail?: string;
}

/**
 * Picks a model, calls it, records what it cost, and moves down the cascade
 * when a model is unavailable.
 *
 * Design note: a model being rate-limited or out of quota is *normal traffic*,
 * not an error. Free tiers are the whole premise of this project, so the
 * router treats exhaustion as routing information and only fails when every
 * model in the cascade is unusable.
 */
export class Router {
  private readonly key: string;
  private readonly nvidiaKey: string;
  private readonly usage: UsageStore;
  private readonly margin: number;
  private readonly onEvent: (e: RouterEvent) => void;

  /** model id -> timestamps of recent calls, for the per-minute gate. */
  private readonly recentCalls = new Map<string, number[]>();
  /** model id -> epoch ms before which we should not retry it. */
  private readonly cooldownUntil = new Map<string, number>();
  /**
   * model id -> the day (YYYY-MM-DD) its daily quota ran out.
   *
   * Discovered from Google's own 429, never guessed. The published free-tier
   * numbers turned out to be off by two orders of magnitude — the catalogue
   * said 1500 requests/day and the API replied "limit: 20" — so the server is
   * the only trustworthy source, and one wasted call per model per day is a
   * small price for never being wrong about it.
   */
  private readonly exhaustedOn = new Map<string, string>();
  /**
   * provider -> until when its key is known to be refused, and why.
   *
   * Tracked per provider, not per model: every model behind one key shares
   * the refusal, and a refusal does not fix itself between two ticks.
   */
  private readonly deniedUntil = new Map<ProviderId, { until: number; detail: string }>();

  constructor(opts: RouterOptions) {
    if (!opts.geminiApiKey) {
      throw new Error(
        "GEMINI_API_KEY is missing. Fill it in .env.local — see the file for the link.",
      );
    }
    this.key = opts.geminiApiKey;
    this.nvidiaKey = opts.nvidiaApiKey ?? "";
    this.usage = opts.usage;
    this.margin = opts.quotaSafetyMargin ?? 0.9;
    this.onEvent = opts.onEvent ?? (() => {});
  }

  async complete(req: CompletionRequest): Promise<CompletionResult> {
    const cascade = CASCADES[req.role];
    const attempts: AttemptRecord[] = [];
    // 240 s, not 120: DeepSeek V4 Pro took ~49 s on a real 4k-token prompt,
    // and a cold NIM instance plus a long reasoning pass pushed a coder call
    // past two minutes. A timeout that fires on a healthy model reads as an
    // outage, and outages were being charged to the agents.
    const timeoutMs = req.timeoutMs ?? 240_000;

    for (const modelId of cascade) {
      const spec = specFor(modelId);

      const cooling = this.cooldownUntil.get(modelId) ?? 0;
      if (Date.now() < cooling) {
        this.note(attempts, req.role, modelId, "rate_limited", "in cooldown");
        continue;
      }

      // No NVIDIA key: skip its models rather than failing on each one.
      // The cascade then degrades to Gemini on its own.
      if (spec.provider === "nvidia" && !this.nvidiaKey) {
        this.note(attempts, req.role, modelId, "skipped", "NVIDIA_API_KEY absente");
        continue;
      }

      const denied = this.deniedUntil.get(spec.provider);
      if (denied && denied.until > Date.now()) {
        this.note(attempts, req.role, modelId, "skipped", `${spec.provider} : clé refusée`);
        continue;
      }

      if (this.exhaustedOn.get(modelId) === today()) {
        const spent = await this.usage.requestsToday(modelId);
        this.note(
          attempts,
          req.role,
          modelId,
          "quota_exhausted",
          `quota journalier atteint (${spent} appels aujourd'hui)`,
        );
        continue;
      }

      // Per-minute gate. Waiting a few seconds costs nothing here: agents work
      // in the background and nobody is watching a spinner.
      const waitMs = this.msUntilSlotFree(modelId, spec.rpm);
      if (waitMs > 0) {
        if (waitMs > 20_000) {
          this.note(attempts, req.role, modelId, "rate_limited", `${waitMs}ms wait`);
          continue;
        }
        await sleep(waitMs);
      }

      const started = Date.now();
      try {
        this.markCall(modelId);

        let budget = req.maxOutputTokens ?? DEFAULT_OUTPUT_TOKENS;
        let out;
        try {
          out = await this.invoke(spec, req, budget, timeoutMs);
        } catch (err) {
          // The model thought its way through the whole budget without
          // answering. Switching models would hit the same wall, so give this
          // one more room instead — once.
          if (!(err instanceof TruncatedError)) throw err;
          budget = Math.min(budget * 3, MAX_OUTPUT_TOKENS);
          this.note(attempts, req.role, modelId, "error", `${err.message} — nouvel essai à ${budget} tokens`);
          this.markCall(modelId);
          out = await this.invoke(spec, req, budget, timeoutMs);
        }

        await this.usage.record({
          provider: spec.provider,
          model: modelId,
          tokensIn: out.tokensIn,
          tokensOut: out.tokensOut,
        });

        this.note(attempts, req.role, modelId, "ok");
        return {
          text: out.text,
          model: modelId,
          provider: spec.provider,
          tokensIn: out.tokensIn,
          tokensOut: out.tokensOut,
          latencyMs: Date.now() - started,
          attempts,
        };
      } catch (err) {
        const detail = err instanceof Error ? err.message : String(err);

        // Every model here shares one key and one Google Cloud project, so a
        // refusal is not a reason to try the next one — it is the same refusal
        // three times over, ending in a "no model available" message that
        // hides the real cause.
        if (err instanceof AccessDeniedError) {
          this.note(attempts, req.role, modelId, "error", detail);
          await this.usage.record({
            provider: spec.provider,
            model: modelId,
            tokensIn: 0,
            tokensOut: 0,
            error: detail.slice(0, 300),
          });
          // Every model behind this key shares the refusal, so the whole
          // provider is set aside — ten minutes, not one 403 per tick. The
          // cascade then moves on to the other provider rather than stopping:
          // that rule dates from when Gemini was the only one, and with two
          // it meant a refused Gemini key left NVIDIA unasked.
          this.deniedUntil.set(spec.provider, {
            until: Date.now() + DENIED_COOLDOWN_MS,
            detail,
          });
          continue;
        }

        if (err instanceof RateLimitedError) {
          // Believe the server over our own accounting.
          if (err.isDaily) {
            // Done until tomorrow. Every further call today is a round trip
            // that can only return the same 429.
            this.exhaustedOn.set(modelId, today());
            this.note(attempts, req.role, modelId, "quota_exhausted", detail);
          } else {
            this.cooldownUntil.set(
              modelId,
              Date.now() + (err.retryAfterMs ?? 60_000),
            );
            this.note(attempts, req.role, modelId, "rate_limited", detail);
          }
        } else {
          this.note(attempts, req.role, modelId, "error", detail);
        }

        await this.usage.record({
          provider: spec.provider,
          model: modelId,
          tokensIn: 0,
          tokensOut: 0,
          error: detail.slice(0, 300),
        });

        // A model that has vanished from the API must not stop the system.
        // Free-tier catalogues change without notice; the next model runs.
        if (err instanceof ProviderError && err.status === 404) {
          this.cooldownUntil.set(modelId, Date.now() + 6 * 60 * 60 * 1000);
        } else if (
          err instanceof ProviderError &&
          (err.status === undefined
            ? /timed out|network failure/i.test(err.message)
            : err.status >= 500)
        ) {
          // Overloaded (503), gateway (502/504) or timed out: the provider is
          // struggling. Retrying on the next tick — every 15 s — hammers it
          // while it recovers and turns one outage into a stream of failed
          // attempts. 90 s of cooldown lets the cascade route around it.
          this.cooldownUntil.set(modelId, Date.now() + 90_000);
        }
      }
    }

    // A refused key is the one failure a human must act on, so it outranks
    // "no model available" and names the provider — the old message blamed
    // Gemini for every refusal, including NVIDIA's.
    const refused = [...this.deniedUntil.entries()].filter(([, d]) => d.until > Date.now());
    if (refused.length > 0) throw new Error(refusedMessage(refused));

    throw new RouterExhaustedError(req.role, attempts);
  }

  /**
   * Can anything in this role's cascade answer right now, without trying?
   *
   * The dispatcher asks before claiming a task. When every model is cooling
   * down, claiming would run the cascade to exhaustion without a single
   * network call, requeue the task, and repeat next tick — a stream of
   * provider_error events about an outage the router already knows about.
   */
  available(role: ModelRole): boolean {
    const now = Date.now();
    return CASCADES[role].some((modelId) => {
      const spec = MODELS[modelId];
      if (!spec) return false;
      if (spec.provider === "nvidia" && !this.nvidiaKey) return false;
      if ((this.cooldownUntil.get(modelId) ?? 0) > now) return false;
      if ((this.deniedUntil.get(spec.provider)?.until ?? 0) > now) return false;
      return this.exhaustedOn.get(modelId) !== today();
    });
  }

  /**
   * One call to one model, routed to its provider's adapter.
   *
   * Both adapters throw the same error types, so everything above this line
   * reasons about failures rather than about vendors — which is what lets a
   * cascade mix providers freely.
   */
  private invoke(
    spec: ModelSpec,
    req: CompletionRequest,
    maxOutputTokens: number,
    timeoutMs: number,
  ) {
    if (spec.provider === "nvidia") {
      return callNvidia({
        apiKey: this.nvidiaKey,
        model: spec.id,
        system: req.system,
        messages: req.messages,
        maxOutputTokens,
        temperature: req.temperature ?? 0.2,
        timeoutMs,
      });
    }
    return callGemini({
      apiKey: this.key,
      model: spec.id,
      system: req.system,
      messages: req.messages,
      maxOutputTokens,
      temperature: req.temperature ?? 0.2,
      json: req.json ?? false,
      timeoutMs,
    });
  }

  /** Snapshot for the dashboard: what is left today, per model. */
  async budgetSnapshot(): Promise<
    Array<{ model: string; label: string; used: number; limit: number | null }>
  > {
    return Promise.all(
      Object.values(MODELS).map(async (spec) => ({
        model: spec.id,
        label: spec.label,
        used: await this.usage.requestsToday(spec.id),
        limit: spec.dailyRequests,
      })),
    );
  }

  private msUntilSlotFree(modelId: string, rpm: number): number {
    const now = Date.now();
    const window = 60_000;
    const calls = (this.recentCalls.get(modelId) ?? []).filter(
      (t) => now - t < window,
    );
    this.recentCalls.set(modelId, calls);
    if (calls.length < rpm) return 0;
    const oldest = calls[0] ?? now;
    return Math.max(0, window - (now - oldest));
  }

  private markCall(modelId: string): void {
    const calls = this.recentCalls.get(modelId) ?? [];
    calls.push(Date.now());
    this.recentCalls.set(modelId, calls);
  }

  private note(
    attempts: AttemptRecord[],
    role: ModelRole,
    model: string,
    outcome: AttemptRecord["outcome"],
    detail?: string,
  ): void {
    attempts.push({ model, outcome, ...(detail ? { detail } : {}) });
    this.onEvent({
      level: outcome === "ok" ? "info" : "warn",
      model,
      role,
      outcome,
      ...(detail ? { detail } : {}),
    });
  }
}

/** For tests and local runs; the worker uses a Supabase-backed store. */
export class InMemoryUsageStore implements UsageStore {
  private readonly counts = new Map<string, number>();

  async requestsToday(model: string): Promise<number> {
    return this.counts.get(model) ?? 0;
  }

  async record(entry: { model: string }): Promise<void> {
    this.counts.set(entry.model, (this.counts.get(entry.model) ?? 0) + 1);
  }
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const today = (): string => new Date().toISOString().slice(0, 10);

/**
 * Gemini 3.x reasons before answering, and those tokens come out of the same
 * allowance as the reply. A budget sized only for the answer produces
 * finishReason MAX_TOKENS and an empty body, so the default is deliberately
 * generous.
 */
const DEFAULT_OUTPUT_TOKENS = 16_384;
const MAX_OUTPUT_TOKENS = 48_000;

/** How long a provider whose key was refused is left alone. */
const DENIED_COOLDOWN_MS = 10 * 60 * 1000;

function refusedMessage(refused: Array<[ProviderId, { detail: string }]>): string {
  return refused
    .map(([provider, { detail }]) =>
      provider === "gemini"
        ? "Gemini refuse la clé ou son projet Google (HTTP 403/401). Ce n'est " +
          "pas un problème de quota : tous les modèles Gemini partagent ce " +
          "projet. Vérifie la clé sur https://aistudio.google.com/apikey, ou " +
          "crée-en une dans un nouveau projet. Détail : " +
          detail.slice(0, 300)
        : "NVIDIA refuse la clé (HTTP 401/403). Régénère-la sur " +
          "https://build.nvidia.com/settings/api-keys. Détail : " +
          detail.slice(0, 300),
    )
    .join(" | ");
}
