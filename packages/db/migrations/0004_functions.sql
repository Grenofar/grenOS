-- =============================================================================
-- grenOS 0004 — fonctions appelées par le worker
--
-- Tout ce qui doit être atomique vit ici plutôt que dans le worker. Un worker
-- peut être tué à tout moment (D-002) : si un compteur ou un verrou dépendait
-- d'une séquence lire-modifier-écrire côté Node, un kill au mauvais moment
-- laisserait la base dans un état incohérent.
-- =============================================================================


-- -----------------------------------------------------------------------------
-- bump_model_usage — incrément atomique des compteurs de quota
-- -----------------------------------------------------------------------------
create or replace function bump_model_usage(
  p_day      date,
  p_provider text,
  p_model    text,
  p_in       bigint,
  p_out      bigint,
  p_error    text default null
) returns void
language sql
security definer
set search_path = public
as $$
  insert into model_usage as m (day, provider, model, requests, input_tokens, output_tokens, errors, last_error, updated_at)
  values (
    p_day, p_provider, p_model, 1, p_in, p_out,
    case when p_error is null then 0 else 1 end,
    p_error, now()
  )
  on conflict (day, provider, model) do update set
    requests      = m.requests + 1,
    input_tokens  = m.input_tokens + excluded.input_tokens,
    output_tokens = m.output_tokens + excluded.output_tokens,
    errors        = m.errors + excluded.errors,
    last_error    = coalesce(excluded.last_error, m.last_error),
    updated_at    = now();
$$;


-- -----------------------------------------------------------------------------
-- acquire_leases — prend tous les verrous ou aucun
-- -----------------------------------------------------------------------------
-- Le tout-ou-rien est le point important. Une tâche qui obtiendrait 3 chemins
-- sur 4 puis échouerait laisserait 3 fichiers verrouillés par une tâche qui ne
-- démarre jamais, et le Maître ne pourrait plus les réattribuer.
--
-- Renvoie le tableau des chemins déjà pris, vide si l'acquisition a réussi.
create or replace function acquire_leases(
  p_task_id  uuid,
  p_agent_id text,
  p_paths    text[],
  p_ttl_s    integer default 1800
) returns text[]
language plpgsql
security definer
set search_path = public
as $$
declare
  conflicts text[];
begin
  -- Un worker tué laisse des baux derrière lui : on les recycle d'abord,
  -- sinon un crash gèlerait un fichier jusqu'à intervention manuelle.
  update leases
     set released_at = now()
   where released_at is null
     and expires_at < now();

  select coalesce(array_agg(l.path), '{}')
    into conflicts
    from leases l
   where l.released_at is null
     and l.path = any(p_paths)
     and l.task_id <> p_task_id;

  if array_length(conflicts, 1) > 0 then
    return conflicts;
  end if;

  insert into leases (path, task_id, agent_id, expires_at)
  select p, p_task_id, p_agent_id, now() + make_interval(secs => p_ttl_s)
    from unnest(p_paths) as p
  on conflict (path) where released_at is null
  do update set
    expires_at = excluded.expires_at,
    task_id    = excluded.task_id,
    agent_id   = excluded.agent_id;

  return '{}';
end $$;


create or replace function release_leases(p_task_id uuid)
returns integer
language sql
security definer
set search_path = public
as $$
  with done as (
    update leases set released_at = now()
     where task_id = p_task_id and released_at is null
     returning 1
  )
  select count(*)::integer from done;
$$;


-- -----------------------------------------------------------------------------
-- claim_next_task — un seul worker prend une tâche donnée
-- -----------------------------------------------------------------------------
-- `for update skip locked` : si deux workers tournent en parallèle (ou si un
-- redémarrage en recouvre un autre), ils ne peuvent pas récupérer la même
-- tâche. Sans ça, deux agents traiteraient la même tâche et produiraient deux
-- diffs concurrents.
create or replace function claim_next_task(p_agent_ids text[])
returns tasks
language plpgsql
security definer
set search_path = public
as $$
declare
  claimed tasks;
begin
  select * into claimed
    from tasks
   where status = 'ready'
     and assigned_to = any(p_agent_ids)
   order by created_at
   limit 1
     for update skip locked;

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


-- -----------------------------------------------------------------------------
-- add_mission_tokens — comptabilité de budget, atomique
-- -----------------------------------------------------------------------------
create or replace function add_mission_tokens(
  p_mission_id uuid,
  p_task_id    uuid,
  p_tokens     integer
) returns bigint
language plpgsql
security definer
set search_path = public
as $$
declare
  total bigint;
begin
  update tasks
     set tokens_used = tokens_used + p_tokens
   where id = p_task_id;

  update missions
     set tokens_used = tokens_used + p_tokens
   where id = p_mission_id
   returning tokens_used into total;

  return total;
end $$;


-- -----------------------------------------------------------------------------
-- Ces fonctions sont security definer : appelables uniquement par le worker
-- et la CI, jamais depuis le navigateur.
-- -----------------------------------------------------------------------------
revoke all on function bump_model_usage(date, text, text, bigint, bigint, text) from public, anon, authenticated;
revoke all on function acquire_leases(uuid, text, text[], integer)             from public, anon, authenticated;
revoke all on function release_leases(uuid)                                    from public, anon, authenticated;
revoke all on function claim_next_task(text[])                                 from public, anon, authenticated;
revoke all on function add_mission_tokens(uuid, uuid, integer)                 from public, anon, authenticated;
