-- =============================================================================
-- grenOS 0002 — Row Level Security
--
-- Modèle d'accès :
--   service_role (worker + CI) : contourne RLS, écrit tout.
--   anon / authenticated (site): lecture seule, sauf créer une mission et
--                                actionner le coupe-circuit.
--
-- Autrement dit : le navigateur ne peut jamais fabriquer une tâche ni un
-- verdict. Il demande une mission, le worker décide. C'est ce qui empêche
-- qu'une faille du front devienne une faille du système d'agents.
-- =============================================================================

-- -----------------------------------------------------------------------------
-- Liste blanche (2 personnes max, D-006)
-- -----------------------------------------------------------------------------
create table if not exists operators (
  email      text primary key,
  label      text,
  added_at   timestamptz not null default now()
);

alter table operators enable row level security;

-- Remplace par tes adresses. Un compte Supabase Auth dont l'email n'est pas
-- ici est authentifié mais ne voit rien.
insert into operators (email, label) values
  ('belgacemmaroua@gmail.com', 'Grenofar')
on conflict (email) do nothing;


-- -----------------------------------------------------------------------------
-- is_operator() — le seul prédicat utilisé par toutes les policies
-- -----------------------------------------------------------------------------
-- security definer : la fonction lit `operators` sans que l'appelant ait le
-- droit de la lire lui-même, sinon la policy s'auto-bloquerait.
create or replace function is_operator() returns boolean
language sql
stable
security definer
set search_path = public
as $$
  select exists (
    select 1 from operators
    where email = coalesce(auth.jwt() ->> 'email', '')
  );
$$;

revoke all on function is_operator() from public;
grant execute on function is_operator() to authenticated, anon;


-- -----------------------------------------------------------------------------
-- Activation de RLS partout
-- -----------------------------------------------------------------------------
alter table agents      enable row level security;
alter table missions    enable row level security;
alter table tasks       enable row level security;
alter table messages    enable row level security;
alter table artifacts   enable row level security;
alter table runs        enable row level security;
alter table leases      enable row level security;
alter table model_usage enable row level security;
alter table events      enable row level security;
alter table settings    enable row level security;


-- -----------------------------------------------------------------------------
-- Lecture : les opérateurs voient tout
-- -----------------------------------------------------------------------------
do $$
declare t text;
begin
  foreach t in array array['agents', 'missions', 'tasks', 'messages', 'artifacts',
                           'runs', 'leases', 'model_usage', 'events', 'settings',
                           'operators'] loop
    execute format('drop policy if exists %I on %I', t || '_read', t);
    execute format(
      'create policy %I on %I for select to authenticated using (is_operator())',
      t || '_read', t
    );
  end loop;
end $$;


-- -----------------------------------------------------------------------------
-- Écriture : très restreinte
-- -----------------------------------------------------------------------------

-- Créer une mission depuis le site. Le worker fait le reste.
drop policy if exists missions_insert on missions;
create policy missions_insert on missions
  for insert to authenticated
  with check (is_operator() and created_by = auth.uid());

-- Un opérateur peut annuler une mission, rien de plus.
-- Il ne peut pas la marquer 'done' : seule la CI décide de ça (D-009).
drop policy if exists missions_abort on missions;
create policy missions_abort on missions
  for update to authenticated
  using (is_operator())
  with check (is_operator() and status in ('aborted', 'draft', 'planning'));

-- Coupe-circuit : le bouton "stop" de l'UI écrit ici.
drop policy if exists settings_toggle on settings;
create policy settings_toggle on settings
  for update to authenticated
  using (is_operator() and key in ('agents_paused', 'max_concurrent_tasks'))
  with check (is_operator() and key in ('agents_paused', 'max_concurrent_tasks'));


-- -----------------------------------------------------------------------------
-- Ce qui n'a AUCUNE policy d'écriture, donc reste impossible depuis le site :
--   tasks, messages, runs, leases, model_usage, events, agents, artifacts
--
-- Le front ne peut pas inventer une tâche, effacer un verdict de CI, ni
-- réécrire le journal. Absence de policy = refus par défaut sous RLS.
-- -----------------------------------------------------------------------------


-- -----------------------------------------------------------------------------
-- Vues de confort pour le dashboard
-- -----------------------------------------------------------------------------

-- security_invoker : la vue s'exécute avec les droits de l'appelant, donc RLS
-- s'applique normalement au travers. Sans ça une vue devient un contournement.
create or replace view mission_overview
with (security_invoker = true) as
select
  m.id,
  m.title,
  m.status,
  m.token_budget,
  m.tokens_used,
  m.created_at,
  count(t.id)                                            as task_count,
  count(t.id) filter (where t.status = 'done')           as tasks_done,
  count(t.id) filter (where t.status = 'failed')         as tasks_failed,
  count(t.id) filter (where t.status in ('in_progress',
                                         'awaiting_verification')) as tasks_active
from missions m
left join tasks t on t.mission_id = m.id
group by m.id;

create or replace view usage_today
with (security_invoker = true) as
select
  provider,
  model,
  requests,
  input_tokens,
  output_tokens,
  errors
from model_usage
where day = current_date;
