import type { ChatMessage } from "./types.ts";

const BASE = "https://generativelanguage.googleapis.com/v1beta/models";

export interface GeminiCallOptions {
  apiKey: string;
  model: string;
  system: string;
  messages: ChatMessage[];
  maxOutputTokens: number;
  temperature: number;
  json: boolean;
  timeoutMs: number;
}

export interface GeminiCallResult {
  text: string;
  tokensIn: number;
  tokensOut: number;
}

// Fields are declared then assigned rather than using constructor parameter
// properties: the worker runs this TypeScript without a build step, and Node's
// strip-only mode rejects that syntax.

/** Provider said "slow down" or "you are out of quota" — try the next model. */
export class RateLimitedError extends Error {
  readonly retryAfterMs: number | null;
  /**
   * True when the message names a *daily* quota rather than a per-minute one.
   * The two need opposite responses: waiting 30 seconds clears a per-minute
   * limit, while a daily limit means this model is done until tomorrow and
   * every further call is a wasted round trip.
   */
  readonly isDaily: boolean;

  constructor(retryAfterMs: number | null, message: string, isDaily = false) {
    super(message);
    this.name = "RateLimitedError";
    this.retryAfterMs = retryAfterMs;
    this.isDaily = isDaily;
  }
}

/**
 * The model spent its output budget before producing an answer.
 *
 * Gemini 3.x models think before they write, and those reasoning tokens come
 * out of the same `maxOutputTokens` allowance. Ask for 50 tokens and you get
 * finishReason MAX_TOKENS with an empty body — the model reasoned and never
 * got to speak. Separate from ProviderError because the fix is to give the
 * same model more room, not to switch models.
 */
export class TruncatedError extends Error {
  readonly thoughtTokens: number;

  constructor(message: string, thoughtTokens: number) {
    super(message);
    this.name = "TruncatedError";
    this.thoughtTokens = thoughtTokens;
  }
}

/** Anything else: transport failure, 5xx, malformed response, blocked output. */
export class ProviderError extends Error {
  readonly status: number | undefined;

  constructor(message: string, status?: number) {
    super(message);
    this.name = "ProviderError";
    this.status = status;
  }
}

/**
 * The key or its Google Cloud project is refused outright.
 *
 * Distinct from ProviderError because every model in the cascade shares one
 * key and one project: if the project is denied, trying the next model is
 * three identical 403s and a misleading "no model available" at the end. This
 * stops the cascade at the first refusal and says what actually happened.
 */
export class AccessDeniedError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AccessDeniedError";
  }
}

/**
 * One call to one Gemini model. No retries here on purpose: retry policy is the
 * router's business, because "retry" for us usually means "different model",
 * not "same model again".
 */
export async function callGemini(
  opts: GeminiCallOptions,
): Promise<GeminiCallResult> {
  const body: Record<string, unknown> = {
    systemInstruction: { parts: [{ text: opts.system }] },
    contents: opts.messages.map((m) => ({
      role: m.role === "assistant" ? "model" : "user",
      parts: [{ text: m.content }],
    })),
    generationConfig: {
      maxOutputTokens: opts.maxOutputTokens,
      temperature: opts.temperature,
      ...(opts.json ? { responseMimeType: "application/json" } : {}),
    },
  };

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), opts.timeoutMs);

  let res: Response;
  try {
    res = await fetch(
      `${BASE}/${encodeURIComponent(opts.model)}:generateContent`,
      {
        method: "POST",
        headers: {
          "content-type": "application/json",
          // Header rather than query string: keeps the key out of URLs, which
          // are the thing that ends up in logs and error reports.
          "x-goog-api-key": opts.apiKey,
        },
        body: JSON.stringify(body),
        signal: controller.signal,
      },
    );
  } catch (err) {
    const aborted = err instanceof Error && err.name === "AbortError";
    throw new ProviderError(
      aborted ? `Timed out after ${opts.timeoutMs}ms` : `Network failure: ${err}`,
    );
  } finally {
    clearTimeout(timer);
  }

  if (res.status === 429) {
    const body = await briefly(res);
    throw new RateLimitedError(parseRetryAfter(res), body, mentionsDailyQuota(body));
  }
  if (res.status === 403 || res.status === 401) {
    throw new AccessDeniedError(await briefly(res));
  }
  if (!res.ok) {
    // 5xx and 503 "model overloaded" are transient; the cascade handles them
    // the same way it handles a rate limit: move on to the next model.
    throw new ProviderError(await briefly(res), res.status);
  }

  const data = (await res.json()) as GeminiResponse;

  const candidate = data.candidates?.[0];
  if (!candidate) {
    const reason = data.promptFeedback?.blockReason ?? "no candidate returned";
    throw new ProviderError(`Empty response: ${reason}`);
  }

  const thoughts = data.usageMetadata?.thoughtsTokenCount ?? 0;

  if (candidate.finishReason === "MAX_TOKENS") {
    throw new TruncatedError(
      `Budget de sortie épuisé (${thoughts} tokens de raisonnement consommés avant la réponse)`,
      thoughts,
    );
  }
  if (candidate.finishReason && candidate.finishReason !== "STOP") {
    throw new ProviderError(`Generation stopped: ${candidate.finishReason}`);
  }

  const text = (candidate.content?.parts ?? [])
    .map((p) => p.text ?? "")
    .join("")
    .trim();

  if (!text) throw new ProviderError("Model returned empty text");

  return {
    text,
    tokensIn: data.usageMetadata?.promptTokenCount ?? 0,
    // Reasoning tokens are billed and count against the budget, so they belong
    // in the output total. Leaving them out makes every mission look cheaper
    // than it is, and the Master would abort far too late.
    tokensOut: (data.usageMetadata?.candidatesTokenCount ?? 0) + thoughts,
  };
}

/**
 * Distinguish a daily quota from a per-minute one. Google phrases it in the
 * metric name, e.g. "generate_content_free_tier_requests" with a per-day
 * limit, versus "...per_minute".
 */
function mentionsDailyQuota(body: string): boolean {
  const t = body.toLowerCase();
  if (t.includes("per_minute") || t.includes("perminute")) return false;
  return (
    t.includes("free_tier_requests") ||
    t.includes("per_day") ||
    t.includes("perday") ||
    t.includes("requests per day")
  );
}

function parseRetryAfter(res: Response): number | null {
  const header = res.headers.get("retry-after");
  if (!header) return null;
  const seconds = Number(header);
  return Number.isFinite(seconds) ? seconds * 1000 : null;
}

/** Error bodies can be enormous; keep enough to diagnose, not enough to flood. */
async function briefly(res: Response): Promise<string> {
  let body = "";
  try {
    body = (await res.text()).slice(0, 400);
  } catch {
    body = "<unreadable body>";
  }
  return `HTTP ${res.status}: ${body}`;
}

interface GeminiResponse {
  candidates?: Array<{
    content?: { parts?: Array<{ text?: string }> };
    finishReason?: string;
  }>;
  promptFeedback?: { blockReason?: string };
  usageMetadata?: {
    promptTokenCount?: number;
    candidatesTokenCount?: number;
    /** Reasoning tokens on models that think before answering. */
    thoughtsTokenCount?: number;
  };
}
