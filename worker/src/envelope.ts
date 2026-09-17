/**
 * Parsing and validation of the JSON envelope every agent must return
 * (agents/README.md §4).
 *
 * Models are asked for raw JSON via responseMimeType, but they still
 * occasionally wrap it in a code fence or prepend a sentence. Recovering from
 * that is worth a few lines here: the alternative is burning a whole retry —
 * and a whole task attempt — on a formatting slip that changes nothing about
 * the work the agent actually did.
 *
 * What we do NOT do is repair the *content*. An envelope missing its status,
 * or carrying an unknown action type, is rejected. Guessing what an agent meant
 * is how a system starts doing things nobody asked for.
 */

export type AgentStatus = "done" | "needs_input" | "failed" | "delegated";

export type AgentAction =
  | { type: "write_file"; path: string; content: string }
  | { type: "patch_file"; path: string; old_str: string; new_str: string }
  | { type: "delete_file"; path: string }
  | { type: "request_build"; profile?: string }
  | { type: "request_test"; suite?: string }
  | { type: "request_help"; capability: string; question: string }
  | {
      type: "propose_task";
      assigned_to: string;
      goal: string;
      acceptance_criteria: string[];
      allowed_paths?: string[];
      /** The task whose branch this one builds on, or "main" (lineage.ts). */
      continue_from?: string;
    }
  | { type: "escalate"; reason: string; options?: string[] }
  /**
   * The Master says the mission is finished, with one line of evidence per
   * acceptance criterion (the green run or merged branch that proves it).
   * Without it a mission never closes: "no new task this cycle" used to mean
   * "done", and on 2026-09-16 a four-task mission closed twice — after its
   * plan, then after its first task.
   */
  | { type: "complete_mission"; evidence: string[] }
  /**
   * Read a document before writing (consult.ts). The host allowlist is
   * enforced where the fetch happens, not here: a refused URL is answered
   * with the list of allowed hosts, not by failing the attempt.
   */
  | { type: "consult"; url: string; why?: string }
  /**
   * Ends a mission-intake conversation and launches the mission.
   *
   * Only the intake agent emits this, and emitting it locks the chat: the
   * human cannot add anything afterwards. That is why the criteria carried
   * here have to be checkable without asking anyone.
   */
  | {
      type: "finalize_mission";
      title: string;
      description: string;
      acceptance_criteria: string[];
      /** The docs/ROADMAP.md step this mission implements, when it is one. */
      roadmap_key?: string;
    };

export interface AgentEnvelope {
  task_id?: string;
  status: AgentStatus;
  summary: string;
  reasoning_brief?: string;
  actions: AgentAction[];
  tokens_used?: number;
}

export class EnvelopeError extends Error {
  // Champs déclarés puis assignés : Node exécute ce TypeScript sans build
  // (mode strip-only), et les « parameter properties » n'y sont pas supportées.
  readonly raw: string;

  constructor(message: string, raw: string) {
    super(message);
    this.name = "EnvelopeError";
    this.raw = raw;
  }
}

const STATUSES = new Set<AgentStatus>(["done", "needs_input", "failed", "delegated"]);

export function parseEnvelope(raw: string): AgentEnvelope {
  // File contents may come outside the JSON, in raw blocks (splitBlocks):
  // they are taken out first, so the braces of the Rust inside them are
  // never mistaken for the envelope.
  const { rest, blocks } = splitBlocks(raw);
  const json = extractJson(rest);

  let value: unknown;
  try {
    value = JSON.parse(json);
  } catch (err) {
    throw new EnvelopeError(
      `Réponse non parsable en JSON : ${err instanceof Error ? err.message : err}`,
      raw,
    );
  }

  if (!isRecord(value)) throw new EnvelopeError("La réponse n'est pas un objet", raw);

  const status = value["status"];
  if (typeof status !== "string" || !STATUSES.has(status as AgentStatus)) {
    throw new EnvelopeError(
      `status invalide : ${JSON.stringify(status)}. Attendu : ${[...STATUSES].join(" | ")}`,
      raw,
    );
  }

  const summary = value["summary"];
  if (typeof summary !== "string" || !summary.trim()) {
    throw new EnvelopeError("summary manquant ou vide", raw);
  }

  const rawActions = value["actions"] ?? [];
  if (!Array.isArray(rawActions)) throw new EnvelopeError("actions doit être un tableau", raw);

  const actions = rawActions.map((a, i) => validateAction(a, i, raw, blocks));

  return {
    ...(typeof value["task_id"] === "string" ? { task_id: value["task_id"] } : {}),
    status: status as AgentStatus,
    summary: summary.trim(),
    ...(typeof value["reasoning_brief"] === "string"
      ? { reasoning_brief: value["reasoning_brief"] }
      : {}),
    actions,
    ...(typeof value["tokens_used"] === "number"
      ? { tokens_used: value["tokens_used"] }
      : {}),
  };
}

/**
 * Raw blocks taken out of an answer, and what is left of it. Pure.
 *
 * A model quoting a whole Rust file inside a JSON string has to escape every
 * quote, backslash and newline of it, and one slip loses the answer: on
 * 2026-09-16 Nemotron's answers failed on "Bad Unicode escape" and "Expected
 * double-quoted property name" three times in one task. So an action may name
 * a block instead (`content_block`, `old_block`, `new_block`), and the block
 * follows the JSON, unescaped:
 *
 *     -----BEGIN BLOCK main-----
 *     fn main() {}
 *     -----END BLOCK main-----
 *
 * The block is the lines between the two markers, each ending with a newline.
 */
export function splitBlocks(raw: string): { rest: string; blocks: Map<string, string> } {
  const blocks = new Map<string, string>();
  const lines = raw.split(/\r?\n/);
  const kept: string[] = [];
  for (let i = 0; i < lines.length; i++) {
    const begin = /^-----BEGIN BLOCK ([A-Za-z0-9_.-]+)-----\s*$/.exec(lines[i]!);
    if (!begin) {
      kept.push(lines[i]!);
      continue;
    }
    const id = begin[1]!;
    const end = lines.findIndex((line, j) => j > i && line.trim() === `-----END BLOCK ${id}-----`);
    if (end === -1) {
      kept.push(lines[i]!);
      continue;
    }
    const inner = lines.slice(i + 1, end);
    blocks.set(id, inner.map((line) => `${line}\n`).join(""));
    i = end;
  }
  return { rest: kept.join("\n"), blocks };
}

function validateAction(
  value: unknown,
  index: number,
  raw: string,
  blocks: Map<string, string> = new Map(),
): AgentAction {
  if (!isRecord(value)) throw new EnvelopeError(`actions[${index}] n'est pas un objet`, raw);

  const type = value["type"];
  const at = `actions[${index}] (${String(type)})`;

  const text = (key: string): string => {
    const v = value[key];
    if (typeof v !== "string") throw new EnvelopeError(`${at} : "${key}" doit être une chaîne`, raw);
    return v;
  };

  // A string field, or the raw block its `<name>_block` sibling names.
  const textOrBlock = (key: string, blockKey: string): string => {
    const id = value[blockKey];
    if (typeof id === "string") {
      const block = blocks.get(id.trim());
      if (block === undefined) {
        throw new EnvelopeError(
          `${at} : ${blockKey} "${id}" introuvable. Écris le bloc après le JSON, entre ` +
            `-----BEGIN BLOCK ${id.trim()}----- et -----END BLOCK ${id.trim()}----- sur leurs propres lignes`,
          raw,
        );
      }
      return block;
    }
    return text(key);
  };

  switch (type) {
    case "write_file":
      return { type, path: text("path"), content: textOrBlock("content", "content_block") };

    case "patch_file": {
      const old_str = textOrBlock("old_str", "old_block");
      if (!old_str) {
        throw new EnvelopeError(`${at} : old_str vide remplacerait tout le fichier`, raw);
      }
      return { type, path: text("path"), old_str, new_str: textOrBlock("new_str", "new_block") };
    }

    case "delete_file":
      return { type, path: text("path") };

    case "request_build":
      return {
        type,
        ...(typeof value["profile"] === "string" ? { profile: value["profile"] } : {}),
      };

    case "request_test":
      return {
        type,
        ...(typeof value["suite"] === "string" ? { suite: value["suite"] } : {}),
      };

    case "request_help":
      return { type, capability: text("capability"), question: text("question") };

    case "propose_task": {
      const criteria = value["acceptance_criteria"];
      if (!Array.isArray(criteria) || criteria.length === 0) {
        // Mirrors the CHECK constraint on tasks: a task nobody can verify is
        // not a task (agents/README.md §3).
        throw new EnvelopeError(`${at} : acceptance_criteria vide`, raw);
      }
      return {
        type,
        assigned_to: text("assigned_to"),
        goal: text("goal"),
        acceptance_criteria: criteria.map(String),
        ...(Array.isArray(value["allowed_paths"])
          ? { allowed_paths: value["allowed_paths"].map(String) }
          : {}),
        ...(typeof value["continue_from"] === "string" && value["continue_from"].trim()
          ? { continue_from: value["continue_from"].trim() }
          : {}),
      };
    }

    case "escalate":
      return {
        type,
        reason: text("reason"),
        ...(Array.isArray(value["options"]) ? { options: value["options"].map(String) } : {}),
      };

    case "complete_mission": {
      const evidence = value["evidence"];
      if (!Array.isArray(evidence) || evidence.length === 0) {
        throw new EnvelopeError(`${at} : evidence vide (une ligne par critère d'acceptation)`, raw);
      }
      return { type, evidence: evidence.map(String) };
    }

    case "consult":
      return {
        type,
        url: text("url").trim(),
        ...(typeof value["why"] === "string" ? { why: value["why"] } : {}),
      };

    case "finalize_mission": {
      const criteria = value["acceptance_criteria"];
      if (!Array.isArray(criteria) || criteria.length === 0) {
        // Launching a mission nobody can grade is the exact failure this whole
        // conversation exists to prevent.
        throw new EnvelopeError(`${at} : acceptance_criteria vide`, raw);
      }
      const rawKey = value["roadmap_key"];
      const roadmapKey = typeof rawKey === "string" ? rawKey.trim() : "";
      return {
        type,
        title: text("title"),
        description: text("description"),
        acceptance_criteria: criteria.map(String),
        ...(roadmapKey ? { roadmap_key: roadmapKey } : {}),
      };
    }

    default:
      throw new EnvelopeError(`${at} : type d'action inconnu`, raw);
  }
}

/** Strip a code fence or surrounding prose, then take the outermost object. */
function extractJson(raw: string): string {
  const text = raw.trim();

  const fenced = text.match(/```(?:json)?\s*\r?\n([\s\S]*?)```/);
  const candidate = (fenced?.[1] ?? text).trim();

  if (candidate.startsWith("{")) {
    const end = matchingBrace(candidate);
    if (end !== -1) return candidate.slice(0, end + 1);
  }

  const start = candidate.indexOf("{");
  if (start !== -1) {
    const end = matchingBrace(candidate.slice(start));
    if (end !== -1) return candidate.slice(start, start + end + 1);
  }

  return candidate;
}

/** Index of the brace closing the one at position 0, ignoring braces in strings. */
function matchingBrace(text: string): number {
  let depth = 0;
  let inString = false;
  let escaped = false;

  for (let i = 0; i < text.length; i++) {
    const ch = text[i]!;

    if (escaped) {
      escaped = false;
      continue;
    }
    if (ch === "\\" && inString) {
      escaped = true;
      continue;
    }
    if (ch === '"') {
      inString = !inString;
      continue;
    }
    if (inString) continue;

    if (ch === "{") depth++;
    else if (ch === "}" && --depth === 0) return i;
  }
  return -1;
}

const isRecord = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);
