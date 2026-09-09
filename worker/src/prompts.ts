import { readFileSync, readdirSync, existsSync } from "node:fs";
import { createHash } from "node:crypto";
import { join } from "node:path";
import { ROOT } from "./config.ts";

/**
 * Loads agents/**\/*.md and turns them into system prompts.
 *
 * These files are the configuration, not documentation (D-007): the body of
 * each file becomes the agent's system prompt, prefixed by the shared protocol
 * in agents/README.md. Changing behaviour is therefore a reviewable git diff,
 * and `promptSha` lets us tell at a glance whether a running agent matches
 * what is on disk.
 */

export interface AgentDefinition {
  id: string;
  name: string;
  status: "active" | "dormant" | "disabled";
  reportsTo: string | null;
  roleClass: "orchestrator" | "planner" | "worker" | "verifier";
  modelRole: "master" | "architect" | "coder" | "tester";
  maxTokensPerTask: number;
  maxAttempts: number;
  canWrite: boolean;
  allowedPaths: string[];
  forbiddenPaths: string[];
  /** README protocol + this agent's own body. */
  systemPrompt: string;
  promptSha: string;
}

const AGENTS_DIR = join(ROOT, "agents");

export function loadAgents(): AgentDefinition[] {
  const protocolPath = join(AGENTS_DIR, "README.md");
  if (!existsSync(protocolPath)) {
    throw new Error(`agents/README.md introuvable sous ${AGENTS_DIR}`);
  }
  const protocol = readFileSync(protocolPath, "utf8");

  const files = [
    ...listMarkdown(AGENTS_DIR),
    ...listMarkdown(join(AGENTS_DIR, "sub")),
  ].filter((f) => !f.endsWith("README.md"));

  const agents = files.map((file) => parseAgent(file, protocol));

  const seen = new Set<string>();
  for (const agent of agents) {
    if (seen.has(agent.id)) {
      throw new Error(
        `Deux agents partagent l'id "${agent.id}". Les ids sont référencés en base, ils doivent être uniques.`,
      );
    }
    seen.add(agent.id);
  }
  return agents;
}

function listMarkdown(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .filter((e) => e.isFile() && e.name.endsWith(".md"))
    .map((e) => join(dir, e.name))
    .sort();
}

function parseAgent(file: string, protocol: string): AgentDefinition {
  const raw = readFileSync(file, "utf8");
  const match = raw.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n([\s\S]*)$/);
  if (!match) {
    throw new Error(`${file} n'a pas de frontmatter YAML délimité par ---`);
  }
  const [, frontmatter, body] = match as unknown as [string, string, string];
  const meta = parseFrontmatter(frontmatter);

  const id = str(meta, "id", file);

  // The protocol is prepended rather than duplicated into every file: one
  // edit to agents/README.md changes the rules for the whole team at once.
  const systemPrompt = `${protocol.trim()}\n\n---\n\n${body.trim()}`;

  return {
    id,
    name: str(meta, "name", file),
    status: str(meta, "status", file) as AgentDefinition["status"],
    reportsTo: (meta["reports_to"] as string | undefined) ?? null,
    roleClass: str(meta, "role_class", file) as AgentDefinition["roleClass"],
    modelRole: str(meta, "model_role", file) as AgentDefinition["modelRole"],
    maxTokensPerTask: Number(meta["max_tokens_per_task"] ?? 60000),
    maxAttempts: Number(meta["max_attempts"] ?? 3),
    canWrite: meta["can_write"] === true,
    allowedPaths: list(meta["allowed_paths"]),
    // Enforced for every agent regardless of what its file says. A prompt is a
    // request; the sandbox is the law (agents/README.md §1).
    forbiddenPaths: [
      ...list(meta["forbidden_paths"]),
      "agents/**",
      ".github/workflows/**",
      "**/.env*",
    ],
    systemPrompt,
    promptSha: createHash("sha256").update(systemPrompt).digest("hex"),
  };
}

/**
 * Minimal YAML subset: scalars, inline lists, and block lists. Enough for the
 * frontmatter we control, and far cheaper than a YAML dependency. Anything it
 * cannot parse throws rather than guessing, so a malformed agent file fails
 * loudly at boot instead of silently losing its path restrictions.
 */
function parseFrontmatter(text: string): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  const lines = text.split(/\r?\n/);
  let currentKey: string | null = null;
  let currentList: string[] | null = null;

  for (const raw of lines) {
    const line = raw.replace(/\s+#.*$/, "").trimEnd();
    if (!line.trim()) continue;

    const item = line.match(/^\s+-\s*(.+)$/);
    if (item && currentList) {
      currentList.push(unquote(item[1]!));
      continue;
    }

    const kv = line.match(/^([A-Za-z_][A-Za-z0-9_]*):\s*(.*)$/);
    if (!kv) throw new Error(`Frontmatter illisible : "${raw}"`);

    if (currentKey && currentList) out[currentKey] = currentList;
    currentList = null;

    const [, key, rest] = kv as unknown as [string, string, string];
    currentKey = key;

    if (rest === "") {
      currentList = [];
    } else if (rest.startsWith("[")) {
      out[key] = rest
        .slice(1, rest.lastIndexOf("]"))
        .split(",")
        .map((s) => unquote(s.trim()))
        .filter(Boolean);
    } else if (rest === "true" || rest === "false") {
      out[key] = rest === "true";
    } else if (rest === "null" || rest === "~") {
      out[key] = null;
    } else if (/^-?\d+$/.test(rest)) {
      out[key] = Number(rest);
    } else {
      out[key] = unquote(rest);
    }
  }
  if (currentKey && currentList) out[currentKey] = currentList;
  return out;
}

const unquote = (s: string): string => s.replace(/^["'](.*)["']$/, "$1");

function str(meta: Record<string, unknown>, key: string, file: string): string {
  const value = meta[key];
  if (typeof value !== "string" || !value) {
    throw new Error(`${file} : champ "${key}" manquant ou invalide dans le frontmatter`);
  }
  return value;
}

function list(value: unknown): string[] {
  if (Array.isArray(value)) return value.map(String);
  if (typeof value === "string" && value) return [value];
  return [];
}
