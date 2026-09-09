-- =============================================================================
-- grenOS 0005 — raccordement de la CI
--
-- GitHub Actions ne connaît que le nom de la branche. Le rattachement d'un run
-- à sa tâche et à sa mission se fait ici, pas dans le workflow : un fichier
-- YAML n'a pas à savoir comment nos tables sont reliées, et le worker n'a pas
-- à sonder GitHub en boucle pour l'apprendre.
-- =============================================================================

-- -----------------------------------------------------------------------------
-- resolve_run_task — remplit task_id et mission_id depuis la branche
-- -----------------------------------------------------------------------------
-- Les agents travaillent sur agent/<8 premiers caractères de l'uuid de tâche>.
create or replace function resolve_run_task() returns trigger
language plpgsql
security definer
set search_path = public
as $$
declare
  prefix text;
begin
  if new.task_id is not null then
    return new;
  end if;

  prefix := substring(new.branch from 'agent/([0-9a-f]{8})');
  if prefix is null then
    return new;
  end if;

  select t.id, t.mission_id
    into new.task_id, new.mission_id
    from tasks t
   where t.id::text like prefix || '%'
   limit 1;

  return new;
end $$;

drop trigger if exists runs_resolve_task on runs;
create trigger runs_resolve_task
  before insert on runs
  for each row execute function resolve_run_task();


-- -----------------------------------------------------------------------------
-- apply_run_verdict — la CI décide, la tâche suit
-- -----------------------------------------------------------------------------
-- C'est ici que D-009 devient mécanique : une tâche ne passe à 'done' que
-- lorsqu'un run réel est vert. Aucun agent ne peut écrire ce statut lui-même,
-- puisque seule la CI insère dans `runs` (avec la service_role).
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
    -- Une tentative restante : la tâche repart au Codeur avec le log réel.
    -- Sinon elle échoue et le Maître devra escalader (agents/README.md §7).
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

  -- Les baux sont libérés quoi qu'il arrive : une tâche qui a fini de passer
  -- en CI ne doit plus bloquer ses fichiers.
  update leases set released_at = now()
   where task_id = new.task_id and released_at is null;

  return new;
end $$;

drop trigger if exists runs_apply_verdict on runs;
create trigger runs_apply_verdict
  after insert or update of status on runs
  for each row execute function apply_run_verdict();
