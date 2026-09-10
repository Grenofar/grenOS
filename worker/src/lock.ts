import { hostname } from "node:os";
import { randomUUID } from "node:crypto";
import { log } from "./config.ts";
import { db } from "./db.ts";

/**
 * One brain at a time.
 *
 * Six workers were found running at once on 2026-09-10, the oldest since the
 * morning. On Windows, stopping `npm run worker` kills npm and leaves the
 * node child alive, so every restart added a worker instead of replacing
 * one. Each ran its own Master cycle, with its own code version and its own
 * keys: duplicate tasks, double executions, and failures stamped with
 * messages from code that no longer existed.
 *
 * The lock lives in the database rather than in a PID file because the brain
 * is meant to move to a remote host, and two machines must not both believe
 * they are in charge either.
 */

export interface LockValue {
  instance: string;
  pid: number;
  host: string;
  started_at: string;
  heartbeat_at: string;
}

/** A holder silent for longer than this is presumed dead. */
export const STALE_AFTER_MS = 90_000;
const HEARTBEAT_EVERY_MS = 20_000;
const KEY = "worker_lock";

export type LockDecision =
  | { acquire: true; takeover: LockValue | null }
  | { acquire: false; holder: LockValue; silentForMs: number };

/** Pure: may `mine` take the lock, given what is stored? */
export function decide(existing: LockValue | null, mine: string, now: number): LockDecision {
  if (!existing || existing.instance === mine) return { acquire: true, takeover: null };

  const silentForMs = now - Date.parse(existing.heartbeat_at);
  // An unreadable heartbeat is treated as dead: refusing forever on a
  // corrupted row would lock the system out until someone edits the database.
  if (!Number.isFinite(silentForMs) || silentForMs > STALE_AFTER_MS) {
    return { acquire: true, takeover: existing };
  }
  return { acquire: false, holder: existing, silentForMs };
}

export const instanceId = randomUUID();
let held: LockValue | null = null;

export async function acquireLock(): Promise<void> {
  const { data, error: readError } = await db
    .from("settings")
    .select("value")
    .eq("key", KEY)
    .maybeSingle();
  if (readError) throw new Error(`verrou du worker (lecture) : ${readError.message}`);

  const verdict = decide((data?.value ?? null) as LockValue | null, instanceId, Date.now());

  if (!verdict.acquire) {
    throw new Error(
      `Un autre worker tourne déjà (PID ${verdict.holder.pid} sur ${verdict.holder.host}, ` +
        `dernier signe de vie il y a ${Math.round(verdict.silentForMs / 1000)} s). ` +
        `Deux cerveaux en parallèle dupliquent les tâches et se contredisent : ` +
        `arrête l'autre, ou attends ${STALE_AFTER_MS / 1000} s s'il est mort.`,
    );
  }
  if (verdict.takeover) {
    log.warn(
      `verrou repris à un worker muet (PID ${verdict.takeover.pid} sur ${verdict.takeover.host})`,
    );
  }

  const now = new Date().toISOString();
  held = { instance: instanceId, pid: process.pid, host: hostname(), started_at: now, heartbeat_at: now };
  const { error } = await db.from("settings").upsert({ key: KEY, value: held, updated_at: now });
  if (error) throw new Error(`verrou du worker (écriture) : ${error.message}`);
}

/**
 * Keep the lock alive, and give up the moment someone else holds it.
 *
 * Runs on its own timer, not in the tick: a tick can spend minutes waiting on
 * a model, and a heartbeat that waits with it would let the lock go stale
 * under a perfectly healthy worker.
 *
 * The update only matches while the stored instance is still ours. Two
 * workers that raced at startup cannot both keep running: the one whose
 * heartbeat finds another instance in the row stops itself.
 */
export function startHeartbeat(onLost: () => void): () => void {
  const timer = setInterval(async () => {
    if (!held) return;
    const now = new Date().toISOString();
    const next = { ...held, heartbeat_at: now };

    const { data, error } = await db
      .from("settings")
      .update({ value: next, updated_at: now })
      .eq("key", KEY)
      .eq("value->>instance", instanceId)
      .select("key");

    if (error) {
      // A transient database error is not a lost lock. If it lasts past the
      // stale threshold, another worker may take over — and then the next
      // successful heartbeat matches nothing and this one stops.
      log.warn(`battement du verrou : ${error.message}`);
      return;
    }
    if (!data || data.length === 0) {
      clearInterval(timer);
      onLost();
      return;
    }
    held = next;
  }, HEARTBEAT_EVERY_MS);

  return () => clearInterval(timer);
}

export async function releaseLock(): Promise<void> {
  await db.from("settings").delete().eq("key", KEY).eq("value->>instance", instanceId);
  held = null;
}
