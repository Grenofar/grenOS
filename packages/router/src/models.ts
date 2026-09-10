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
}

export const MODELS: Record<string, ModelSpec> = {
  // --- NVIDIA NIM ----------------------------------------------------------
  "deepseek-ai/deepseek-v4-pro-0813": {
    provider: "nvidia",
    id: "deepseek-ai/deepseek-v4-pro-0813",
    label: "DeepSeek V4 Pro",
    contextTokens: 128_000,
    dailyRequests: null,
    rpm: 40,
    latencyMs: 4900,
  },
  "nvidia/nemotron-3-super-120b-a12b": {
    provider: "nvidia",
    id: "nvidia/nemotron-3-super-120b-a12b",
    label: "Nemotron 3 Super",
    contextTokens: 128_000,
    dailyRequests: null,
    rpm: 40,
    latencyMs: 2500,
  },
  "moonshotai/kimi-k3": {
    provider: "nvidia",
    id: "moonshotai/kimi-k3",
    label: "Kimi K3",
    contextTokens: 200_000,
    dailyRequests: null,
    rpm: 40,
    latencyMs: 14400,
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
 * model that is neither out of quota nor rate-limited.
 *
 * Measured latency drove the ordering as much as capability. The Master runs
 * on every state change, so it gets the fast one; the Coder produces the diff
 * everything downstream depends on, so it gets the strongest; the Architect
 * runs rarely and thinks longest, so the 14-second model is fine there.
 */
export const CASCADES: Record<ModelRole, string[]> = {
  master: [
    "nvidia/nemotron-3-super-120b-a12b",
    "deepseek-ai/deepseek-v4-pro-0813",
    "gemini-3.8-flash",
  ],
  architect: [
    "deepseek-ai/deepseek-v4-pro-0813",
    "moonshotai/kimi-k3",
    "gemini-3.8-flash",
  ],
  coder: [
    "deepseek-ai/deepseek-v4-pro-0813",
    "nvidia/nemotron-3-super-120b-a12b",
    "gemini-3.8-flash",
  ],
  tester: [
    "nvidia/nemotron-3-super-120b-a12b",
    "deepseek-ai/deepseek-v4-pro-0813",
    "gemini-3.6-flash",
  ],
};

export function specFor(modelId: string): ModelSpec {
  const spec = MODELS[modelId];
  if (!spec) throw new Error(`Unknown model: ${modelId}`);
  return spec;
}
