"use client";

import Link from "next/link";
import { supabase } from "@/lib/supabase";
import { useLive } from "@/lib/useLive";
import { useLang } from "@/lib/i18n";

/**
 * L'avancement du projet, déduit et non déclaré.
 *
 * Aucune case ne se coche ici : une étape passe à « terminée » parce que sa
 * mission est terminée, et une mission ne se termine que sur un verdict de CI
 * (D-009). C'est ce qui distingue cette page d'un tableau de suivi — elle ne
 * peut pas mentir sur ce qui marche.
 */

interface Step {
  position: number;
  key: string;
  title: string;
  goal: string;
  owner_agent: string | null;
  done_when: string[];
  mission_id: string | null;
  mission_title: string | null;
  state: "todo" | "scoping" | "active" | "done" | "blocked" | "aborted";
  task_count: number;
  tasks_done: number;
}

export default function RoadmapPage() {
  const { t } = useLang();
  const { rows: steps } = useLive<Step>("missions", () =>
    supabase().from("roadmap_progress").select("*"),
  );

  const done = steps.filter((s) => s.state === "done").length;
  const pct = steps.length ? (done / steps.length) * 100 : 0;

  return (
    <>
      <div className="page-head">
        <h1>{t("nav.roadmap")}</h1>
        <span className="faint mono">
          {done}/{steps.length}
        </span>
      </div>

      <div className="card" style={{ marginBottom: 24 }}>
        <div className="meter" style={{ marginBottom: 10 }}>
          <span style={{ width: `${pct}%` }} />
        </div>
        <p className="muted" style={{ margin: 0 }}>
          {t("roadmap.intro")}
        </p>
      </div>

      <div className="timeline">
        {steps.map((step) => (
          <StepRow key={step.key} step={step} />
        ))}
        {steps.length === 0 && (
          <div className="empty">{t("roadmap.empty")}</div>
        )}
      </div>
    </>
  );
}

function StepRow({ step }: { step: Step }) {
  const { t } = useLang();

  const tone =
    step.state === "done"
      ? "ok"
      : step.state === "blocked" || step.state === "aborted"
        ? "err"
        : step.state === "active" || step.state === "scoping"
          ? "warn"
          : "";

  const dot =
    step.state === "done"
      ? "ok"
      : step.state === "active"
        ? "live"
        : step.state === "blocked" || step.state === "aborted"
          ? "err"
          : "idle";

  const running = step.state === "active" || step.state === "scoping";

  return (
    <div className={`step ${step.state === "todo" ? "future" : ""}`}>
      <div className="step-rail">
        <span className={`dot ${dot}`} />
      </div>

      <div className="card step-card">
        <div className="between" style={{ marginBottom: 6 }}>
          <div className="row">
            <span className="step-num">{step.position}</span>
            <b>{step.title}</b>
          </div>
          <span className={`badge ${tone}`}>{t(`roadmap.${step.state}`)}</span>
        </div>

        <p className="muted" style={{ margin: "0 0 10px" }}>
          {step.goal}
        </p>

        {/* Les critères sont le contrat : ce que la CI devra constater pour que
            l'étape puisse être déclarée terminée. */}
        <details>
          <summary className="faint">
            {t("roadmap.doneWhen")} ({step.done_when.length})
          </summary>
          <ul className="criteria">
            {step.done_when.map((c, i) => (
              <li key={i}>{c}</li>
            ))}
          </ul>
        </details>

        <div className="between faint" style={{ marginTop: 10 }}>
          <span>
            {step.owner_agent && (
              <span className="mono">{step.owner_agent}</span>
            )}
          </span>
          {running && step.mission_id ? (
            <Link href={`/missions/${step.mission_id}`}>
              {step.tasks_done}/{step.task_count} {t("missions.tasks")} →
            </Link>
          ) : step.state === "done" && step.mission_id ? (
            <Link href={`/missions/${step.mission_id}`}>
              {t("roadmap.seeMission")} →
            </Link>
          ) : null}
        </div>
      </div>
    </div>
  );
}
