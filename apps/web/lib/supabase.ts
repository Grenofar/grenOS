"use client";

import { createClient, type SupabaseClient } from "@supabase/supabase-js";

/**
 * Browser Supabase client.
 *
 * Only the anon key ever reaches this file, and every table is protected by
 * RLS (migration 0002). The browser can read what an operator may read, create
 * a mission, and flip the kill switch — nothing else. It cannot invent a task
 * or overwrite a CI verdict, which is what keeps a front-end bug from becoming
 * an agent-system bug.
 */
const url = process.env.NEXT_PUBLIC_SUPABASE_URL;
const anonKey = process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY;

let client: SupabaseClient | null = null;

export function supabase(): SupabaseClient {
  if (!url || !anonKey) {
    throw new Error(
      "NEXT_PUBLIC_SUPABASE_URL / NEXT_PUBLIC_SUPABASE_ANON_KEY manquants. " +
        "Remplis .env.local, puis ajoute-les aussi dans Vercel → Settings → Environment Variables.",
    );
  }
  if (!client) {
    client = createClient(url, anonKey, {
      auth: { persistSession: true, autoRefreshToken: true },
      realtime: { params: { eventsPerSecond: 5 } },
    });
  }
  return client;
}

export const isConfigured = Boolean(url && anonKey);
