-- =============================================================================
-- grenOS 0010 — parler au Maître pendant une mission
--
-- Le chat de mission réutilise draft_messages, sans changement de schéma : la
-- conversation de cadrage et celle qui suit le lancement sont une seule
-- conversation avec le même Maître, et la RLS existante (l'humain n'insère que
-- role = 'user') vaut pour les deux. Le worker ne répond plus seulement aux
-- brouillons : le cycle du Maître lit les messages d'une mission en cours avant
-- tout le reste (worker/src/master.ts).
--
-- Deux corrections vues en conditions réelles l'accompagnent.
-- =============================================================================


-- -----------------------------------------------------------------------------
-- 1. Une mission escaladée ne consomme plus rien
-- -----------------------------------------------------------------------------
-- Le 2026-09-10, le Maître a escaladé la mission 1 et le Codeur a pourtant pris
-- sa tâche dans la minute : claim_next_task ne regardait que la tâche, jamais
-- sa mission. Une escalade est une question posée à l'humain ; tant qu'il n'a
-- pas répondu, dépenser des tokens sur cette mission, c'est décider à sa place.
create or replace function claim_next_task(p_agent_ids text[])
returns tasks
language plpgsql
security definer
set search_path = public
as $$
declare
  claimed tasks;
begin
  select t.* into claimed
    from tasks t
    join missions m on m.id = t.mission_id
   where t.status = 'ready'
     and t.assigned_to = any(p_agent_ids)
     and m.status in ('planning', 'running')
   order by t.created_at
   limit 1
     for update of t skip locked;

  if not found then
    return null;
  end if;

  update tasks
     set status = 'in_progress',
         started_at = now()
   where id = claimed.id
   returning * into claimed;

  return claimed;
end $$;

revoke all on function claim_next_task(text[]) from public, anon, authenticated;


-- -----------------------------------------------------------------------------
-- 2. La carte compte les tâches qui comptent
-- -----------------------------------------------------------------------------
-- Une tâche annulée a été remplacée : la compter affichait la mission 1 à
-- « 1 tâche sur 6 » alors que quatre des six n'existaient plus que dans
-- l'historique. Mêmes colonnes, dans le même ordre, que la 0008.
create or replace view roadmap_progress
with (security_invoker = true) as
select
  r.position,
  r.key,
  r.title,
  r.goal,
  r.owner_agent,
  r.done_when,
  r.mission_id,
  m.title       as mission_title,
  m.status      as mission_status,
  m.tokens_used,
  m.token_budget,
  case
    when m.status = 'done'                              then 'done'
    when m.status in ('planning', 'running')            then 'active'
    when m.status = 'blocked'                           then 'blocked'
    when m.status = 'aborted'                           then 'aborted'
    when m.status = 'draft'                             then 'scoping'
    else 'todo'
  end as state,
  coalesce((select count(*) from tasks t where t.mission_id = m.id and t.status <> 'cancelled'), 0) as task_count,
  coalesce((select count(*) from tasks t where t.mission_id = m.id and t.status = 'done'), 0)        as tasks_done
from roadmap r
left join missions m on m.id = r.mission_id
order by r.position;
