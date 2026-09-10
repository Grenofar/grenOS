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
    }
  | { type: "escalate"; reason: string; options?: string[] }
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
  const json = extractJson(raw);

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

  const actions = rawActions.map((a, i) => validateAction(a, i, raw));

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

function validateAction(value: unknown, index: number, raw: string): AgentAction {
  if (!isRecord(value)) throw new EnvelopeError(`actions[${index}] n'est pas un objet`, raw);

  const type = value["type"];
  const at = `actions[${index}] (${String(type)})`;

  const text = (key: string): string => {
    const v = value[key];
    if (typeof v !== "string") throw new EnvelopeError(`${at} : "${key}" doit être une chaîne`, raw);
    return v;
  };

  switch (type) {
    case "write_file":
      return { type, path: text("path"), content: text("content") };

    case "patch_file": {
      const old_str = text("old_str");
      if (!old_str) {
        throw new EnvelopeError(`${at} : old_str vide remplacerait tout le fichier`, raw);
      }
      return { type, path: text("path"), old_str, new_str: text("new_str") };
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
      };
    }

    case "escalate":
      return {
        type,
        reason: text("reason"),
        ...(Array.isArray(value["options"]) ? { options: value["options"].map(String) } : {}),
      };

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
