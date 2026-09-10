-- =============================================================================
-- grenOS 0006 — rattrapage
--
-- À passer sur toute base créée avant le 2026-09-10. Sans effet sur une base
-- fraîche : 0001 contient déjà tout ceci, cette migration existe pour les
-- bases où 0001 avait déjà été exécutée quand la colonne a été ajoutée.
--
-- Entièrement idempotente.
-- =============================================================================


-- -----------------------------------------------------------------------------
-- messages.seen_at — sans elle, le Maître ne voit rien
-- -----------------------------------------------------------------------------
-- Le Maître filtre les messages non traités avec `.is("seen_at", null)`. Si la
-- colonne n'existe pas, PostgREST renvoie une erreur, le worker reçoit une
-- liste vide, et le Maître ne prend jamais connaissance de ce que ses agents
-- lui ont renvoyé. Aucune erreur visible : le système avance en ignorant la
-- moitié de ce qu'il produit.
alter table messages add column if not exists seen_at timestamptz;

create index if not exists messages_unseen_idx on messages (mission_id, created_at desc)
  where seen_at is null;


-- -----------------------------------------------------------------------------
-- Contrôle : ce que cette migration garantit
-- -----------------------------------------------------------------------------
do $$
declare missing text[] := '{}';
begin
  if not exists (select 1 from information_schema.columns
                 where table_name = 'messages' and column_name = 'seen_at') then
    missing := missing || 'messages.seen_at';
  end if;

  if array_length(missing, 1) > 0 then
    raise exception 'Rattrapage incomplet : %', array_to_string(missing, ', ');
  end if;

  raise notice 'Base à jour.';
end $$;
