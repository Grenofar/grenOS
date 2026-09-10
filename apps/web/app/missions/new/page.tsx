"use client";

import Link from "next/link";
import { useCallback, useEffect, useRef, useState } from "react";
import { supabase } from "@/lib/supabase";
import { useLang } from "@/lib/i18n";

/**
 * Cadrage d'une mission par conversation.
 *
 * Le navigateur n'appelle aucun modèle : il insère une ligne dans
 * `draft_messages` et attend la réponse par Realtime. C'est le worker qui
 * détient les clés et interroge le Maître (D-001). La RLS n'autorise
 * l'insertion qu'avec `role = 'user'`, donc le front ne peut pas fabriquer une
 * réponse de l'IA ni faire lancer une mission qu'elle n'a pas validée.
 *
 * Le chat se verrouille dès que la mission quitte l'état `draft` : à ce moment
 * l'équipe travaille déjà, et un message de plus n'aurait nulle part où aller.
 */

interface Msg {
  id: string;
  role: "user" | "master";
  content: string;
  created_at: string;
}

export default function NewMissionPage() {
  const { t } = useLang();

  const [missionId, setMissionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Msg[]>([]);
  const [draft, setDraft] = useState("");
  const [waiting, setWaiting] = useState(false);
  const [launched, setLaunched] = useState<{ title: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const bottom = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottom.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length, waiting]);

  // Realtime sur les deux tables : les messages, et le statut de la mission
  // qui bascule au moment où le Maître décide de lancer.
  useEffect(() => {
    if (!missionId) return;
    const sb = supabase();

    const reload = async () => {
      const { data } = await sb
        .from("draft_messages")
        .select("id,role,content,created_at")
        .eq("mission_id", missionId)
        .order("created_at");
      const rows = (data ?? []) as Msg[];
      setMessages(rows);
      // Le Maître a répondu : on rend la main.
      if (rows.length && rows[rows.length - 1]!.role === "master") setWaiting(false);
    };

    const checkStatus = async () => {
      const { data } = await sb
        .from("missions")
        .select("status,title")
        .eq("id", missionId)
        .maybeSingle();
      if (data && data.status !== "draft") setLaunched({ title: data.title });
    };

    void reload();
    void checkStatus();

    const channel = sb
      .channel(`intake:${missionId}`)
      .on(
        "postgres_changes",
        { event: "*", schema: "public", table: "draft_messages", filter: `mission_id=eq.${missionId}` },
        () => void reload(),
      )
      .on(
        "postgres_changes",
        { event: "UPDATE", schema: "public", table: "missions", filter: `id=eq.${missionId}` },
        () => void checkStatus(),
      )
      .subscribe();

    return () => {
      void sb.removeChannel(channel);
    };
  }, [missionId]);

  const send = useCallback(async () => {
    const text = draft.trim();
    if (!text || waiting || launched) return;

    setError(null);
    setDraft("");
    setWaiting(true);

    const sb = supabase();
    let id = missionId;

    // La mission naît avec le premier message. Son titre est provisoire : le
    // Maître la renomme lui-même au moment de la lancer.
    if (!id) {
      const { data: auth } = await sb.auth.getUser();
      const { data, error } = await sb
        .from("missions")
        .insert({
          title: t("intake.untitled"),
          description: "",
          status: "draft",
          created_by: auth.user?.id,
        })
        .select("id")
        .single();

      if (error || !data) {
        setError(error?.message ?? "création impossible");
        setWaiting(false);
        return;
      }
      id = data.id;
      setMissionId(id);
    }

    const { error } = await sb
      .from("draft_messages")
      .insert({ mission_id: id, role: "user", content: text });

    if (error) {
      setError(error.message);
      setWaiting(false);
    }
  }, [draft, waiting, launched, missionId, t]);

  return (
    <>
      <div className="page-head">
        <div>
          <Link href="/missions" className="faint">
            ← {t("missions.title")}
          </Link>
          <h1 style={{ marginTop: 6 }}>
            {launched ? launched.title : t("intake.title")}
          </h1>
        </div>
        {launched && <span className="badge ok">{t("status.planning")}</span>}
      </div>

      <div className="card chat">
        {messages.length === 0 && !waiting && (
          <div className="chat-intro">
            <p>{t("intake.intro")}</p>
            <p className="faint">{t("intake.hint")}</p>
          </div>
        )}

        {messages.map((m) => (
          <div key={m.id} className={`bubble ${m.role}`}>
            <div className="bubble-who">
              {m.role === "master" ? t("intake.master") : t("intake.you")}
            </div>
            <div className="bubble-text">{m.content}</div>
          </div>
        ))}

        {waiting && (
          <div className="bubble master">
            <div className="bubble-who">{t("intake.master")}</div>
            <div className="bubble-text faint">{t("intake.thinking")}</div>
          </div>
        )}

        <div ref={bottom} />
      </div>

      {launched ? (
        <div className="card launched">
          <div className="row" style={{ marginBottom: 8 }}>
            <span className="dot ok" />
            <b>{t("intake.launchedTitle")}</b>
          </div>
          <p className="muted" style={{ marginTop: 0 }}>
            {t("intake.launchedBody")}
          </p>
          <div className="row" style={{ marginTop: 14 }}>
            <Link href={`/missions/${missionId}`}>
              <button>{t("intake.backToApp")}</button>
            </Link>
            <Link href="/missions" className="faint" style={{ marginLeft: 12 }}>
              {t("missions.title")}
            </Link>
          </div>
        </div>
      ) : (
        <div className="composer">
          <textarea
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            placeholder={t("intake.placeholder")}
            rows={3}
            disabled={waiting}
            onKeyDown={(e) => {
              // Entrée envoie, Maj+Entrée saute une ligne : on écrit des
              // paragraphes ici, pas des messages de chat courts.
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                void send();
              }
            }}
          />
          <div className="between" style={{ marginTop: 8 }}>
            <span className="faint">{t("intake.enterHint")}</span>
            <button onClick={() => void send()} disabled={waiting || !draft.trim()}>
              {t("intake.send")}
            </button>
          </div>
          {error && (
            <p className="faint" style={{ color: "var(--err)", marginTop: 8 }}>
              {error}
            </p>
          )}
        </div>
      )}
    </>
  );
}
