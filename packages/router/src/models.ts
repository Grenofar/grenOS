/**
 * Model catalogue and per-role cascades.
 *
 * Only Google Gemini is wired for now. Every free-tier quota Google publishes
 * is counted *per model*, so spreading the same workload across 3.6, 3.7 and
 * 3.8 Flash multiplies the effective daily budget rather than sharing one pool:
 *
 *     1 model  = ~1500 requests/day
 *     3 models = ~4500 requests/day
 *
 * That is the entire reason the cascades below list sibling models rather than
 * repeating the newest one. A cascade step is not only a failure fallback, it
 * is also a quota valve.
 *
 * Adding a provider later means adding entries here and one adapter. Nothing
 * in the agent prompts refers to a model name: `agents/*.md` declares a
 * `model_role`, which is a key into CASCADES.
 */

export type ProviderId = "gemini";

export interface ModelSpec {
  provider: ProviderId;
  /** Exact API model id. */
  id: string;
  /** Human label for the dashboard. */
  label: string;
  /** Input context ceiling, in tokens. */
  contextTokens: number;
  /** Free-tier requests per day, per model. Best published figure. */
  dailyRequests: number;
  /** Free-tier requests per minute, per model. */
  rpm: number;
}

export const MODELS: Record<string, ModelSpec> = {
  "gemini-3.8-flash": {
    provider: "gemini",
    id: "gemini-3.8-flash",
    label: "Gemini 3.8 Flash",
    contextTokens: 1_000_000,
    dailyRequests: 1500,
    rpm: 15,
  },
  "gemini-3.7-flash": {
    provider: "gemini",
    id: "gemini-3.7-flash",
    label: "Gemini 3.7 Flash",
    contextTokens: 1_000_000,
    dailyRequests: 1500,
    rpm: 15,
  },
  "gemini-3.6-flash": {
    provider: "gemini",
    id: "gemini-3.6-flash",
    label: "Gemini 3.6 Flash",
    contextTokens: 1_000_000,
    dailyRequests: 1500,
    rpm: 15,
  },
};

export type ModelRole = "master" | "architect" | "coder" | "tester";

/**
 * Ordered preference per role. The router walks the list and takes the first
 * model that is neither out of quota nor rate-limited.
 *
 * Newest first for the roles whose output quality drives everything downstream
 * (Master, Architect, Coder). The Tester runs far more often than it reasons
 * hard, so it starts on 3.6 and leaves 3.8 quota for the roles that need it.
 */
export const CASCADES: Record<ModelRole, string[]> = {
  master: ["gemini-3.8-flash", "gemini-3.7-flash", "gemini-3.6-flash"],
  architect: ["gemini-3.8-flash", "gemini-3.7-flash", "gemini-3.6-flash"],
  coder: ["gemini-3.8-flash", "gemini-3.7-flash", "gemini-3.6-flash"],
  tester: ["gemini-3.6-flash", "gemini-3.7-flash", "gemini-3.8-flash"],
};

export function specFor(modelId: string): ModelSpec {
  const spec = MODELS[modelId];
  if (!spec) throw new Error(`Unknown model: ${modelId}`);
  return spec;
}
