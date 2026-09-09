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

  constructor(retryAfterMs: number | null, message: string) {
    super(message);
    this.name = "RateLimitedError";
    this.retryAfterMs = retryAfterMs;
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
    throw new RateLimitedError(parseRetryAfter(res), await briefly(res));
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

  // MAX_TOKENS means the JSON envelope is very likely truncated, so treat it
  // as a failure rather than handing the caller a half-object to parse.
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
    tokensOut: data.usageMetadata?.candidatesTokenCount ?? 0,
  };
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
  };
}
