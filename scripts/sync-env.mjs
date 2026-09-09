#!/usr/bin/env node
/**
 * Derive the NEXT_PUBLIC_* Supabase variables from their private twins and
 * check .env.local for the mistakes that actually happen.
 *
 * Runs automatically before `npm run dev` and `npm run worker`, so the two
 * pairs can never drift apart. Values are never printed: everything this
 * script reports is masked, because the most common way a key leaks is a
 * terminal scrollback pasted into a chat or an issue.
 */

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const file = join(root, ".env.local");

if (!existsSync(file)) {
  console.error("\n  .env.local absent. Copie .env.example et remplis-le.\n");
  process.exit(1);
}

let text = readFileSync(file, "utf8");

const read = (key) => {
  const m = text.match(new RegExp(`^${key}=(.*)$`, "m"));
  return m ? m[1].trim() : "";
};

const write = (key, value) => {
  const line = `${key}=${value}`;
  if (new RegExp(`^${key}=`, "m").test(text)) {
    text = text.replace(new RegExp(`^${key}=.*$`, "m"), line);
  } else {
    text += `\n${line}\n`;
  }
};

const mask = (v) =>
  !v ? "vide" : v.length <= 12 ? `${v.length} car.` : `${v.slice(0, 6)}…${v.slice(-4)}`;

const url = read("SUPABASE_URL");
const anon = read("SUPABASE_ANON_KEY");
const service = read("SUPABASE_SERVICE_ROLE_KEY");

const problems = [];
const notes = [];

// --- Recopie automatique ---------------------------------------------------
if (url && anon) {
  if (read("NEXT_PUBLIC_SUPABASE_URL") !== url || read("NEXT_PUBLIC_SUPABASE_ANON_KEY") !== anon) {
    write("NEXT_PUBLIC_SUPABASE_URL", url);
    write("NEXT_PUBLIC_SUPABASE_ANON_KEY", anon);
    writeFileSync(file, text, "utf8");
    notes.push("NEXT_PUBLIC_* synchronisées depuis les valeurs privées");
  }
}

// --- Vérifications ---------------------------------------------------------
const required = {
  SUPABASE_URL: url,
  SUPABASE_ANON_KEY: anon,
  SUPABASE_SERVICE_ROLE_KEY: service,
  GEMINI_API_KEY: read("GEMINI_API_KEY"),
  GITHUB_TOKEN: read("GITHUB_TOKEN"),
};

for (const [key, value] of Object.entries(required)) {
  if (!value) problems.push(`${key} est vide`);
}

if (url && !/^https:\/\/[a-z0-9-]+\.supabase\.co\/?$/.test(url)) {
  problems.push("SUPABASE_URL ne ressemble pas à https://xxxx.supabase.co");
}

// La confusion la plus coûteuse du projet, et la plus facile à faire :
// les deux clés se ressemblent et sont côte à côte dans le dashboard.
if (anon && service && anon === service) {
  problems.push(
    "anon et service_role sont IDENTIQUES — la service_role contourne toute " +
      "la sécurité RLS. Si elle atteint le navigateur, n'importe qui lit et " +
      "écrit toute la base. Recopie les deux depuis Supabase → Settings → API.",
  );
}

// Un JWT Supabase porte son rôle dans le payload : on peut détecter l'inversion
// sans jamais afficher la clé.
const roleOf = (jwt) => {
  try {
    const part = jwt.split(".")[1];
    if (!part) return null;
    return JSON.parse(Buffer.from(part, "base64url").toString("utf8")).role ?? null;
  } catch {
    return null;
  }
};

if (anon && roleOf(anon) === "service_role") {
  problems.push(
    "SUPABASE_ANON_KEY contient en fait la clé service_role. Inversion à corriger " +
      "immédiatement : cette valeur est recopiée dans NEXT_PUBLIC_* et partirait " +
      "en clair dans le JavaScript du site.",
  );
}
if (service && roleOf(service) === "anon") {
  problems.push("SUPABASE_SERVICE_ROLE_KEY contient en fait la clé anon (inversion).");
}

// Google émet au moins deux formats de clé : l'historique "AIza..." et le
// plus récent "AQ....". Les deux sont valides — vérifié en appelant l'API.
// N'avertir que sur ce qui ne ressemble à aucun des deux.
{
  const gem = read("GEMINI_API_KEY");
  if (gem && !gem.startsWith("AIza") && !gem.startsWith("AQ.")) {
    notes.push("GEMINI_API_KEY a un format inhabituel — attendu 'AIza...' ou 'AQ....'");
  }
}

// --- Rapport ---------------------------------------------------------------
console.log("\n  .env.local");
for (const [key, value] of Object.entries(required)) {
  console.log(`    ${value ? "✓" : "·"} ${key.padEnd(28)} ${mask(value)}`);
}
for (const n of notes) console.log(`\n  → ${n}`);

if (problems.length) {
  console.error("\n  À corriger :");
  for (const p of problems) console.error(`    ✗ ${p}`);
  console.error("");
  process.exit(1);
}

console.log("\n  Tout est en place.\n");
