/** Mirrors the tables in packages/db/migrations. Kept hand-written and small
 *  rather than generated, so the shapes the UI relies on stay obvious. */

export type AgentStatus = "active" | "dormant" | "disabled";
export type RoleClass = "orchestrator" | "planner" | "worker" | "verifier";

export type MissionStatus =
  | "draft" | "planning" | "running" | "blocked" | "done" | "aborted";

export type TaskStatus =
  | "pending" | "ready" | "in_progress" | "awaiting_verification"
  | "done" | "failed" | "blocked" | "cancelled";

export type RunStatus = "queued" | "running" | "passed" | "failed" | "error" | "timeout";
export type EventLevel = "debug" | "info" | "warn" | "error";

export interface Agent {
  id: string;
  name: string;
  status: AgentStatus;
  role_class: RoleClass;
  reports_to: string | null;
  model_role: string;
  can_write: boolean;
}

export interface Mission {
  id: string;
  title: string;
  description: string;
  status: MissionStatus;
  token_budget: number;
  tokens_used: number;
  created_at: string;
}

export interface MissionOverview extends Mission {
  task_count: number;
  tasks_done: number;
  tasks_failed: number;
  tasks_active: number;
}

export interface Task {
  id: string;
  mission_id: string;
  assigned_to: string;
  goal: string;
  status: TaskStatus;
  attempt: number;
  max_attempts: number;
  failure: string | null;
  created_at: string;
}

export interface Run {
  id: string;
  task_id: string | null;
  branch: string;
  commit_sha: string | null;
  status: RunStatus;
  failure: string | null;
  verdicts: Array<{ criterion: string; verdict: string; evidence?: string }>;
  log_excerpt: string | null;
  started_at: string;
  finished_at: string | null;
}

export interface AppEvent {
  id: number;
  mission_id: string | null;
  task_id: string | null;
  agent_id: string | null;
  level: EventLevel;
  type: string;
  message: string;
  /** For an escalation, the options the Master offers the human. */
  payload?: { options?: string[]; [key: string]: unknown } | null;
  created_at: string;
}

export interface ModelUsage {
  provider: string;
  model: string;
  requests: number;
  input_tokens: number;
  output_tokens: number;
  errors: number;
}
