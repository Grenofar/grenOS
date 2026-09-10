-- =============================================================================
-- grenOS 0009 — un verdict ne s'applique qu'à une tâche qui l'attend
--
-- Constaté sur la mission 1. La création d'une branche d'agent était un push
-- du contenu de main : la CI tournait sur une branche sans kernel, rendait un
-- verdict `spec_gap`, et le trigger l'appliquait à la tâche — qui était en
-- train de s'exécuter. Il la remettait en `ready`, un second worker la prenait,
-- et deux exécutions concurrentes écrivaient sur la même tâche.
--
-- Un verdict n'a de sens que pour une tâche en `awaiting_verification` : c'est
-- le seul état où l'agent a rendu son travail et attend la CI. Dans tout autre
-- état, le verdict porte sur un commit périmé ou fantôme, et l'appliquer ne
-- peut que corrompre l'état. Il est enregistré, jamais exécuté.
--
-- Identique à 0005 à ce garde-fou près.
-- =============================================================================

create or replace function apply_run_verdict() returns trigger
language plpgsql
security definer
set search_path = public
as $$
declare
  t tasks;
begin
  if new.task_id is null or new.status not in ('passed','failed','error','timeout') then
    return new;
  end if;

  select * into t from tasks where id = new.task_id;
  if not found then
    return new;
  end if;

  if t.status <> 'awaiting_verification' then
    insert into events (mission_id, task_id, agent_id, level, type, message)
    values (new.mission_id, new.task_id, 'tester', 'info', 'stale_verdict',
            'Verdict ignoré sur ' || new.branch || ' : la tâche était « ' ||
            t.status || ' » et n''attendait pas de vérification.');
    return new;
  end if;

  if new.status = 'passed' then
    update tasks
       set status = 'done',
           failure = null,
           failure_detail = null,
           finished_at = now()
     where id = new.task_id;

    insert into events (mission_id, task_id, agent_id, level, type, message)
    values (new.mission_id, new.task_id, 'tester', 'info', 'ci_passed',
            'CI verte sur ' || new.branch);

  else
    if t.attempt < t.max_attempts then
      update tasks
         set status = 'ready',
             attempt = t.attempt + 1,
             failure = coalesce(new.failure, 'test_failure'),
             failure_detail = left(coalesce(new.log_excerpt, 'CI rouge, pas de log'), 4000)
       where id = new.task_id;
    else
      update tasks
         set status = 'failed',
             failure = coalesce(new.failure, 'test_failure'),
             failure_detail = left(coalesce(new.log_excerpt, 'CI rouge, pas de log'), 4000),
             finished_at = now()
       where id = new.task_id;
    end if;

    insert into events (mission_id, task_id, agent_id, level, type, message)
    values (new.mission_id, new.task_id, 'tester', 'warn',
            coalesce(new.failure::text, 'test_failure'),
            'CI rouge sur ' || new.branch ||
            ' (tentative ' || t.attempt || '/' || t.max_attempts || ')');
  end if;

  update leases set released_at = now()
   where task_id = new.task_id and released_at is null;

  return new;
end $$;
