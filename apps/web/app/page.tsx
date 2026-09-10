"use client";

import { useEffect, useState } from "react";
import { supabase } from "@/lib/supabase";
import { useLive } from "@/lib/useLive";
import { useLang } from "@/lib/i18n";
import type { Agent, AppEvent, ModelUsage, Task } from "@/lib/types";

export default function Dashboard() {
  const { t } = useLang();

  const { rows: agents } = useLive<Agent>("agents", () =>
    supabase().from("agents").select("*").order("role_class"),
  );

  const { rows: activeTasks } = useLive<Task>("tasks", () =>
    supabase()
      .from("tasks")
      .select("*")
      .in("status", ["in_progress", "awaiting_verification"]),
  );

  const { rows: events } = useLive<AppEvent>("events", () =>
    supabase().from("events").select("*").order("id", { ascending: false }).limit(25),
  );

  const [usage, setUsage] = useState<ModelUsage[]>([]);
  useEffect(() => {
    supabase()
      .from("usage_today")
      .select("*")
      .then(({ data }) => setUsage(data ?? []));
  }, [events.length]);

  const taskByAgent = new Map(activeTasks.map((task) => [task.assigned_to, task]));

  return (
    <>
      <div className="page-head">
        <h1>{t("nav.dashboard")}</h1>
        <KillSwitch />
      </div>

      <section>
        <h2>{t("agents.title")}</h2>
        <div className="grid cols-4">
          {agents.map((agent) => (
            <AgentCard
              key={agent.id}
              agent={agent}
              task={taskByAgent.get(agent.id)}
            />
          ))}
          {agents.length === 0 && (
            <div className="empty" style={{ gridColumn: "1 / -1" }}>
              {t("events.empty")}
            </div>
          )}
        </div>
      </section>

      <div className="grid cols-2">
        <section>
          <h2>{t("quota.title")}</h2>
          <div className="card">
            <QuotaPanel usage={usage} />
            <p className="faint" style={{ marginTop: 12, marginBottom: 0 }}>
              {t("quota.explain")}
            </p>
          </div>
        </section>

        <section>
          <h2>{t("events.title")}</h2>
          <div className="card">
            {events.length === 0 ? (
              <p className="faint" style={{ margin: 0 }}>
                {t("events.empty")}
              </p>
            ) : (
              <div className="feed">
                {events.map((e) => (
                  <div key={e.id} className={`feed-row ${e.level}`}>
                    <span className="feed-time">{clock(e.created_at)}</span>
                    <span className="feed-msg">
                      {e.agent_id && <b>{e.agent_id} </b>}
                      {e.message}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </section>
      </div>
    </>
  );
}

function AgentCard({ agent, task }: { agent: Agent; task?: Task }) {
  const { t } = useLang();
  const working = Boolean(task);

  return (
    <div className={`card agent-card ${agent.status === "dormant" ? "dormant" : ""}`}>
      <div className="agent-top">
        <div className="row">
          <span className={`dot ${working ? "live" : agent.status === "active" ? "ok" : "idle"}`} />
          <span className="agent-name">{agent.name}</span>
        </div>
        <span className="badge">{t(`agents.${agent.status}`)}</span>
      </div>

      <div className="agent-role">{agent.role_class}</div>

      <div className="agent-meta">
        {working ? t("agents.working") : t("agents.idle")}
        {!agent.can_write && ` · ${t("agents.readonly")}`}
      </div>

      {task && <div className="agent-task">{task.goal}</div>}
    </div>
  );
}

function QuotaPanel({ usage }: { usage: ModelUsage[] }) {
  const { t } = useLang();

  // Gemini meters requests per day, per model — observed at 20, not the 1500
  // the published figures claimed. NVIDIA meters credits instead, so no
  // per-day gauge would be honest there: it gets a plain count.
  const GEMINI_DAILY = 20;

  if (usage.length === 0) {
    return (
      <p className="faint" style={{ margin: 0 }}>
        0 {t("quota.requests")}
      </p>
    );
  }

  return (
    <>
      {usage.map((u) => {
        const metered = u.provider === "gemini";
        const pct = metered ? Math.min(100, (u.requests / GEMINI_DAILY) * 100) : 0;
        const tone = pct > 90 ? "err" : pct > 70 ? "warn" : "";
        return (
          <div className="quota-row" key={`${u.provider}/${u.model}`}>
            <div className="quota-head">
              <b>{u.model.replace(/^[^/]+\//, "")}</b>
              <span>
                {metered ? `${u.requests} / ${GEMINI_DAILY}` : `${u.requests} ${t("quota.requests")}`}
              </span>
            </div>
            {metered ? (
              <div className="meter">
                <span className={tone} style={{ width: `${pct}%` }} />
              </div>
            ) : (
              <div className="faint" style={{ fontSize: 11 }}>
                {u.provider} · {t("quota.credits")}
              </div>
            )}
          </div>
        );
      })}
    </>
  );
}

function KillSwitch() {
  const { t } = useLang();
  const [paused, setPaused] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    supabase()
      .from("settings")
      .select("value")
      .eq("key", "agents_paused")
      .single()
      .then(({ data }) => setPaused(data?.value === true));
  }, []);

  if (paused === null) return null;

  return (
    <div className="row">
      <span className={`dot ${paused ? "err" : "ok"}`} />
      <span className="muted">{paused ? t("kill.paused") : t("kill.running")}</span>
      <button
        className={paused ? "ghost" : "danger"}
        disabled={busy}
        title={t("kill.explain")}
        onClick={async () => {
          setBusy(true);
          const next = !paused;
          const { error } = await supabase()
            .from("settings")
            .update({ value: next, updated_at: new Date().toISOString() })
            .eq("key", "agents_paused");
          if (!error) setPaused(next);
          setBusy(false);
        }}
      >
        {paused ? t("kill.resume") : t("kill.stop")}
      </button>
    </div>
  );
}

function clock(iso: string): string {
  return new Date(iso).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}
