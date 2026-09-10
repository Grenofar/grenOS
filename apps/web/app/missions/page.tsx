"use client";

import Link from "next/link";
import { supabase } from "@/lib/supabase";
import { useLive } from "@/lib/useLive";
import { useLang } from "@/lib/i18n";
import type { MissionOverview } from "@/lib/types";

export default function MissionsPage() {
  const { t } = useLang();
  const { rows: missions } = useLive<MissionOverview>("missions", () =>
    supabase()
      .from("mission_overview")
      .select("*")
      .order("created_at", { ascending: false }),
  );

  return (
    <>
      <div className="page-head">
        <h1>{t("missions.title")}</h1>
      </div>

      <div className="grid cols-2">
        <section>
          <h2>{t("missions.new")}</h2>
          <Link href="/missions/new" className="card card-link">
            <div className="row" style={{ marginBottom: 8 }}>
              <span className="dot live" />
              <b>{t("intake.title")}</b>
            </div>
            <p className="muted" style={{ margin: 0 }}>
              {t("intake.hint")}
            </p>
          </Link>
        </section>

        <section>
          <h2>{t("missions.title")}</h2>
          {missions.length === 0 ? (
            <div className="empty">{t("missions.empty")}</div>
          ) : (
            <div className="grid">
              {missions.map((m) => (
                <MissionCard key={m.id} mission={m} />
              ))}
            </div>
          )}
        </section>
      </div>
    </>
  );
}

function MissionCard({ mission }: { mission: MissionOverview }) {
  const { t } = useLang();
  const pct = mission.task_count
    ? (mission.tasks_done / mission.task_count) * 100
    : 0;

  const tone =
    mission.status === "done"
      ? "ok"
      : mission.status === "blocked" || mission.status === "aborted"
        ? "err"
        : mission.status === "running"
          ? "warn"
          : "";

  return (
    <Link href={`/missions/${mission.id}`} className="card card-link">
      <div className="between" style={{ marginBottom: 8 }}>
        <b>{mission.title}</b>
        <span className={`badge ${tone}`}>{t(`status.${mission.status}`)}</span>
      </div>

      <div className="meter" style={{ marginBottom: 8 }}>
        <span style={{ width: `${pct}%` }} />
      </div>

      <div className="between faint">
        <span>
          {mission.tasks_done}/{mission.task_count} {t("missions.tasks")}
          {mission.tasks_failed > 0 && (
            <span style={{ color: "var(--err)" }}> · {mission.tasks_failed} ✕</span>
          )}
        </span>
        <span className="mono">
          {fmtTokens(mission.tokens_used)} / {fmtTokens(mission.token_budget)}
        </span>
      </div>
    </Link>
  );
}

function fmtTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${Math.round(n / 1_000)}k`;
  return String(n);
}
