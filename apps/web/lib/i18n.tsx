"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";

/**
 * Bilingual FR/EN with a plain dictionary (D-010).
 *
 * No i18n library: two languages and a few dozen strings do not justify a
 * dependency, a build plugin and a message-extraction step.
 */
export type Lang = "fr" | "en";

type Dict = Record<string, { fr: string; en: string }>;

const STRINGS: Dict = {
  "nav.dashboard": { fr: "Tableau de bord", en: "Dashboard" },
  "nav.missions": { fr: "Missions", en: "Missions" },
  "nav.runs": { fr: "Vérifications", en: "Runs" },
  "nav.agents": { fr: "Agents", en: "Agents" },
  "nav.roadmap": { fr: "Feuille de route", en: "Roadmap" },

  "roadmap.intro": {
    fr: "Aucune case ne se coche ici : une étape passe à « terminée » parce que sa mission l'est, et une mission ne se termine que sur un verdict de CI. L'ordre est imposé par la matière — sans port série on ne sait pas pourquoi le kernel a redémarré.",
    en: "Nothing is ticked by hand: a step turns done because its mission did, and a mission only ends on a CI verdict. The order is imposed by the material — without serial output you cannot know why the kernel reset.",
  },
  "roadmap.empty": {
    fr: "Feuille de route absente. Passe la migration 0008.",
    en: "No roadmap. Run migration 0008.",
  },
  "roadmap.viewMap": { fr: "Carte", en: "Map" },
  "roadmap.viewTimeline": { fr: "Chronologie", en: "Timeline" },
  "roadmap.mapLabel": { fr: "Carte de progression du projet", en: "Project progress map" },
  "roadmap.doneWhen": { fr: "Terminée quand", en: "Done when" },
  "roadmap.seeMission": { fr: "Voir la mission", en: "See the mission" },
  "roadmap.todo": { fr: "à venir", en: "upcoming" },
  "roadmap.scoping": { fr: "cadrage", en: "scoping" },
  "roadmap.active": { fr: "en cours", en: "in progress" },
  "roadmap.done": { fr: "terminée", en: "done" },
  "roadmap.blocked": { fr: "bloquée", en: "blocked" },
  "roadmap.aborted": { fr: "abandonnée", en: "aborted" },

  "agents.title": { fr: "L'équipe", en: "The team" },
  "agents.active": { fr: "actif", en: "active" },
  "agents.dormant": { fr: "dormant", en: "dormant" },
  "agents.disabled": { fr: "désactivé", en: "disabled" },
  "agents.idle": { fr: "au repos", en: "idle" },
  "agents.working": { fr: "au travail", en: "working" },
  "agents.reportsTo": { fr: "rend compte à", en: "reports to" },
  "agents.readonly": { fr: "lecture seule", en: "read-only" },

  "quota.title": { fr: "Quota du jour", en: "Today's quota" },
  "quota.requests": { fr: "requêtes", en: "requests" },
  "quota.remaining": { fr: "restantes", en: "remaining" },
  "quota.credits": { fr: "décompté en crédits", en: "metered in credits" },
  "quota.explain": {
    fr: "NVIDIA : 40 requêtes/minute, crédits limités. Gemini : ~20 requêtes/jour et par modèle, mais permanent — c'est le plancher quand les crédits s'épuisent.",
    en: "NVIDIA: 40 requests/minute, finite credits. Gemini: ~20 requests/day per model but permanent — the floor when credits run out.",
  },

  "missions.title": { fr: "Missions", en: "Missions" },
  "missions.new": { fr: "Nouvelle mission", en: "New mission" },
  "missions.name": { fr: "Titre", en: "Title" },
  "missions.goal": { fr: "Ce que l'équipe doit accomplir", en: "What the team must accomplish" },
  "missions.budget": { fr: "Budget (tokens)", en: "Budget (tokens)" },
  "missions.create": { fr: "Lancer", en: "Launch" },
  "missions.empty": { fr: "Aucune mission pour l'instant.", en: "No missions yet." },
  "missions.tasks": { fr: "tâches", en: "tasks" },

  "events.title": { fr: "Activité", en: "Activity" },
  "events.empty": {
    fr: "Rien encore. Le worker n'a pas démarré.",
    en: "Nothing yet. The worker has not started.",
  },

  "kill.title": { fr: "Coupe-circuit", en: "Kill switch" },
  "kill.running": { fr: "Les agents tournent", en: "Agents running" },
  "kill.paused": { fr: "Agents en pause", en: "Agents paused" },
  "kill.stop": { fr: "Tout arrêter", en: "Stop everything" },
  "kill.resume": { fr: "Reprendre", en: "Resume" },
  "kill.explain": {
    fr: "Arrête la prise de nouvelles tâches. Les tâches en cours finissent proprement.",
    en: "Stops new tasks being picked up. Running tasks finish cleanly.",
  },

  "auth.title": { fr: "Connexion", en: "Sign in" },
  "auth.google": { fr: "Continuer avec Google", en: "Continue with Google" },
  "auth.useEmail": { fr: "Utiliser un lien par email", en: "Use an email link" },
  "auth.email": { fr: "Adresse email", en: "Email address" },
  "auth.send": { fr: "Recevoir un lien", en: "Send me a link" },
  "auth.sent": {
    fr: "Lien envoyé. Regarde ta boîte mail.",
    en: "Link sent. Check your inbox.",
  },
  "auth.restricted": {
    fr: "Accès restreint aux opérateurs déclarés.",
    en: "Access restricted to declared operators.",
  },
  "auth.signout": { fr: "Se déconnecter", en: "Sign out" },

  "intake.title": { fr: "Cadrer une mission", en: "Scope a mission" },
  "intake.untitled": { fr: "Mission en cours de cadrage", en: "Mission being scoped" },
  "intake.master": { fr: "Maître", en: "Master" },
  "intake.you": { fr: "Toi", en: "You" },
  "intake.thinking": { fr: "réfléchit…", en: "thinking…" },
  "intake.send": { fr: "Envoyer", en: "Send" },
  "intake.enterHint": {
    fr: "Entrée pour envoyer · Maj+Entrée pour une nouvelle ligne",
    en: "Enter to send · Shift+Enter for a new line",
  },
  "intake.placeholder": {
    fr: "Décris ce que tu veux obtenir. Le Maître te posera des questions.",
    en: "Describe what you want. The Master will ask you questions.",
  },
  "intake.intro": {
    fr: "Dis au Maître ce que tu veux construire.",
    en: "Tell the Master what you want to build.",
  },
  "intake.hint": {
    fr: "Il va t'interroger jusqu'à pouvoir écrire des critères qu'une machine peut vérifier — c'est ce que la CI contrôlera. Puis il nommera et lancera la mission.",
    en: "It will question you until it can write criteria a machine can check — that is what CI will verify. Then it names and launches the mission.",
  },
  "intake.launchedTitle": { fr: "Mission lancée", en: "Mission launched" },
  "intake.launchedBody": {
    fr: "Le Maître a tout ce qu'il lui faut. L'équipe travaille, la conversation est close.",
    en: "The Master has what it needs. The team is working; this conversation is closed.",
  },
  "intake.backToApp": { fr: "Suivre la mission", en: "Follow the mission" },

  "mission.escalation": { fr: "Décision requise", en: "Decision needed" },
  "mission.tasks": { fr: "Tâches", en: "Tasks" },
  "mission.noTasks": {
    fr: "Le Maître n'a pas encore décomposé la mission.",
    en: "The Master has not decomposed the mission yet.",
  },

  "task.pending": { fr: "en attente", en: "pending" },
  "task.ready": { fr: "prête", en: "ready" },
  "task.in_progress": { fr: "en cours", en: "in progress" },
  "task.awaiting_verification": { fr: "vérification", en: "verifying" },
  "task.done": { fr: "terminée", en: "done" },
  "task.failed": { fr: "échouée", en: "failed" },
  "task.blocked": { fr: "bloquée", en: "blocked" },
  "task.cancelled": { fr: "annulée", en: "cancelled" },
  "task.attempt": { fr: "tentative", en: "attempt" },
  "task.awaitingCI": {
    fr: "en attente du verdict CI — rien n'est terminé avant",
    en: "waiting on the CI verdict — nothing is done before that",
  },

  "status.draft": { fr: "brouillon", en: "draft" },
  "status.planning": { fr: "planification", en: "planning" },
  "status.running": { fr: "en cours", en: "running" },
  "status.blocked": { fr: "bloqué", en: "blocked" },
  "status.done": { fr: "terminé", en: "done" },
  "status.aborted": { fr: "abandonné", en: "aborted" },

  "setup.title": { fr: "Configuration incomplète", en: "Setup incomplete" },
  "setup.body": {
    fr: "Les variables Supabase ne sont pas définies. Remplis .env.local puis relance.",
    en: "Supabase variables are not set. Fill .env.local then restart.",
  },
};

const LangContext = createContext<{
  lang: Lang;
  setLang: (l: Lang) => void;
  t: (key: string) => string;
}>({ lang: "fr", setLang: () => {}, t: (k) => k });

export function LangProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>("fr");

  useEffect(() => {
    try {
      const saved = localStorage.getItem("grenos.lang");
      if (saved === "fr" || saved === "en") setLangState(saved);
    } catch {
      // Private browsing or blocked storage: the default is fine.
    }
  }, []);

  const setLang = useCallback((l: Lang) => {
    setLangState(l);
    try {
      localStorage.setItem("grenos.lang", l);
    } catch {
      // Preference simply will not persist. Not worth surfacing.
    }
  }, []);

  const t = useCallback(
    (key: string) => STRINGS[key]?.[lang] ?? key,
    [lang],
  );

  return (
    <LangContext.Provider value={{ lang, setLang, t }}>
      {children}
    </LangContext.Provider>
  );
}

export const useLang = () => useContext(LangContext);
