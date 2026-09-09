"use client";

import { useState } from "react";
import { supabase } from "@/lib/supabase";
import { useLive } from "@/lib/useLive";
import { useLang } from "@/lib/i18n";
import type { MissionOverview } from "@/lib/types";

export default function MissionsPage() {
  const { t } = useLang();
  const { rows: missions, refresh } = useLive<MissionOverview>("missions", () =>
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
          <NewMission onCreated={refresh} />
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

function NewMission({ onCreated }: { onCreated: () => void }) {
  const { t } = useLang();
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [budget, setBudget] = useState(2_000_000);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  return (
    <form
      className="card"
      onSubmit={async (e) => {
        e.preventDefault();
        setBusy(true);
        setError(null);

        const { data: auth } = await supabase().auth.getUser();
        const { error } = await supabase().from("missions").insert({
          title,
          description,
          token_budget: budget,
          // RLS requires created_by = auth.uid(); the browser cannot post a
          // mission on someone else's behalf.
          created_by: auth.user?.id,
          status: "draft",
        });

        if (error) setError(error.message);
        else {
          setTitle("");
          setDescription("");
          onCreated();
        }
        setBusy(false);
      }}
    >
      <div className="field">
        <label htmlFor="title">{t("missions.name")}</label>
        <input
          id="title"
          required
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Boot a hello-world kernel in QEMU"
        />
      </div>

      <div className="field">
        <label htmlFor="desc">{t("missions.goal")}</label>
        <textarea
          id="desc"
          required
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="Décris le résultat attendu, pas la manière de l'obtenir. L'Architecte s'occupe du comment."
        />
      </div>

      <div className="field">
        <label htmlFor="budget">{t("missions.budget")}</label>
        <input
          id="budget"
          type="number"
          min={100000}
          step={100000}
          value={budget}
          onChange={(e) => setBudget(Number(e.target.value))}
        />
      </div>

      <button type="submit" disabled={busy}>
        {t("missions.create")}
      </button>

      {error && (
        <p className="faint" style={{ color: "var(--err)", marginTop: 10 }}>
          {error}
        </p>
      )}
    </form>
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
    <div className="card">
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
    </div>
  );
}

function fmtTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${Math.round(n / 1_000)}k`;
  return String(n);
}
