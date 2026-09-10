import {
  AccessDeniedError,
  ProviderError,
  RateLimitedError,
  TruncatedError,
} from "./gemini.ts";
import type { ChatMessage } from "./types.ts";

/**
 * NVIDIA NIM (build.nvidia.com).
 *
 * One account-wide key unlocks the whole catalogue, and every model exposes
 * the same OpenAI-shaped /v1/chat/completions endpoint — so this adapter is
 * short, and adding a model is one entry in the catalogue.
 *
 * Why it matters here: the Gemini free tier allows about 20 requests per model
 * per day (D-016). NVIDIA allows 40 per *minute*. That is the difference
 * between proving the factory works and actually building an OS with it, so
 * NVIDIA leads every cascade and Gemini becomes the fallback that never runs
 * out permanently.
 *
 * Error types are shared with the Gemini adapter on purpose: the router
 * reasons about failures, not about vendors.
 */

const BASE = "https://integrate.api.nvidia.com/v1";

export interface NvidiaCallOptions {
  apiKey: string;
  model: string;
  system: string;
  messages: ChatMessage[];
  maxOutputTokens: number;
  temperature: number;
  timeoutMs: number;
}

export interface NvidiaCallResult {
  text: string;
  tokensIn: number;
  tokensOut: number;
}

export async function callNvidia(
  opts: NvidiaCallOptions,
): Promise<NvidiaCallResult> {
  const body = {
    model: opts.model,
    messages: [
      { role: "system", content: opts.system },
      ...opts.messages.map((m) => ({
        role: m.role === "assistant" ? "assistant" : "user",
        content: m.content,
      })),
    ],
    max_tokens: opts.maxOutputTokens,
    temperature: opts.temperature,
    // Deliberately no response_format: support for JSON mode varies across an
    // 80-model catalogue, and a model that rejects it fails the whole call.
    // The envelope parser already recovers JSON from a code fence or a
    // surrounding sentence, so robustness lives in one place instead of in a
    // per-model capability table that would rot.
  };

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), opts.timeoutMs);

  let res: Response;
  try {
    res = await fetch(`${BASE}/chat/completions`, {
      method: "POST",
      headers: {
        authorization: `Bearer ${opts.apiKey}`,
        "content-type": "application/json",
        accept: "application/json",
      },
      body: JSON.stringify(body),
      signal: controller.signal,
    });
  } catch (err) {
    const aborted = err instanceof Error && err.name === "AbortError";
    throw new ProviderError(
      aborted
        ? `Timed out after ${opts.timeoutMs}ms — les gros modèles NIM démarrent lentement à froid`
        : `Network failure: ${err}`,
    );
  } finally {
    clearTimeout(timer);
  }

  if (res.status === 429) {
    const detail = await briefly(res);
    // NIM limits requests per minute and spends a finite credit pool. Only the
    // per-minute limit clears on its own, so the two must not be confused: a
    // per-minute wait treated as exhaustion retires a working model until
    // tomorrow, and exhaustion treated as a wait retries forever.
    //
    // Order matters. "Quota exceeded ... requests_per_minute" contains the
    // word quota, so the per-minute test has to run first — checking credits
    // first classified every transient limit as permanent.
    const perMinute = /per[_ -]?minute|per[_ -]?min|rpm/i.test(detail);
    const outOfCredits =
      !perMinute && /credit|exhaust|per[_ -]?day|daily|free_tier/i.test(detail);
    throw new RateLimitedError(retryAfter(res), detail, outOfCredits);
  }
  if (res.status === 401 || res.status === 403) {
    throw new AccessDeniedError(await briefly(res));
  }
  if (!res.ok) {
    throw new ProviderError(await briefly(res), res.status);
  }

  const data = (await res.json()) as NvidiaResponse;
  const choice = data.choices?.[0];
  if (!choice) throw new ProviderError("Aucune réponse renvoyée");

  const usage = data.usage ?? {};
  const tokensIn = usage.prompt_tokens ?? 0;
  const tokensOut = usage.completion_tokens ?? 0;

  if (choice.finish_reason === "length") {
    throw new TruncatedError(
      `Budget de sortie épuisé (${tokensOut} tokens produits)`,
      tokensOut,
    );
  }

  const text = (choice.message?.content ?? "").trim();
  if (!text) {
    // Reasoning models can return an empty content with the thinking in a
    // separate field. That is a truncation in disguise: the model reasoned and
    // never answered, and more room is the fix.
    if (choice.message?.reasoning_content) {
      throw new TruncatedError(
        "Le modèle a raisonné sans produire de réponse",
        tokensOut,
      );
    }
    throw new ProviderError("Réponse vide");
  }

  return { text, tokensIn, tokensOut };
}

function retryAfter(res: Response): number | null {
  const header = res.headers.get("retry-after");
  if (!header) return null;
  const seconds = Number(header);
  return Number.isFinite(seconds) ? seconds * 1000 : null;
}

async function briefly(res: Response): Promise<string> {
  let body = "";
  try {
    body = (await res.text()).slice(0, 400);
  } catch {
    body = "<unreadable body>";
  }
  return `HTTP ${res.status}: ${body}`;
}

interface NvidiaResponse {
  choices?: Array<{
    message?: { content?: string; reasoning_content?: string };
    finish_reason?: string;
  }>;
  usage?: { prompt_tokens?: number; completion_tokens?: number };
}
