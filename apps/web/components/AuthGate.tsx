"use client";

import { useEffect, useState, type ReactNode } from "react";
import type { Session } from "@supabase/supabase-js";
import { isConfigured, supabase } from "@/lib/supabase";
import { useLang } from "@/lib/i18n";

/**
 * Magic-link sign-in.
 *
 * This gate is convenience, not security: authorisation lives in RLS
 * (migration 0002), where `is_operator()` checks the email against the
 * `operators` table. Someone who signs in with an unlisted address gets a
 * valid session and sees nothing at all, which is the correct outcome.
 */
export function AuthGate({ children }: { children: ReactNode }) {
  const { t } = useLang();
  const [session, setSession] = useState<Session | null>(null);
  const [ready, setReady] = useState(false);
  const [email, setEmail] = useState("");
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isConfigured) {
      setReady(true);
      return;
    }
    const sb = supabase();
    sb.auth.getSession().then(({ data }) => {
      setSession(data.session);
      setReady(true);
    });
    const { data: sub } = sb.auth.onAuthStateChange((_e, s) => setSession(s));
    return () => sub.subscription.unsubscribe();
  }, []);

  if (!isConfigured) {
    return (
      <div className="center-page">
        <div className="card center-card">
          <h1>{t("setup.title")}</h1>
          <p className="muted" style={{ marginTop: 10 }}>
            {t("setup.body")}
          </p>
          <p className="faint mono" style={{ marginTop: 14 }}>
            NEXT_PUBLIC_SUPABASE_URL
            <br />
            NEXT_PUBLIC_SUPABASE_ANON_KEY
          </p>
        </div>
      </div>
    );
  }

  if (!ready) return null;

  if (!session) {
    return (
      <div className="center-page">
        <div className="card center-card">
          <div className="brand" style={{ padding: 0, marginBottom: 4 }}>
            gren<span>OS</span>
          </div>
          <p className="faint" style={{ marginBottom: 18 }}>
            {t("auth.restricted")}
          </p>

          {sent ? (
            <p className="muted">{t("auth.sent")}</p>
          ) : (
            <form
              onSubmit={async (e) => {
                e.preventDefault();
                setError(null);
                const { error } = await supabase().auth.signInWithOtp({
                  email,
                  options: { emailRedirectTo: window.location.origin },
                });
                if (error) setError(error.message);
                else setSent(true);
              }}
            >
              <div className="field">
                <label htmlFor="email">{t("auth.email")}</label>
                <input
                  id="email"
                  type="email"
                  required
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  placeholder="toi@exemple.com"
                />
              </div>
              <button type="submit" style={{ width: "100%" }}>
                {t("auth.send")}
              </button>
              {error && (
                <p className="faint" style={{ color: "var(--err)", marginTop: 10 }}>
                  {error}
                </p>
              )}
            </form>
          )}
        </div>
      </div>
    );
  }

  return <>{children}</>;
}
