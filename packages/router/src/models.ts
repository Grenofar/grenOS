/**
 * Model catalogue and per-role cascades.
 *
 * Two providers with very different shapes:
 *
 *   NVIDIA NIM  — 40 requests per MINUTE, one account key for 80+ models,
 *                 but a finite credit pool. Leads every cascade.
 *   Gemini      — ~20 requests per DAY per model, free forever. The floor the
 *                 system falls back to when credits run out.
 *
 * That asymmetry is the whole design (D-016, D-017). Gemini alone allowed
 * roughly twenty agent tasks a day: enough to prove the factory works, nowhere
 * near enough to build an OS with it.
 *
 * Every model listed here answered a real call. `deepseek-v4-flash-0731` is
 * deliberately absent: it returned HTTP 504, and a cascade built on a model
 * that does not answer is worse than a shorter cascade.
 *
 * On `dailyRequests`: a DISPLAY HINT, never a limit the router enforces.
 * Published free-tier figures were wrong by two orders of magnitude — blogs
 * said 1500/day, the live API answered "limit: 20" — so the router calls and
 * treats the provider's own 429 as the authority (see `exhaustedOn` in
 * index.ts). `null` means the provider meters credits rather than requests, so
 * no gauge could be honest.
 *
 * Adding a provider means entries here plus one adapter. Nothing in the agent
 * prompts names a model: `agents/*.md` declares a `model_role`, which is a key
 * into CASCADES.
 */

export type ProviderId = "gemini" | "nvidia";

export interface ModelSpec {
  provider: ProviderId;
  /** Exact API model id. */
  id: string;
  /** Human label for the dashboard. */
  label: string;
  /** Input context ceiling, in tokens. */
  contextTokens: number;
  /** Observed free-tier requests/day, or null when metered in credits. */
  dailyRequests: number | null;
  /** Requests per minute the provider allows. */
  rpm: number;
  /** Measured round trip on a trivial prompt, for cascade ordering. */
  latencyMs: number;
  /**
   * Extra fields for the request body. On NIM, the models that reason before
   * answering do so by default, and it is what made them slow: on 2026-09-16
   * Nemotron 3 Super took two minutes for a function and ran out of tokens
   * thinking, then answered the same prompt in 5 s with thinking off.
   */
  extra?: Record<string, unknown>;
  /** A longer ceiling for a model that is slow but worth waiting for. */
  timeoutMs?: number;
}

// Reasoning off: on NIM the reasoning models think before every answer by
// default. Measured 2026-09-16 on the same Rust task: Nemotron 3 Super 125 s
// and no code with it, 5 s with it off; DeepSeek V4 Flash 2.4 s with it off.
const NO_THINKING = { chat_template_kwargs: { thinking: false, enable_thinking: false } };

export const MODELS: Record<string, ModelSpec> = {
  // --- NVIDIA NIM ----------------------------------------------------------
  // DeepSeek V4 Pro led the Coder and the Architect until NVIDIA retired it on
  // 2026-09-14 (HTTP 410). Its successor on the account is V4 Flash.
  "deepseek-ai/deepseek-v4-flash-0731": {
    provider: "nvidia",
    id: "deepseek-ai/deepseek-v4-flash-0731",
    label: "DeepSeek V4 Flash",
    contextTokens: 128_000,
    dailyRequests: null,
    rpm: 40,
    latencyMs: 2400,
    extra: NO_THINKING,
  },
  "nvidia/nemotron-3-super-120b-a12b": {
    provider: "nvidia",
    id: "nvidia/nemotron-3-super-120b-a12b",
    label: "Nemotron 3 Super",
    contextTokens: 128_000,
    dailyRequests: null,
    rpm: 40,
    latencyMs: 5300,
    extra: NO_THINKING,
  },
  // Slow on 2026-09-16 (no answer within 180 s, reasoning off or not), but a
  // strong model and a third opinion in a panel, whose grace period means it
  // never holds a task back: a long ceiling, and never first.
  "moonshotai/kimi-k3": {
    provider: "nvidia",
    id: "moonshotai/kimi-k3",
    label: "Kimi K3",
    contextTokens: 200_000,
    dailyRequests: null,
    rpm: 40,
    latencyMs: 180_000,
    extra: NO_THINKING,
    timeoutMs: 600_000,
  },

  // --- Google Gemini -------------------------------------------------------
  // The permanent floor: tiny daily quota, but it never runs out of credits.
  // Quota is counted per model, so three models is three separate budgets.
  "gemini-3.8-flash": {
    provider: "gemini",
    id: "gemini-3.8-flash",
    label: "Gemini 3.8 Flash",
    contextTokens: 1_000_000,
    dailyRequests: 20,
    rpm: 15,
    latencyMs: 3000,
  },
  "gemini-3.7-flash": {
    provider: "gemini",
    id: "gemini-3.7-flash",
    label: "Gemini 3.7 Flash",
    contextTokens: 1_000_000,
    dailyRequests: 20,
    rpm: 15,
    latencyMs: 3000,
  },
  "gemini-3.6-flash": {
    provider: "gemini",
    id: "gemini-3.6-flash",
    label: "Gemini 3.6 Flash",
    contextTokens: 1_000_000,
    dailyRequests: 20,
    rpm: 15,
    latencyMs: 3000,
  },
};

export type ModelRole = "master" | "architect" | "coder" | "tester";

/**
 * Ordered preference per role. The router walks the list and takes the first
 * model that is neither out of quota nor rate-limited; a panel (executor.ts)
 * takes the first few NVIDIA ones side by side.
 *
 * Measured on 2026-09-16, when the old leader had been retired: only DeepSeek
 * V4 Flash and Nemotron 3 Super answered a real coding prompt within three
 * minutes (GLM 5.3, GLM 5.3 Flash, Nemotron 3 Ultra, Gemma 4 and Kimi K3 did
 * not). Neither wrote the test function right on its first try, which is why
 * writers work as a panel with a local build, rather than on one "best" model.
 *
 * The floor has two floors. On 2026-09-11 the NIM models timed out or answered
 * 503 while gemini-3.8-flash refused for "high demand", all at once, and every
 * cascade was down for most of an hour. Each Gemini model has its own
 * capacity and its own daily quota, so gemini-3.7-flash is a real second
 * floor.
 */
export const CASCADES: Record<ModelRole, string[]> = {
  master: [
    "nvidia/nemotron-3-super-120b-a12b",
    "deepseek-ai/deepseek-v4-flash-0731",
    "gemini-3.8-flash",
    "gemini-3.7-flash",
  ],
  architect: [
    "deepseek-ai/deepseek-v4-flash-0731",
    "nvidia/nemotron-3-super-120b-a12b",
    "moonshotai/kimi-k3",
    "gemini-3.8-flash",
    "gemini-3.7-flash",
  ],
  coder: [
    "deepseek-ai/deepseek-v4-flash-0731",
    "nvidia/nemotron-3-super-120b-a12b",
    "moonshotai/kimi-k3",
    "gemini-3.8-flash",
    "gemini-3.7-flash",
  ],
  tester: [
    "nvidia/nemotron-3-super-120b-a12b",
    "deepseek-ai/deepseek-v4-flash-0731",
    "gemini-3.6-flash",
  ],
};

export function specFor(modelId: string): ModelSpec {
  const spec = MODELS[modelId];
  if (!spec) throw new Error(`Unknown model: ${modelId}`);
  return spec;
}
