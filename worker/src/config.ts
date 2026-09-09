import { readFileSync, existsSync } from "node:fs";
import { join } from "node:path";

/**
 * Environment loading, without dotenv.
 *
 * A dependency to split strings on "=" is not worth 40 packages when the whole
 * worker has a 512 MB disk budget (D-002). Real environment variables win over
 * the file, so the same code runs on bot-hosting.net where secrets come from
 * the panel rather than from disk.
 */

export const ROOT = process.env.GRENOS_ROOT ?? join(process.cwd());

function loadEnvFile(path: string): void {
  if (!existsSync(path)) return;
  for (const raw of readFileSync(path, "utf8").split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const eq = line.indexOf("=");
    if (eq === -1) continue;
    const key = line.slice(0, eq).trim();
    let value = line.slice(eq + 1).trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    if (!(key in process.env)) process.env[key] = value;
  }
}

loadEnvFile(join(ROOT, ".env.local"));

function required(key: string): string {
  const value = process.env[key];
  if (!value) {
    throw new Error(
      `${key} manquant. Remplis .env.local puis relance (npm run env pour vérifier).`,
    );
  }
  return value;
}

function num(key: string, fallback: number): number {
  const raw = process.env[key];
  const parsed = raw ? Number(raw) : NaN;
  return Number.isFinite(parsed) ? parsed : fallback;
}

export const config = {
  supabaseUrl: required("SUPABASE_URL"),
  // service_role : contourne RLS. Ne doit jamais quitter ce process.
  supabaseServiceKey: required("SUPABASE_SERVICE_ROLE_KEY"),
  geminiApiKey: required("GEMINI_API_KEY"),
  githubToken: required("GITHUB_TOKEN"),
  githubRepo: process.env.GITHUB_REPO ?? "Grenofar/grenOS",

  maxConcurrentTasks: num("MAX_CONCURRENT_TASKS", 3),
  maxAttemptsPerTask: num("MAX_ATTEMPTS_PER_TASK", 3),
  missionTokenBudget: num("MISSION_TOKEN_BUDGET", 2_000_000),

  logLevel: (process.env.LOG_LEVEL ?? "info") as "debug" | "info" | "warn",

  /** Idle poll interval. Realtime is the primary wake-up; this is the safety net. */
  pollIntervalMs: num("POLL_INTERVAL_MS", 15_000),
  /** Lease lifetime, refreshed while a task runs. */
  leaseTtlMs: num("LEASE_TTL_MS", 30 * 60 * 1000),
} as const;

const LEVELS = { debug: 0, info: 1, warn: 2 } as const;

export const log = {
  debug: (...a: unknown[]) => emit("debug", a),
  info: (...a: unknown[]) => emit("info", a),
  warn: (...a: unknown[]) => emit("warn", a),
};

function emit(level: keyof typeof LEVELS, args: unknown[]): void {
  if (LEVELS[level] < LEVELS[config.logLevel]) return;
  const stamp = new Date().toISOString().slice(11, 19);
  const line = `${stamp} ${level.padEnd(5)}`;
  if (level === "warn") console.warn(line, ...args);
  else console.log(line, ...args);
}
