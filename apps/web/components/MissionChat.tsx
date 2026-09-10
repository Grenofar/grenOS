"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { supabase } from "@/lib/supabase";
import { useLang } from "@/lib/i18n";

/**
 * Talking to the Master while the mission runs.
 *
 * Same table and same rule as the scoping chat: the browser inserts a
 * `role = 'user'` row and nothing else (RLS), and the worker answers (D-001).
 * The whole conversation shows, scoping included: it is one conversation with
 * the same Master, and what was agreed at the start is context for what is
 * said now.
 *
 * The Master reads these messages before anything else on its next cycle, and
 * keeps what it is told in its notebook, docs/MASTER.md (D-022).
 */

interface Msg {
  id: string;
  role: "user" | "master";
  content: string;
  created_at: string;
}

// A blocked mission is exactly when the human needs to answer.
const OPEN = ["planning", "running", "blocked"];

export default function MissionChat({ missionId, status }: { missionId: string; status: string }) {
  const { t } = useLang();
  const [messages, setMessages] = useState<Msg[]>([]);
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const sb = supabase();
    const reload = async () => {
      const { data } = await sb
        .from("draft_messages")
        .select("id,role,content,created_at")
        .eq("mission_id", missionId)
        .order("created_at");
      setMessages((data ?? []) as Msg[]);
    };

    void reload();
    const channel = sb
      .channel(`mission-chat:${missionId}`)
      .on(
        "postgres_changes",
        { event: "*", schema: "public", table: "draft_messages", filter: `mission_id=eq.${missionId}` },
        () => void reload(),
      )
      .subscribe();

    return () => {
      void sb.removeChannel(channel);
    };
  }, [missionId]);

  // Scroll the conversation, never the page: the chat sits mid-page, and
  // jumping to it on every message would hide the tasks being watched.
  useEffect(() => {
    if (box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [messages.length]);

  const open = OPEN.includes(status);
  const waiting = messages.length > 0 && messages[messages.length - 1]!.role === "user";

  const send = useCallback(async () => {
    const text = draft.trim();
    if (!text || sending || !open) return;

    setSending(true);
    setError(null);
    const { error } = await supabase()
      .from("draft_messages")
      .insert({ mission_id: missionId, role: "user", content: text });
    if (error) setError(error.message);
    else setDraft("");
    setSending(false);
  }, [draft, sending, open, missionId]);

  return (
    <section>
      <h2>{t("mission.chat")}</h2>

      <div ref={box} className="card chat" style={{ maxHeight: 420, overflowY: "auto" }}>
        {messages.length === 0 && (
          <div className="chat-intro">
            <p className="faint">{t("mission.chatIntro")}</p>
          </div>
        )}

        {messages.map((m) => (
          <div key={m.id} className={`bubble ${m.role}`}>
            <div className="bubble-who">{m.role === "master" ? t("intake.master") : t("intake.you")}</div>
            <div className="bubble-text">{m.content}</div>
          </div>
        ))}

        {waiting && (
          <div className="bubble master">
            <div className="bubble-who">{t("intake.master")}</div>
            <div className="bubble-text faint">{t("mission.chatWaiting")}</div>
          </div>
        )}
      </div>

      {open ? (
        <div className="composer">
          <textarea
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            placeholder={t("mission.chatPlaceholder")}
            rows={2}
            disabled={sending}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                void send();
              }
            }}
          />
          <div className="between" style={{ marginTop: 8 }}>
            <span className="faint">{t("intake.enterHint")}</span>
            <button onClick={() => void send()} disabled={sending || !draft.trim()}>
              {t("intake.send")}
            </button>
          </div>
          {error && (
            <p className="faint" style={{ color: "var(--err)", marginTop: 8 }}>
              {error}
            </p>
          )}
        </div>
      ) : (
        <p className="faint">{t("mission.chatClosed")}</p>
      )}
    </section>
  );
}
