"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { supabase } from "./supabase";

/**
 * What a Supabase query builder looks like once awaited.
 *
 * Described structurally rather than imported from postgrest-js: that internal
 * type has changed shape across releases (it now takes 2-3 generic parameters),
 * and pinning to it means a routine dependency bump breaks the build. All this
 * hook needs is something awaitable that yields data and an error.
 */
type Query<T> = PromiseLike<{
  data: T[] | null;
  error: { message: string } | null;
}>;

/**
 * Subscribe to a table and keep a query's result fresh.
 *
 * On any change we re-run the query instead of patching the local array from
 * the payload. Merging change events by hand is where live dashboards quietly
 * drift out of sync with the database: an out-of-order UPDATE, a row that
 * moves out of the filter, a DELETE that arrives before its INSERT. Re-reading
 * is a few extra queries a minute against a table with a handful of rows, and
 * it is always right.
 *
 * Refetches are debounced so a burst of agent writes causes one query, and
 * `inFlight` prevents overlapping reads from landing out of order.
 */
export function useLive<T>(
  table: string,
  buildQuery: () => Query<T>,
  deps: unknown[] = [],
): { rows: T[]; loading: boolean; error: string | null; refresh: () => void } {
  const [rows, setRows] = useState<T[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const queryRef = useRef(buildQuery);
  queryRef.current = buildQuery;

  const inFlight = useRef(false);
  const queued = useRef(false);

  const load = useCallback(async () => {
    if (inFlight.current) {
      queued.current = true;
      return;
    }
    inFlight.current = true;
    try {
      const { data, error } = await queryRef.current();
      if (error) setError(error.message);
      else {
        setRows((data ?? []) as T[]);
        setError(null);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
      inFlight.current = false;
      if (queued.current) {
        queued.current = false;
        void load();
      }
    }
  }, []);

  useEffect(() => {
    void load();

    let timer: ReturnType<typeof setTimeout> | null = null;
    const channel = supabase()
      .channel(`live:${table}`)
      .on(
        "postgres_changes",
        { event: "*", schema: "public", table },
        () => {
          if (timer) clearTimeout(timer);
          timer = setTimeout(() => void load(), 250);
        },
      )
      .subscribe();

    return () => {
      if (timer) clearTimeout(timer);
      void supabase().removeChannel(channel);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [table, load, ...deps]);

  return { rows, loading, error, refresh: load };
}
