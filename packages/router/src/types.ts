import type { ModelRole } from "./models.ts";

export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

export interface CompletionRequest {
  role: ModelRole;
  /** System prompt: the shared protocol plus the agent's own .md body. */
  system: string;
  messages: ChatMessage[];
  maxOutputTokens?: number;
  temperature?: number;
  /**
   * Agents must answer with a single JSON object (agents/README.md §4).
   * When true the provider is asked for JSON directly, which removes a whole
   * class of "model wrapped it in a code fence" parse failures.
   */
  json?: boolean;
  /** Abort the whole cascade if it outlives this. */
  timeoutMs?: number;
}

export interface CompletionResult {
  text: string;
  model: string;
  provider: string;
  tokensIn: number;
  tokensOut: number;
  latencyMs: number;
  /** Models skipped or failed before this one succeeded, for the dashboard. */
  attempts: AttemptRecord[];
}

export interface AttemptRecord {
  model: string;
  outcome: "ok" | "rate_limited" | "quota_exhausted" | "error" | "skipped";
  detail?: string;
}

/**
 * Persistence seam for usage counters. The router does not know about
 * Supabase; the worker passes an implementation backed by `model_usage`.
 * Keeps the router testable and dependency-free.
 */
export interface UsageStore {
  /** Requests already spent today on this model. */
  requestsToday(model: string): Promise<number>;
  record(entry: {
    provider: string;
    model: string;
    tokensIn: number;
    tokensOut: number;
    error?: string;
  }): Promise<void>;
}

/** Every model in the cascade refused or failed. */
export class RouterExhaustedError extends Error {
  // Declared then assigned: the worker runs this TypeScript with no build step
  // (Node strip-only mode), where constructor parameter properties are a
  // syntax error.
  readonly role: ModelRole;
  readonly attempts: AttemptRecord[];

  constructor(role: ModelRole, attempts: AttemptRecord[]) {
    super(
      `No model available for role "${role}". Tried: ` +
        attempts.map((a) => `${a.model} (${a.outcome})`).join(", "),
    );
    this.name = "RouterExhaustedError";
    this.role = role;
    this.attempts = attempts;
  }
}
