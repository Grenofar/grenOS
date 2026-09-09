-- =============================================================================
-- grenOS 0001 — schéma de base
--
-- À exécuter dans Supabase : SQL Editor → New query → coller → Run.
-- Idempotent : réexécutable sans casser une base existante.
-- =============================================================================

create extension if not exists "pgcrypto";

-- -----------------------------------------------------------------------------
-- Énumérations
-- -----------------------------------------------------------------------------
do $$ begin
  create type agent_status  as enum ('active', 'dormant', 'disabled');
  create type role_class    as enum ('orchestrator', 'planner', 'worker', 'verifier');
  create type mission_status as enum ('draft', 'planning', 'running', 'blocked', 'done', 'aborted');
  -- awaiting_verification : le Codeur a rendu, la CI n'a pas encore tranché.
  -- C'est l'état qui matérialise D-009 : personne ne valide son propre travail.
  create type task_status   as enum ('pending', 'ready', 'in_progress',
                                     'awaiting_verification', 'done',
                                     'failed', 'blocked', 'cancelled');
  create type run_status    as enum ('queued', 'running', 'passed', 'failed', 'error', 'timeout');
  create type failure_class as enum ('compile_error', 'test_failure', 'spec_gap',
                                     'capability_gap', 'provider_error', 'policy_violation');
  create type event_level   as enum ('debug', 'info', 'warn', 'error');
exception when duplicate_object then null; end $$;


-- -----------------------------------------------------------------------------
-- agents — miroir en base de agents/**/*.md
-- La source de vérité reste les fichiers .md (D-007). Cette table sert au
-- routage, aux compteurs et à l'affichage ; elle est resynchronisée au boot.
-- -----------------------------------------------------------------------------
create table if not exists agents (
  id                  text primary key,
  name                text        not null,
  status              agent_status not null default 'dormant',
  role_class          role_class  not null,
  reports_to          text        references agents(id) on delete set null,
  model_role          text        not null,
  max_tokens_per_task integer     not null default 60000,
  max_attempts        smallint    not null default 3,
  can_write           boolean     not null default false,
  allowed_paths       text[]      not null default '{}',
  forbidden_paths     text[]      not null default '{}',
  prompt_sha256       text,
  synced_at           timestamptz,
  created_at          timestamptz not null default now()
);

comment on column agents.prompt_sha256 is
  'Empreinte du .md chargé. Permet de détecter une dérive entre le fichier et le comportement observé.';


-- -----------------------------------------------------------------------------
-- missions — une demande humaine
-- -----------------------------------------------------------------------------
create table if not exists missions (
  id            uuid primary key default gen_random_uuid(),
  title         text            not null,
  description   text            not null default '',
  status        mission_status  not null default 'draft',
  token_budget  bigint          not null default 2000000,
  tokens_used   bigint          not null default 0,
  created_by    uuid            references auth.users(id) on delete set null,
  created_at    timestamptz     not null default now(),
  updated_at    timestamptz     not null default now(),
  finished_at   timestamptz,
  constraint missions_budget_positive check (token_budget > 0)
);

create index if not exists missions_status_idx on missions (status, created_at desc);


-- -----------------------------------------------------------------------------
-- tasks — l'enveloppe de tâche (agents/README.md §3)
-- -----------------------------------------------------------------------------
create table if not exists tasks (
  id                  uuid primary key default gen_random_uuid(),
  mission_id          uuid        not null references missions(id) on delete cascade,
  parent_task_id      uuid        references tasks(id) on delete set null,
  assigned_to         text        not null references agents(id),
  goal                text        not null,
  acceptance_criteria jsonb       not null default '[]'::jsonb,
  context_refs        jsonb       not null default '[]'::jsonb,
  allowed_paths       text[]      not null default '{}',
  status              task_status not null default 'pending',
  attempt             smallint    not null default 1,
  max_attempts        smallint    not null default 3,
  token_budget        integer     not null default 60000,
  tokens_used         integer     not null default 0,
  deadline_s          integer     not null default 1800,
  failure             failure_class,
  failure_detail      text,
  result              jsonb,
  branch              text,
  created_at          timestamptz not null default now(),
  updated_at          timestamptz not null default now(),
  started_at          timestamptz,
  finished_at         timestamptz,

  -- Une tâche sans critère vérifiable est malformée (agents/README.md §3).
  -- La règle est appliquée ici, pas laissée à la bonne volonté du Maître.
  constraint tasks_criteria_not_empty
    check (jsonb_array_length(acceptance_criteria) > 0),
  constraint tasks_attempt_within_limit
    check (attempt <= max_attempts)
);

create index if not exists tasks_mission_idx  on tasks (mission_id, created_at);
create index if not exists tasks_status_idx   on tasks (status) where status in ('ready', 'in_progress', 'awaiting_verification');
create index if not exists tasks_assigned_idx on tasks (assigned_to, status);


-- -----------------------------------------------------------------------------
-- messages — tout le trafic Maître ↔ agents, append-only
-- -----------------------------------------------------------------------------
create table if not exists messages (
  id          uuid primary key default gen_random_uuid(),
  mission_id  uuid        references missions(id) on delete cascade,
  task_id     uuid        references tasks(id) on delete cascade,
  from_agent  text        not null,
  to_agent    text        not null,
  kind        text        not null default 'result',
  content     jsonb       not null,
  model       text,
  tokens_in   integer     not null default 0,
  tokens_out  integer     not null default 0,
  latency_ms  integer,
  created_at  timestamptz not null default now()
);

create index if not exists messages_task_idx    on messages (task_id, created_at);
create index if not exists messages_mission_idx on messages (mission_id, created_at desc);


-- -----------------------------------------------------------------------------
-- artifacts — designs, diffs, logs. Le gros contenu va dans Storage.
-- -----------------------------------------------------------------------------
create table if not exists artifacts (
  id           uuid primary key default gen_random_uuid(),
  mission_id   uuid        references missions(id) on delete cascade,
  task_id      uuid        references tasks(id) on delete set null,
  kind         text        not null,
  path         text,
  storage_key  text,
  preview      text,
  bytes        integer     not null default 0,
  created_at   timestamptz not null default now()
);

create index if not exists artifacts_mission_idx on artifacts (mission_id, created_at desc);


-- -----------------------------------------------------------------------------
-- runs — un passage de CI. Écrit par GitHub Actions avec la service_role.
-- C'est la seule source de vérité du système (D-009).
-- -----------------------------------------------------------------------------
create table if not exists runs (
  id              uuid primary key default gen_random_uuid(),
  task_id         uuid        references tasks(id) on delete cascade,
  mission_id      uuid        references missions(id) on delete cascade,
  branch          text        not null,
  commit_sha      text,
  workflow_run_id bigint,
  status          run_status  not null default 'queued',
  failure         failure_class,
  -- [{ criterion, verdict: PASS|FAIL|UNVERIFIABLE, evidence }]
  verdicts        jsonb       not null default '[]'::jsonb,
  log_excerpt     text,
  log_url         text,
  started_at      timestamptz not null default now(),
  finished_at     timestamptz
);

create index if not exists runs_task_idx   on runs (task_id, started_at desc);
create index if not exists runs_status_idx on runs (status) where status in ('queued', 'running');


-- -----------------------------------------------------------------------------
-- leases — un seul écrivain par chemin (agents/README.md §5)
-- Deux agents ne peuvent pas éditer le même fichier. Garanti par la base,
-- pas par la discipline des modèles.
-- -----------------------------------------------------------------------------
create table if not exists leases (
  id          uuid primary key default gen_random_uuid(),
  path        text        not null,
  task_id     uuid        not null references tasks(id) on delete cascade,
  agent_id    text        not null references agents(id),
  acquired_at timestamptz not null default now(),
  expires_at  timestamptz not null,
  released_at timestamptz
);

-- Le cœur du verrou : un seul bail actif par chemin.
create unique index if not exists leases_one_active_per_path
  on leases (path) where released_at is null;

create index if not exists leases_expiry_idx on leases (expires_at) where released_at is null;


-- -----------------------------------------------------------------------------
-- model_usage — consommation par jour et par modèle
-- Le routeur s'en sert pour répartir la charge sur gemini 3.6 / 3.7 / 3.8 :
-- les quotas étant par modèle, trois modèles = trois fois le quota.
-- -----------------------------------------------------------------------------
create table if not exists model_usage (
  day           date    not null default current_date,
  provider      text    not null,
  model         text    not null,
  requests      integer not null default 0,
  input_tokens  bigint  not null default 0,
  output_tokens bigint  not null default 0,
  errors        integer not null default 0,
  last_error    text,
  updated_at    timestamptz not null default now(),
  primary key (day, provider, model)
);


-- -----------------------------------------------------------------------------
-- events — journal append-only, alimente la timeline de l'UI
-- -----------------------------------------------------------------------------
create table if not exists events (
  id         bigserial primary key,
  mission_id uuid        references missions(id) on delete cascade,
  task_id    uuid        references tasks(id) on delete cascade,
  agent_id   text,
  level      event_level not null default 'info',
  type       text        not null,
  message    text        not null,
  payload    jsonb       not null default '{}'::jsonb,
  created_at timestamptz not null default now()
);

create index if not exists events_mission_idx on events (mission_id, id desc);
create index if not exists events_level_idx   on events (level, id desc) where level in ('warn', 'error');


-- -----------------------------------------------------------------------------
-- settings — coupe-circuit et réglages globaux
-- -----------------------------------------------------------------------------
create table if not exists settings (
  key        text primary key,
  value      jsonb       not null,
  updated_at timestamptz not null default now()
);

insert into settings (key, value) values
  ('agents_paused',        'false'::jsonb),
  ('max_concurrent_tasks', '3'::jsonb)
on conflict (key) do nothing;


-- -----------------------------------------------------------------------------
-- updated_at automatique
-- -----------------------------------------------------------------------------
create or replace function touch_updated_at() returns trigger
language plpgsql as $$
begin
  new.updated_at = now();
  return new;
end $$;

drop trigger if exists missions_touch on missions;
create trigger missions_touch before update on missions
  for each row execute function touch_updated_at();

drop trigger if exists tasks_touch on tasks;
create trigger tasks_touch before update on tasks
  for each row execute function touch_updated_at();


-- -----------------------------------------------------------------------------
-- Realtime — c'est ce qui rend le dashboard vivant sans polling
-- -----------------------------------------------------------------------------
do $$
begin
  if not exists (select 1 from pg_publication where pubname = 'supabase_realtime') then
    create publication supabase_realtime;
  end if;
end $$;

do $$
declare t text;
begin
  foreach t in array array['missions', 'tasks', 'runs', 'events', 'agents'] loop
    if not exists (
      select 1 from pg_publication_tables
      where pubname = 'supabase_realtime' and tablename = t
    ) then
      execute format('alter publication supabase_realtime add table %I', t);
    end if;
  end loop;
end $$;
