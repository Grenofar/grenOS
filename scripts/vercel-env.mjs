#!/usr/bin/env node
/**
 * Print the exact block to paste into Vercel → Settings → Environment
 * Variables (its bulk field accepts KEY=value lines).
 *
 * Only the two NEXT_PUBLIC_* variables are ever printed. The site runs no
 * agent, so it has no use for the service_role key, the Gemini key or the PAT
 * — and anything behind a NEXT_PUBLIC_ prefix is shipped in clear text to
 * every visitor of a public repo. This script refuses to emit them, so the
 * mistake cannot be made by copying its output.
 */

import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const file = join(root, ".env.local");

if (!existsSync(file)) {
  console.error("\n  .env.local absent. Copie .env.example et remplis-le.\n");
  process.exit(1);
}

const text = readFileSync(file, "utf8");
const read = (key) => {
  const m = text.match(new RegExp(`^${key}=(.*)$`, "m"));
  return m ? m[1].trim() : "";
};

const url = read("SUPABASE_URL") || read("NEXT_PUBLIC_SUPABASE_URL");
const anon = read("SUPABASE_ANON_KEY") || read("NEXT_PUBLIC_SUPABASE_ANON_KEY");

// A JWT carries its role in the payload, so an inverted pair can be caught
// before it is pasted into a public site rather than after.
const roleOf = (jwt) => {
  try {
    return JSON.parse(Buffer.from(jwt.split(".")[1], "base64url").toString("utf8")).role ?? null;
  } catch {
    return null;
  }
};

if (anon && roleOf(anon) === "service_role") {
  console.error(
    "\n  STOP : SUPABASE_ANON_KEY contient la clé service_role.\n" +
      "  Collée dans Vercel, elle partirait en clair dans le JavaScript du site\n" +
      "  et donnerait à n'importe qui un accès total à la base.\n" +
      "  Reprends les deux clés dans Supabase → Settings → API.\n",
  );
  process.exit(1);
}

const missing = !url || !anon;

console.log(`
  Vercel → Settings → General
    Root Directory = apps/web

  Vercel → Settings → Environment Variables
    Coller ceci (Production + Preview + Development) :

NEXT_PUBLIC_SUPABASE_URL=${url || "https://TON-PROJET.supabase.co"}
NEXT_PUBLIC_SUPABASE_ANON_KEY=${anon || "TA_CLE_ANON"}
`);

if (missing) {
  console.log(
    "  (valeurs manquantes : remplis SUPABASE_URL et SUPABASE_ANON_KEY dans\n" +
      "   .env.local, puis relance `npm run vercel` pour obtenir le bloc réel)\n",
  );
} else {
  console.log("  Rien d'autre. Ni service_role, ni Gemini, ni le PAT.\n");
}
