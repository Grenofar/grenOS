"use client";

import { supabase } from "@/lib/supabase";
import { useLive } from "@/lib/useLive";
import { useLang } from "@/lib/i18n";
import type { Run } from "@/lib/types";

/**
 * CI verdicts. This page is the only place in the product that says whether
 * anything actually works: agents report, CI decides (D-009).
 */
export default function RunsPage() {
  const { t } = useLang();
  const { rows: runs } = useLive<Run>("runs", () =>
    supabase().from("runs").select("*").order("started_at", { ascending: false }).limit(40),
  );

  return (
    <>
      <div className="page-head">
        <h1>{t("nav.runs")}</h1>
      </div>

      {runs.length === 0 ? (
        <div className="empty">{t("events.empty")}</div>
      ) : (
        <div className="grid">
          {runs.map((run) => (
            <RunCard key={run.id} run={run} />
          ))}
        </div>
      )}
    </>
  );
}

function RunCard({ run }: { run: Run }) {
  const tone =
    run.status === "passed"
      ? "ok"
      : run.status === "queued" || run.status === "running"
        ? ""
        : "err";

  return (
    <div className="card">
      <div className="between" style={{ marginBottom: 10 }}>
        <div className="row">
          <span className={`dot ${tone || "idle"}`} />
          <span className="mono" style={{ fontSize: 12.5 }}>
            {run.branch}
          </span>
          {run.commit_sha && (
            <span className="faint mono">{run.commit_sha.slice(0, 7)}</span>
          )}
        </div>
        <span className={`badge ${tone}`}>{run.status}</span>
      </div>

      {run.failure && (
        <div className="faint" style={{ color: "var(--err)", marginBottom: 8 }}>
          {run.failure}
        </div>
      )}

      {run.verdicts.length > 0 && (
        <div className="feed">
          {run.verdicts.map((v, i) => (
            <div key={i} className="feed-row">
              <span className={`badge ${v.verdict === "PASS" ? "ok" : v.verdict === "FAIL" ? "err" : "warn"}`}>
                {v.verdict}
              </span>
              <span className="feed-msg">
                {v.criterion}
                {v.evidence && (
                  <span className="faint mono" style={{ display: "block", marginTop: 3 }}>
                    {v.evidence}
                  </span>
                )}
              </span>
            </div>
          ))}
        </div>
      )}

      {run.log_excerpt && (
        <pre
          className="mono"
          style={{
            marginTop: 10,
            marginBottom: 0,
            padding: 10,
            background: "var(--bg)",
            border: "1px solid var(--border)",
            borderRadius: 6,
            fontSize: 11.5,
            color: "var(--text-dim)",
            overflowX: "auto",
            maxHeight: 220,
          }}
        >
          {run.log_excerpt}
        </pre>
      )}
    </div>
  );
}
