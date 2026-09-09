"use client";

import { supabase } from "@/lib/supabase";
import { useLive } from "@/lib/useLive";
import { useLang } from "@/lib/i18n";
import type { Agent } from "@/lib/types";

/**
 * Read-only view of the team. Deliberately not editable from the browser:
 * agent behaviour lives in `agents/**\/*.md` and changes through a reviewed
 * git diff, never through a form (D-007).
 */
export default function AgentsPage() {
  const { t } = useLang();
  const { rows: agents } = useLive<Agent>("agents", () =>
    supabase().from("agents").select("*").order("id"),
  );

  const active = agents.filter((a) => a.status === "active");
  const dormant = agents.filter((a) => a.status !== "active");

  return (
    <>
      <div className="page-head">
        <h1>{t("nav.agents")}</h1>
        <span className="faint mono">agents/**/*.md</span>
      </div>

      <section>
        <h2>{t("agents.active")}</h2>
        <div className="grid cols-2">
          {active.map((a) => (
            <AgentRow key={a.id} agent={a} />
          ))}
        </div>
      </section>

      <section>
        <h2>{t("agents.dormant")}</h2>
        <div className="grid cols-2">
          {dormant.map((a) => (
            <AgentRow key={a.id} agent={a} />
          ))}
        </div>
      </section>
    </>
  );
}

function AgentRow({ agent }: { agent: Agent }) {
  const { t } = useLang();
  return (
    <div className={`card ${agent.status !== "active" ? "dormant" : ""}`}>
      <div className="between" style={{ marginBottom: 6 }}>
        <div className="row">
          <span className={`dot ${agent.status === "active" ? "ok" : "idle"}`} />
          <b>{agent.name}</b>
        </div>
        <span className="badge">{agent.role_class}</span>
      </div>
      <div className="faint mono">{agent.id}</div>
      <div className="muted" style={{ marginTop: 6 }}>
        {agent.reports_to
          ? `${t("agents.reportsTo")} ${agent.reports_to}`
          : "—"}
        {!agent.can_write && ` · ${t("agents.readonly")}`}
      </div>
    </div>
  );
}
