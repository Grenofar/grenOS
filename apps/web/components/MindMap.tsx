"use client";

import { useRouter } from "next/navigation";
import { useLang } from "@/lib/i18n";

/**
 * La feuille de route en carte mentale.
 *
 * Même source que la chronologie — la vue roadmap_progress, où l'avancement
 * est déduit des verdicts de CI — lue autrement : par familles de travail
 * plutôt que dans l'ordre. On voit d'un coup d'œil quelle branche du projet
 * avance et laquelle attend, ce qu'une liste ne montre pas.
 *
 * SVG écrit à la main plutôt qu'une bibliothèque de graphes : dix nœuds ne
 * justifient pas une dépendance, et une disposition fixe reste lisible d'un
 * rendu à l'autre, là où un moteur de forces déplacerait tout à chaque
 * mise à jour Realtime.
 */

export interface MindMapStep {
  position: number;
  key: string;
  title: string;
  goal: string;
  owner_agent: string | null;
  done_when: string[];
  mission_id: string | null;
  state: "todo" | "scoping" | "active" | "done" | "blocked" | "aborted";
}

type Lang = "fr" | "en";

// Familles de travail. L'angle place la famille autour du centre ; ses étapes
// s'éventaillent autour d'elle.
const PHASES: Array<{ key: string; label: Record<Lang, string>; angle: number; steps: string[] }> = [
  { key: "boot", label: { fr: "Démarrage", en: "Boot" }, angle: 205, steps: ["boot-serial"] },
  {
    key: "core",
    label: { fr: "Noyau", en: "Kernel" },
    angle: -15,
    steps: ["gdt-idt", "phys-mem", "paging", "heap", "timer-irq"],
  },
  { key: "drivers", label: { fr: "Pilotes", en: "Drivers" }, angle: 125, steps: ["pci", "virtio-block", "keyboard"] },
  { key: "storage", label: { fr: "Stockage", en: "Storage" }, angle: 60, steps: ["vfs"] },
];

const W = 1000;
const H = 660;
const CX = W / 2;
const CY = H / 2;
const R_PHASE = 150;
const R_STEP = 290;
const FAN = 20; // degrés entre deux étapes d'une même famille
const RING = 58;

const rad = (deg: number) => (deg * Math.PI) / 180;
const polar = (r: number, deg: number) => ({
  x: CX + r * Math.cos(rad(deg)),
  y: CY + r * Math.sin(rad(deg)),
});

const LIVE = new Set(["active", "scoping"]);

export function MindMap({ steps }: { steps: MindMapStep[] }) {
  const { t, lang } = useLang();
  const router = useRouter();
  const byKey = new Map(steps.map((s) => [s.key, s]));

  const done = steps.filter((s) => s.state === "done").length;
  const circumference = 2 * Math.PI * RING;

  const phases = PHASES.map((phase) => {
    const at = polar(R_PHASE, phase.angle);
    const members = phase.steps
      .map((k) => byKey.get(k))
      .filter((s): s is MindMapStep => Boolean(s));
    // Point de contrôle commun : les arêtes s'arrondissent vers l'extérieur.
    const bend = polar((R_PHASE + R_STEP) / 2, phase.angle);
    const nodes = members.map((step, i) => {
      const angle = phase.angle + (i - (members.length - 1) / 2) * FAN;
      return { step, angle, ...polar(R_STEP, angle) };
    });
    return {
      phase,
      ...at,
      bend,
      nodes,
      done: members.filter((s) => s.state === "done").length,
      live: members.some((s) => LIVE.has(s.state)),
      count: members.length,
    };
  });

  const open = (step: MindMapStep) => {
    if (step.mission_id) router.push(`/missions/${step.mission_id}`);
  };

  return (
    <div className="card mm-wrap">
      <svg className="mm-svg" viewBox={`0 0 ${W} ${H}`} role="img" aria-label={t("roadmap.mapLabel")}>
        {phases.map((ph) => (
          <line
            key={`c-${ph.phase.key}`}
            x1={CX}
            y1={CY}
            x2={ph.x}
            y2={ph.y}
            className={`mm-edge ${ph.count && ph.done === ph.count ? "done" : ""}`}
          />
        ))}

        {phases.flatMap((ph) =>
          ph.nodes.map((n) => (
            <path
              key={`e-${n.step.key}`}
              d={`M ${ph.x} ${ph.y} Q ${ph.bend.x} ${ph.bend.y} ${n.x} ${n.y}`}
              className={`mm-edge ${n.step.state}`}
            />
          )),
        )}

        <g>
          <circle cx={CX} cy={CY} r={RING} className="mm-ring-bg" />
          <circle
            cx={CX}
            cy={CY}
            r={RING}
            className="mm-ring"
            strokeDasharray={`${steps.length ? (done / steps.length) * circumference : 0} ${circumference}`}
            transform={`rotate(-90 ${CX} ${CY})`}
          />
          <text x={CX} y={CY - 2} className="mm-center-title">
            grenOS
          </text>
          <text x={CX} y={CY + 20} className="mm-center-sub">
            {done}/{steps.length}
          </text>
        </g>

        {phases.map((ph) => (
          <g key={ph.phase.key} className={`mm-phase ${ph.live ? "live" : ""}`}>
            <rect x={ph.x - 54} y={ph.y - 16} width={108} height={32} rx={16} />
            <text x={ph.x} y={ph.y + 5}>
              {ph.phase.label[lang as Lang]}
            </text>
            <text x={ph.x} y={ph.y + 34} className="mm-phase-count">
              {ph.done}/{ph.count}
            </text>
          </g>
        ))}

        {phases.flatMap((ph) =>
          ph.nodes.map((n) => {
            const right = Math.cos(rad(n.angle)) >= 0;
            const lx = n.x + (right ? 18 : -18);
            const clickable = Boolean(n.step.mission_id);
            return (
              <g
                key={n.step.key}
                className={`mm-step ${n.step.state} ${clickable ? "clickable" : ""}`}
                onClick={() => open(n.step)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") open(n.step);
                }}
                tabIndex={clickable ? 0 : -1}
                role={clickable ? "link" : undefined}
              >
                <title>{`${n.step.position}. ${n.step.title} — ${n.step.goal}`}</title>
                {LIVE.has(n.step.state) && <circle cx={n.x} cy={n.y} r={16} className="mm-pulse" />}
                <circle cx={n.x} cy={n.y} r={11} className="mm-dot" />
                <text x={n.x} y={n.y + 4} className="mm-num">
                  {n.step.position}
                </text>
                <text x={lx} y={n.y - 2} textAnchor={right ? "start" : "end"} className="mm-label">
                  {n.step.title}
                </text>
                <text x={lx} y={n.y + 14} textAnchor={right ? "start" : "end"} className="mm-state">
                  {t(`roadmap.${n.step.state}`)}
                  {n.step.owner_agent ? ` · ${n.step.owner_agent}` : ""}
                </text>
              </g>
            );
          }),
        )}
      </svg>

      <div className="mm-legend">
        {(["done", "active", "scoping", "blocked", "todo"] as const).map((s) => (
          <span key={s} className="row">
            <span className={`mm-key ${s}`} />
            {t(`roadmap.${s}`)}
          </span>
        ))}
      </div>
    </div>
  );
}
