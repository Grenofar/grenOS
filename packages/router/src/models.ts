/**
 * Model catalogue and per-role cascades.
 *
 * Only Google Gemini is wired for now. Free-tier quota is counted *per model*,
 * so spreading the same workload across 3.6, 3.7 and 3.8 Flash multiplies the
 * effective daily budget instead of sharing one pool. That is why the cascades
 * list sibling models rather than repeating the newest one: a cascade step is
 * a quota valve as much as a failure fallback.
 *
 * On the numbers below: `dailyRequests` is a DISPLAY HINT, not a limit the
 * router enforces. Published figures were wrong by two orders of magnitude —
 * blogs said 1500 requests/day, the live API answered "limit: 20" — so the
 * router never pre-emptively retires a model on a guessed number. It calls,
 * and treats Google's own 429 as the authority (see exhaustedOn in index.ts).
 * One wasted call per model per day is cheaper than being confidently wrong.
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
  /**
   * Free-tier requests per day, per model — observed from the API's own quota
   * error, and used only for the dashboard gauge. The router does not enforce
   * it; the server does.
   */
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
    dailyRequests: 20,
    rpm: 15,
  },
  "gemini-3.7-flash": {
    provider: "gemini",
    id: "gemini-3.7-flash",
    label: "Gemini 3.7 Flash",
    contextTokens: 1_000_000,
    dailyRequests: 20,
    rpm: 15,
  },
  "gemini-3.6-flash": {
    provider: "gemini",
    id: "gemini-3.6-flash",
    label: "Gemini 3.6 Flash",
    contextTokens: 1_000_000,
    dailyRequests: 20,
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
