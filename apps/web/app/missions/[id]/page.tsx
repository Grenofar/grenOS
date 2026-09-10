"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { useEffect, useState } from "react";
import { supabase } from "@/lib/supabase";
import { useLive } from "@/lib/useLive";
import { useLang } from "@/lib/i18n";
import MissionChat from "@/components/MissionChat";
import type { AppEvent, Mission, Run, Task } from "@/lib/types";

/**
 * One mission, watched live.
 *
 * The list page answers "what is running"; this one answers "why is it stuck",
 * which is the question that actually costs time. So the layout leads with
 * escalations and failures rather than burying them under a progress bar: a
 * blocked mission waiting on a human decision is the single most expensive
 * state this system can be in, and it must be impossible to miss.
 *
 * The chat with the Master sits right under the escalations: that is where
 * the human answers them.
 */
export default function MissionPage() {
  const { t } = useLang();
  const params = useParams<{ id: string }>();
  const id = params.id;

  const [mission, setMission] = useState<Mission | null>(null);

  const { rows: tasks } = useLive<Task>(
    "tasks",
    () => supabase().from("tasks").select("*").eq("mission_id", id).order("created_at"),
    [id],
  );

  const { rows: events } = useLive<AppEvent>(
    "events",
    () =>
      supabase()
        .from("events")
        .select("*")
        .eq("mission_id", id)
        .order("id", { ascending: false })
        .limit(60),
    [id],
  );

  const { rows: runs } = useLive<Run>(
    "runs",
    () =>
      supabase()
        .from("runs")
        .select("*")
        .eq("mission_id", id)
        .order("started_at", { ascending: false })
        .limit(10),
    [id],
  );

  useEffect(() => {
    supabase()
      .from("missions")
      .select("*")
      .eq("id", id)
      .single()
      .then(({ data }) => setMission(data));
  }, [id, tasks.length, events.length]);

  if (!mission) return null;

  // Only the latest escalation is still a question; the older ones were
  // answered, and the mission moved on.
  const escalations = mission.status === "blocked"
    ? events.filter((e) => e.type === "escalation").slice(0, 1)
    : [];
  // A cancelled task was replaced: counting it made progress look worse
  // than it was.
  const counted = tasks.filter((x) => x.status !== "cancelled");
  const done = counted.filter((x) => x.status === "done").length;
  const pct = counted.length ? (done / counted.length) * 100 : 0;

  return (
    <>
      <div className="page-head">
        <div>
          <Link href="/missions" className="faint">
            ← {t("missions.title")}
          </Link>
          <h1 style={{ marginTop: 6 }}>{mission.title}</h1>
        </div>
        <span className={`badge ${toneOf(mission.status)}`}>
          {t(`status.${mission.status}`)}
        </span>
      </div>

      {/* Une escalade est une question qui attend une réponse humaine : rien
          n'avance tant qu'elle n'est pas traitée, donc elle passe en premier. */}
      {escalations.length > 0 && (
        <section>
          {escalations.map((e) => (
            <div
              key={e.id}
              className="card"
              style={{
                borderColor: "var(--err)",
                borderLeftWidth: 3,
                marginBottom: 10,
              }}
            >
              <div className="row" style={{ marginBottom: 6 }}>
                <span className="dot err" />
                <b>{t("mission.escalation")}</b>
                <span className="spacer" />
                <span className="faint mono">{clock(e.created_at)}</span>
              </div>
              <div className="muted">{e.message}</div>
            </div>
          ))}
        </section>
      )}

      <MissionChat missionId={id} status={mission.status} />

      <section>
        <div className="card">
          <div className="between" style={{ marginBottom: 10 }}>
            <span className="muted">{mission.description}</span>
          </div>
          <div className="meter" style={{ marginBottom: 8 }}>
            <span style={{ width: `${pct}%` }} />
          </div>
          <div className="between faint">
            <span>
              {done}/{counted.length} {t("missions.tasks")}
            </span>
            <span className="mono">
              {fmt(mission.tokens_used)} / {fmt(mission.token_budget)} tokens
            </span>
          </div>
        </div>
      </section>

      <div className="grid cols-2">
        <section>
          <h2>{t("mission.tasks")}</h2>
          {tasks.length === 0 ? (
            <div className="empty">{t("mission.noTasks")}</div>
          ) : (
            <div className="grid">
              {tasks.map((task) => (
                <TaskRow key={task.id} task={task} />
              ))}
            </div>
          )}

          {runs.length > 0 && (
            <>
              <h2 style={{ marginTop: 24 }}>{t("nav.runs")}</h2>
              <div className="card">
                {runs.map((run) => (
                  <div key={run.id} className="feed-row">
                    <span className={`badge ${run.status === "passed" ? "ok" : "err"}`}>
                      {run.status}
                    </span>
                    <span className="feed-msg mono" style={{ fontSize: 12 }}>
                      {run.branch}
                      {run.failure && (
                        <span style={{ color: "var(--err)" }}> · {run.failure}</span>
                      )}
                    </span>
                  </div>
                ))}
              </div>
            </>
          )}
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

function TaskRow({ task }: { task: Task }) {
  const { t } = useLang();
  const active = task.status === "in_progress" || task.status === "awaiting_verification";

  return (
    <div className="card">
      <div className="between" style={{ marginBottom: 6 }}>
        <div className="row">
          <span className={`dot ${dotOf(task.status)}`} />
          <span className="agent-role">{task.assigned_to}</span>
        </div>
        <span className={`badge ${toneOfTask(task.status)}`}>
          {t(`task.${task.status}`)}
        </span>
      </div>

      <div style={{ fontSize: 13 }}>{task.goal}</div>

      {(task.attempt > 1 || task.failure) && (
        <div className="faint" style={{ marginTop: 6 }}>
          {task.attempt > 1 && (
            <span>
              {t("task.attempt")} {task.attempt}/{task.max_attempts}
            </span>
          )}
          {task.failure && (
            <span style={{ color: "var(--err)" }}>
              {task.attempt > 1 ? " · " : ""}
              {task.failure}
            </span>
          )}
        </div>
      )}

      {active && !task.failure && (
        <div className="faint" style={{ marginTop: 6 }}>
          {task.status === "awaiting_verification"
            ? t("task.awaitingCI")
            : t("agents.working")}
        </div>
      )}
    </div>
  );
}

function dotOf(status: string): string {
  if (status === "in_progress") return "live";
  if (status === "done") return "ok";
  if (status === "failed") return "err";
  if (status === "blocked") return "warn";
  return "idle";
}

function toneOfTask(status: string): string {
  if (status === "done") return "ok";
  if (status === "failed" || status === "blocked") return "err";
  if (status === "awaiting_verification") return "warn";
  return "";
}

function toneOf(status: string): string {
  if (status === "done") return "ok";
  if (status === "blocked" || status === "aborted") return "err";
  if (status === "running") return "warn";
  return "";
}

function fmt(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${Math.round(n / 1_000)}k`;
  return String(n);
}

function clock(iso: string): string {
  return new Date(iso).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}
