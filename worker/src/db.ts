import { createClient, type SupabaseClient } from "@supabase/supabase-js";
import { config, log } from "./config.ts";
import type { AgentDefinition } from "./prompts.ts";

/**
 * Service-role access to Supabase. This client bypasses every RLS policy, so
 * it exists only inside the worker process and never leaves it.
 */
export const db: SupabaseClient = createClient(
  config.supabaseUrl,
  config.supabaseServiceKey,
  { auth: { persistSession: false, autoRefreshToken: false } },
);

/** Usage counters for the router, backed by `model_usage`. */
export class SupabaseUsageStore {
  private cache = new Map<string, { day: string; count: number }>();

  async requestsToday(model: string): Promise<number> {
    const day = today();
    const hit = this.cache.get(model);
    if (hit && hit.day === day) return hit.count;

    const { data } = await db
      .from("model_usage")
      .select("requests")
      .eq("day", day)
      .eq("model", model)
      .maybeSingle();

    const count = data?.requests ?? 0;
    this.cache.set(model, { day, count });
    return count;
  }

  async record(entry: {
    provider: string;
    model: string;
    tokensIn: number;
    tokensOut: number;
    error?: string;
  }): Promise<void> {
    const day = today();

    // Counters must survive concurrent writes, so the increment happens in the
    // database rather than as read-modify-write from here.
    const { error } = await db.rpc("bump_model_usage", {
      p_day: day,
      p_provider: entry.provider,
      p_model: entry.model,
      p_in: entry.tokensIn,
      p_out: entry.tokensOut,
      p_error: entry.error ?? null,
    });

    if (error) log.warn("model_usage non enregistré:", error.message);

    const hit = this.cache.get(entry.model);
    this.cache.set(entry.model, {
      day,
      count: (hit && hit.day === day ? hit.count : 0) + 1,
    });
  }
}

/** Mirror agents/**\/*.md into the `agents` table so the dashboard matches disk. */
export async function syncAgents(agents: AgentDefinition[]): Promise<void> {
  const rows = agents.map((a) => ({
    id: a.id,
    name: a.name,
    status: a.status,
    role_class: a.roleClass,
    reports_to: a.reportsTo,
    model_role: a.modelRole,
    max_tokens_per_task: a.maxTokensPerTask,
    max_attempts: a.maxAttempts,
    can_write: a.canWrite,
    allowed_paths: a.allowedPaths,
    forbidden_paths: a.forbiddenPaths,
    prompt_sha256: a.promptSha,
    synced_at: new Date().toISOString(),
  }));

  // Master first: the others reference it through reports_to.
  const ordered = [
    ...rows.filter((r) => r.id === "master"),
    ...rows.filter((r) => r.id !== "master"),
  ];

  for (const row of ordered) {
    const { error } = await db.from("agents").upsert(row);
    if (error) throw new Error(`sync agent ${row.id}: ${error.message}`);
  }
  log.info(`${rows.length} agents synchronisés depuis agents/`);
}

export async function emit(event: {
  missionId?: string | null;
  taskId?: string | null;
  agentId?: string | null;
  level?: "debug" | "info" | "warn" | "error";
  type: string;
  message: string;
  payload?: Record<string, unknown>;
}): Promise<void> {
  const { error } = await db.from("events").insert({
    mission_id: event.missionId ?? null,
    task_id: event.taskId ?? null,
    agent_id: event.agentId ?? null,
    level: event.level ?? "info",
    type: event.type,
    message: event.message,
    payload: event.payload ?? {},
  });
  if (error) log.warn("event non enregistré:", error.message);
}

export async function agentsPaused(): Promise<boolean> {
  const { data } = await db
    .from("settings")
    .select("value")
    .eq("key", "agents_paused")
    .maybeSingle();
  return data?.value === true;
}

const today = (): string => new Date().toISOString().slice(0, 10);
