-- =============================================================================
-- grenOS 0007 — cadrage d'une mission par conversation
--
-- Une mission ne commence bien que si ses critères d'acceptation sont
-- vérifiables par machine. Les écrire soi-même dans un formulaire, c'est
-- deviner ce que la CI saura constater — et une mission mal cadrée coûte trois
-- tentatives d'agent avant que quiconque s'en aperçoive.
--
-- Ici l'humain décrit son intention, le Maître l'interroge jusqu'à pouvoir
-- écrire ces critères lui-même, puis lance la mission.
--
-- Le chat passe par le worker, jamais par Vercel : le site n'exécute aucun
-- agent et ne détient aucune clé de modèle (D-001).
-- =============================================================================

do $$ begin create type draft_role as enum ('user', 'master');
exception when duplicate_object then null; end $$;


create table if not exists draft_messages (
  id          uuid primary key default gen_random_uuid(),
  mission_id  uuid        not null references missions(id) on delete cascade,
  role        draft_role  not null,
  content     text        not null,
  -- Rempli par le worker après avoir répondu : ce qui distingue une question
  -- de l'humain restée sans réponse d'un échange déjà traité.
  answered_at timestamptz,
  model       text,
  tokens_in   integer     not null default 0,
  tokens_out  integer     not null default 0,
  created_at  timestamptz not null default now()
);

create index if not exists draft_messages_mission_idx
  on draft_messages (mission_id, created_at);

-- Les messages humains encore sans réponse : c'est la file que le worker lit.
create index if not exists draft_messages_pending_idx
  on draft_messages (mission_id, created_at)
  where role = 'user' and answered_at is null;


alter table draft_messages enable row level security;

drop policy if exists draft_messages_read on draft_messages;
create policy draft_messages_read on draft_messages
  for select to authenticated using (is_operator());

-- L'humain peut poser ses questions ; il ne peut pas écrire à la place du
-- Maître. Sans ce `role = 'user'`, le navigateur pourrait fabriquer une
-- réponse de l'IA et faire lancer une mission qu'elle n'a jamais validée.
drop policy if exists draft_messages_insert on draft_messages;
create policy draft_messages_insert on draft_messages
  for insert to authenticated
  with check (is_operator() and role = 'user');


-- -----------------------------------------------------------------------------
-- missions : ce que le cadrage produit
-- -----------------------------------------------------------------------------
-- Les critères d'acceptation vivent sur la mission une fois le cadrage terminé.
-- L'Architecte les reçoit comme contrat, au lieu de les inventer.
alter table missions add column if not exists acceptance_criteria jsonb not null default '[]'::jsonb;

-- Distingue une mission cadrée par conversation d'une mission créée à la main.
alter table missions add column if not exists intake_done_at timestamptz;


-- -----------------------------------------------------------------------------
-- Realtime : le chat se met à jour sans polling
-- -----------------------------------------------------------------------------
do $$
begin
  if not exists (
    select 1 from pg_publication_tables
    where pubname = 'supabase_realtime' and tablename = 'draft_messages'
  ) then
    alter publication supabase_realtime add table draft_messages;
  end if;
end $$;


-- -----------------------------------------------------------------------------
-- L'agent qui mène l'entretien
-- -----------------------------------------------------------------------------
-- Même identité que le Maître pour l'utilisateur, prompt distinct : interroger
-- un humain et router des agents sont deux métiers différents.
insert into agents (
  id, name, status, role_class, reports_to, model_role,
  max_tokens_per_task, max_attempts, can_write, allowed_paths, forbidden_paths
) values (
  'intake', 'Master', 'active', 'orchestrator', null, 'master',
  40000, 2, false,
  array[]::text[],
  array['**']
)
on conflict (id) do update set
  name = excluded.name,
  status = excluded.status,
  can_write = excluded.can_write,
  forbidden_paths = excluded.forbidden_paths;
