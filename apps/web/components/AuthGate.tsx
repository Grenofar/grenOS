"use client";

import { useEffect, useState, type ReactNode } from "react";
import type { Session } from "@supabase/supabase-js";
import { isConfigured, supabase } from "@/lib/supabase";
import { useLang } from "@/lib/i18n";

/**
 * Sign-in gate: Google first, magic link as a fallback.
 *
 * This gate is convenience, not security. Authorisation lives in RLS
 * (migration 0002), where `is_operator()` checks the signed-in email against
 * the `operators` table. Anyone can complete a Google sign-in; someone whose
 * address is not in that table gets a perfectly valid session and sees nothing
 * at all, which is the correct outcome.
 *
 * The magic link is kept deliberately: OAuth depends on a Google Cloud client
 * and a redirect URL that are easy to misconfigure, and locking yourself out
 * of your own control plane while debugging it is a bad afternoon.
 */
export function AuthGate({ children }: { children: ReactNode }) {
  const { t } = useLang();
  const [session, setSession] = useState<Session | null>(null);
  const [ready, setReady] = useState(false);
  const [showEmail, setShowEmail] = useState(false);
  const [email, setEmail] = useState("");
  const [sent, setSent] = useState(false);
  const [busy, setBusy] = useState(false);
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
  if (session) return <>{children}</>;

  async function signInWithGoogle() {
    setBusy(true);
    setError(null);
    const { error } = await supabase().auth.signInWithOAuth({
      provider: "google",
      options: {
        redirectTo: window.location.origin,
        queryParams: { prompt: "select_account" },
      },
    });
    if (error) {
      setError(error.message);
      setBusy(false);
    }
    // On success the browser navigates away; leave the button disabled.
  }

  return (
    <div className="center-page">
      <div className="card center-card">
        <div className="brand" style={{ padding: 0, marginBottom: 4 }}>
          gren<span>OS</span>
        </div>
        <p className="faint" style={{ marginBottom: 18 }}>
          {t("auth.restricted")}
        </p>

        <button
          onClick={signInWithGoogle}
          disabled={busy}
          className="google-btn"
          style={{ width: "100%" }}
        >
          <GoogleMark />
          {t("auth.google")}
        </button>

        {!showEmail && !sent && (
          <button
            className="ghost"
            onClick={() => setShowEmail(true)}
            style={{ width: "100%", marginTop: 8, fontSize: 12 }}
          >
            {t("auth.useEmail")}
          </button>
        )}

        {sent ? (
          <p className="muted" style={{ marginTop: 14 }}>
            {t("auth.sent")}
          </p>
        ) : (
          showEmail && (
            <form
              style={{ marginTop: 14 }}
              onSubmit={async (e) => {
                e.preventDefault();
                setBusy(true);
                setError(null);
                const { error } = await supabase().auth.signInWithOtp({
                  email,
                  options: { emailRedirectTo: window.location.origin },
                });
                if (error) setError(error.message);
                else setSent(true);
                setBusy(false);
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
              <button type="submit" disabled={busy} style={{ width: "100%" }}>
                {t("auth.send")}
              </button>
            </form>
          )
        )}

        {error && (
          <p className="faint" style={{ color: "var(--err)", marginTop: 12 }}>
            {error}
          </p>
        )}
      </div>
    </div>
  );
}

/** Google's mark, inlined: the CDN version is one more thing that can fail. */
function GoogleMark() {
  return (
    <svg width="16" height="16" viewBox="0 0 48 48" aria-hidden="true">
      <path
        fill="#4285F4"
        d="M45.1 24.5c0-1.6-.1-3.1-.4-4.5H24v8.5h11.8c-.5 2.7-2 5-4.4 6.6v5.5h7.1c4.1-3.8 6.6-9.5 6.6-16.1z"
      />
      <path
        fill="#34A853"
        d="M24 46c5.9 0 10.9-2 14.5-5.4l-7.1-5.5c-2 1.3-4.5 2.1-7.4 2.1-5.7 0-10.5-3.8-12.2-9H4.5v5.7C8.1 41.1 15.4 46 24 46z"
      />
      <path
        fill="#FBBC05"
        d="M11.8 28.2c-.4-1.3-.7-2.7-.7-4.2s.3-2.9.7-4.2v-5.7H4.5C3 17.1 2.1 20.4 2.1 24s.9 6.9 2.4 9.9l7.3-5.7z"
      />
      <path
        fill="#EA4335"
        d="M24 10.8c3.2 0 6.1 1.1 8.4 3.3l6.3-6.3C34.9 4.2 29.9 2 24 2 15.4 2 8.1 6.9 4.5 14.1l7.3 5.7c1.7-5.2 6.5-9 12.2-9z"
      />
    </svg>
  );
}
