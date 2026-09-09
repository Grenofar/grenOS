#!/usr/bin/env node
/**
 * Check the whole installation against the real services, in one command.
 *
 * Setting this project up means five keys, five migrations, two dashboards and
 * a CI secret, and every one of them fails in its own way — a denied Google
 * project, a migration not run, an operator missing from the allowlist. Found
 * one at a time, each costs a round trip and a confusing error somewhere far
 * from the cause. This finds all of them at once and says what to do.
 *
 * Never prints a secret: keys are shown masked, and what is reported is
 * whether a service accepted them.
 */

import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const deep = process.argv.includes("--deep");

const env = loadEnv(join(root, ".env.local"));
const results = [];

function ok(area, msg) { results.push({ level: "ok", area, msg }); }
function warn(area, msg, fix) { results.push({ level: "warn", area, msg, fix }); }
function bad(area, msg, fix) { results.push({ level: "bad", area, msg, fix }); }

// ---------------------------------------------------------------------------
// 1. Le fichier
// ---------------------------------------------------------------------------
const REQUIRED = [
  "SUPABASE_URL",
  "SUPABASE_ANON_KEY",
  "SUPABASE_SERVICE_ROLE_KEY",
  "GEMINI_API_KEY",
  "GITHUB_TOKEN",
];

for (const key of REQUIRED) {
  if (!env[key]) {
    bad(".env", `${key} est vide`, "Voir .env.local — chaque bloc contient le lien.");
  }
}

const anonRole = jwtRole(env.SUPABASE_ANON_KEY);
const svcRole = jwtRole(env.SUPABASE_SERVICE_ROLE_KEY);

if (anonRole === "service_role") {
  bad(
    ".env",
    "SUPABASE_ANON_KEY contient la clé service_role",
    "Inversion à corriger : cette valeur est recopiée dans NEXT_PUBLIC_* et partirait en clair dans le JavaScript du site.",
  );
}
if (svcRole === "anon") {
  bad(".env", "SUPABASE_SERVICE_ROLE_KEY contient la clé anon", "Les deux clés sont inversées.");
}
if (env.NEXT_PUBLIC_SUPABASE_ANON_KEY && env.NEXT_PUBLIC_SUPABASE_ANON_KEY !== env.SUPABASE_ANON_KEY) {
  warn(".env", "NEXT_PUBLIC_SUPABASE_ANON_KEY diffère de SUPABASE_ANON_KEY", "npm run env pour resynchroniser.");
}

// ---------------------------------------------------------------------------
// 2. Supabase
// ---------------------------------------------------------------------------
const TABLES = [
  "agents", "missions", "tasks", "messages", "artifacts",
  "runs", "leases", "model_usage", "events", "settings", "operators",
];

async function checkSupabase() {
  const url = env.SUPABASE_URL;
  const key = env.SUPABASE_SERVICE_ROLE_KEY;
  if (!url || !key) return;

  const headers = { apikey: key, authorization: `Bearer ${key}` };

  const missing = [];
  for (const table of TABLES) {
    const res = await fetch(`${url}/rest/v1/${table}?select=*&limit=1`, { headers }).catch(() => null);
    if (!res) return bad("supabase", "Injoignable", "Vérifie SUPABASE_URL et ta connexion.");
    if (res.status === 401) {
      return bad("supabase", "Clé service_role refusée", "Reprends-la dans Settings → API.");
    }
    if (!res.ok) missing.push(table);
  }

  if (missing.length === TABLES.length) {
    return bad(
      "supabase",
      "Aucune table — les migrations n'ont pas été passées",
      "SQL Editor → exécute packages/db/migrations/0001 à 0005, dans l'ordre.",
    );
  }
  if (missing.length) {
    return bad(
      "supabase",
      `Tables manquantes : ${missing.join(", ")}`,
      "Une migration a échoué ou n'a pas été passée. Reprends-les dans l'ordre.",
    );
  }
  ok("supabase", `${TABLES.length} tables présentes`);

  // Les fonctions de 0004 : sans elles le worker ne peut ni prendre une tâche
  // ni poser un verrou, et il échoue à la première boucle.
  const rpc = await fetch(`${url}/rest/v1/rpc/claim_next_task`, {
    method: "POST",
    headers: { ...headers, "content-type": "application/json" },
    body: JSON.stringify({ p_agent_ids: ["__doctor__"] }),
  });
  if (rpc.ok) ok("supabase", "fonctions SQL en place (0004)");
  else bad("supabase", "Fonctions SQL absentes", "Passe packages/db/migrations/0004_functions.sql.");

  const agents = await (await fetch(`${url}/rest/v1/agents?select=id,status`, { headers })).json();
  if (Array.isArray(agents) && agents.length >= 9) {
    ok("supabase", `${agents.length} agents enregistrés (${agents.filter((a) => a.status === "active").length} actifs)`);
  } else {
    warn("supabase", "Agents non enregistrés", "Passe 0003_seed_agents.sql, ou lance le worker : il resynchronise au démarrage.");
  }

  const operators = await (await fetch(`${url}/rest/v1/operators?select=email`, { headers })).json();
  if (Array.isArray(operators) && operators.length) {
    ok("supabase", `opérateurs autorisés : ${operators.map((o) => o.email).join(", ")}`);
  } else {
    bad(
      "supabase",
      "Aucun opérateur autorisé",
      "Personne ne pourra voir le dashboard : insère ton email dans la table operators.",
    );
  }
}

// ---------------------------------------------------------------------------
// 3. Gemini
// ---------------------------------------------------------------------------
async function checkGemini() {
  const key = env.GEMINI_API_KEY;
  if (!key) return;

  const res = await fetch("https://generativelanguage.googleapis.com/v1beta/models?pageSize=200", {
    headers: { "x-goog-api-key": key },
  }).catch(() => null);

  if (!res) return bad("gemini", "Injoignable", "Problème réseau.");
  if (res.status === 403 || res.status === 401) {
    return bad(
      "gemini",
      "Clé ou projet Google refusé",
      "Crée une clé dans un NOUVEAU projet sur aistudio.google.com/apikey.",
    );
  }
  if (!res.ok) return bad("gemini", `HTTP ${res.status}`, "");

  const data = await res.json();
  const available = new Set((data.models ?? []).map((m) => m.name.replace("models/", "")));

  // pathToFileURL : sous Windows un chemin absolu commence par "c:", que le
  // chargeur ESM prend pour un protocole inconnu.
  const { MODELS } = await import(
    pathToFileURL(join(root, "packages/router/src/models.ts")).href
  );
  const absent = Object.keys(MODELS).filter((id) => !available.has(id));

  if (absent.length === 0) {
    ok("gemini", `clé valide, les ${Object.keys(MODELS).length} modèles du catalogue existent`);
  } else {
    warn(
      "gemini",
      `modèles absents de l'API : ${absent.join(", ")}`,
      "Le catalogue gratuit change souvent — mets à jour packages/router/src/models.ts.",
    );
  }

  // Lister ne prouve pas qu'on peut générer : c'est exactement la différence
  // qui a coûté une soirée. Mais générer consomme du quota, d'où --deep.
  if (!deep) {
    warn("gemini", "génération non testée", "npm run doctor -- --deep pour un vrai appel (consomme 1 requête sur ~20/jour).");
    return;
  }

  const first = Object.keys(MODELS)[0];
  const gen = await fetch(`https://generativelanguage.googleapis.com/v1beta/models/${first}:generateContent`, {
    method: "POST",
    headers: { "content-type": "application/json", "x-goog-api-key": key },
    body: JSON.stringify({
      contents: [{ role: "user", parts: [{ text: "ok" }] }],
      generationConfig: { maxOutputTokens: 2048 },
    }),
  });

  if (gen.ok) ok("gemini", `génération testée sur ${first}`);
  else if (gen.status === 429) warn("gemini", "quota journalier atteint", "~20 requêtes/jour et par modèle. Réessaie demain.");
  else {
    const msg = await gen.text();
    bad("gemini", `génération refusée (HTTP ${gen.status})`, msg.slice(0, 160));
  }
}

// ---------------------------------------------------------------------------
// 4. GitHub
// ---------------------------------------------------------------------------
async function checkGitHub() {
  const token = env.GITHUB_TOKEN;
  const repo = env.GITHUB_REPO ?? "Grenofar/grenOS";
  if (!token) return;

  const headers = {
    authorization: `Bearer ${token}`,
    accept: "application/vnd.github+json",
    "x-github-api-version": "2022-11-28",
  };

  const res = await fetch(`https://api.github.com/repos/${repo}`, { headers }).catch(() => null);
  if (!res) return bad("github", "Injoignable", "");
  if (res.status === 401) return bad("github", "PAT refusé ou expiré", "Régénère-le : les fine-grained expirent.");
  if (res.status === 404) {
    return bad("github", `Dépôt ${repo} inaccessible`, "Le PAT doit cibler ce dépôt (Only select repositories).");
  }

  const data = await res.json();
  if (!data.permissions?.push) {
    bad("github", "PAT sans droit d'écriture", "Il faut Contents: Read and write — les agents poussent des branches.");
  } else {
    ok("github", `${repo} accessible en écriture (${data.private ? "privé" : "public"})`);
  }

  const runs = await fetch(`https://api.github.com/repos/${repo}/actions/runs?per_page=1`, { headers });
  if (runs.ok) ok("github", "lecture des runs Actions autorisée");
  else warn("github", "lecture des runs refusée", "Ajoute la permission Actions: Read-only au PAT.");

  // On ne peut pas lire les secrets d'un dépôt via l'API, seulement leurs noms.
  const secrets = await fetch(`https://api.github.com/repos/${repo}/actions/secrets`, { headers });
  if (secrets.ok) {
    const names = (await secrets.json()).secrets?.map((s) => s.name) ?? [];
    const needed = ["SUPABASE_URL", "SUPABASE_SERVICE_ROLE_KEY"];
    const absent = needed.filter((n) => !names.includes(n));
    if (absent.length === 0) ok("github", "secrets Actions configurés");
    else {
      bad(
        "github",
        `secrets Actions manquants : ${absent.join(", ")}`,
        "Sans eux la CI compile mais ne rapporte aucun verdict, et les tâches restent bloquées en awaiting_verification.",
      );
    }
  } else {
    warn("github", "secrets Actions non vérifiables", "Le PAT n'a pas la permission Secrets: Read. Vérifie-les à la main.");
  }
}

// ---------------------------------------------------------------------------
// Rapport
// ---------------------------------------------------------------------------
await checkSupabase().catch((e) => bad("supabase", e.message, ""));
await checkGemini().catch((e) => bad("gemini", e.message, ""));
await checkGitHub().catch((e) => bad("github", e.message, ""));

const icon = { ok: "✓", warn: "!", bad: "✗" };
let area = "";
console.log("");
for (const r of results) {
  if (r.area !== area) {
    area = r.area;
    console.log(`  ${area}`);
  }
  console.log(`    ${icon[r.level]} ${r.msg}`);
  if (r.fix && r.level !== "ok") console.log(`      → ${r.fix}`);
}

const bads = results.filter((r) => r.level === "bad").length;
const warns = results.filter((r) => r.level === "warn").length;

console.log("");
if (bads) {
  console.log(`  ${bads} problème(s) bloquant(s), ${warns} avertissement(s).\n`);
  process.exit(1);
}
console.log(warns ? `  Prêt, avec ${warns} avertissement(s).\n` : "  Tout est opérationnel.\n");

// ---------------------------------------------------------------------------
function loadEnv(file) {
  if (!existsSync(file)) {
    console.error("\n  .env.local absent. Copie .env.example et remplis-le.\n");
    process.exit(1);
  }
  const out = {};
  for (const line of readFileSync(file, "utf8").split(/\r?\n/)) {
    const s = line.trim();
    if (!s || s.startsWith("#") || !s.includes("=")) continue;
    const i = s.indexOf("=");
    out[s.slice(0, i).trim()] = s.slice(i + 1).trim();
  }
  return out;
}

function jwtRole(jwt) {
  try {
    return JSON.parse(Buffer.from(jwt.split(".")[1], "base64url").toString("utf8")).role ?? null;
  } catch {
    return null;
  }
}
