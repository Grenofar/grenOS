import { CASCADES, MODELS, specFor, type ModelRole } from "./models.ts";
import {
  AccessDeniedError,
  callGemini,
  ProviderError,
  RateLimitedError,
  TruncatedError,
} from "./gemini.ts";
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

  constructor(opts: RouterOptions) {
    if (!opts.geminiApiKey) {
      throw new Error(
        "GEMINI_API_KEY is missing. Fill it in .env.local — see the file for the link.",
      );
    }
    this.key = opts.geminiApiKey;
    this.usage = opts.usage;
    this.margin = opts.quotaSafetyMargin ?? 0.9;
    this.onEvent = opts.onEvent ?? (() => {});
  }

  async complete(req: CompletionRequest): Promise<CompletionResult> {
    const cascade = CASCADES[req.role];
    const attempts: AttemptRecord[] = [];
    const timeoutMs = req.timeoutMs ?? 120_000;

    for (const modelId of cascade) {
      const spec = specFor(modelId);

      const cooling = this.cooldownUntil.get(modelId) ?? 0;
      if (Date.now() < cooling) {
        this.note(attempts, req.role, modelId, "rate_limited", "in cooldown");
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
          out = await callGemini({
            apiKey: this.key,
            model: spec.id,
            system: req.system,
            messages: req.messages,
            maxOutputTokens: budget,
            temperature: req.temperature ?? 0.2,
            json: req.json ?? false,
            timeoutMs,
          });
        } catch (err) {
          // The model thought its way through the whole budget without
          // answering. Switching models would hit the same wall, so give this
          // one more room instead — once.
          if (!(err instanceof TruncatedError)) throw err;
          budget = Math.min(budget * 3, MAX_OUTPUT_TOKENS);
          this.note(attempts, req.role, modelId, "error", `${err.message} — nouvel essai à ${budget} tokens`);
          this.markCall(modelId);
          out = await callGemini({
            apiKey: this.key,
            model: spec.id,
            system: req.system,
            messages: req.messages,
            maxOutputTokens: budget,
            temperature: req.temperature ?? 0.2,
            json: req.json ?? false,
            timeoutMs,
          });
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
          throw new Error(
            "Gemini refuse la clé ou son projet Google (HTTP 403/401). " +
              "Ce n'est pas un problème de quota : tous les modèles partagent " +
              "le même projet, donc la cascade n'y changera rien. Vérifie la " +
              "clé sur https://aistudio.google.com/apikey, ou crée-en une dans " +
              "un nouveau projet. Détail : " +
              detail.slice(0, 300),
          );
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
        }
      }
    }

    throw new RouterExhaustedError(req.role, attempts);
  }

  /** Snapshot for the dashboard: what is left today, per model. */
  async budgetSnapshot(): Promise<
    Array<{ model: string; label: string; used: number; limit: number }>
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
