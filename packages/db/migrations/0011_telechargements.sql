-- Le compteur de telechargements du site public.
--
-- CE QU'IL ENREGISTRE, ET POURQUOI SI PEU. Trois colonnes : la version de
-- l'image, le type de fichier, et l'instant. Pas d'adresse IP, pas
-- d'identifiant, pas de temoin de session, rien qui permette de reconnaitre
-- quelqu'un d'une fois sur l'autre.
--
-- grenOS affiche « vos donnees vous appartiennent » sur l'ecran de
-- confidentialite de son assistant, et il n'active aucune telemetrie par
-- defaut. Un site qui poserait un mouchard a cote de ce discours le viderait
-- de son sens. Cette table repond a UNE question — combien de fois cette
-- version a-t-elle ete prise — et elle est incapable d'en repondre une autre.
--
-- L'IMAGE ELLE-MEME N'EST PAS ICI. Elle pese plus d'un gigaoctet, et Supabase
-- Storage est plafonne a 1 Go : elle est publiee en Release GitHub. Supabase
-- sert au compteur, pas au fichier.

create table if not exists public.telechargements (
  id bigserial primary key,
  -- L'etiquette de la Release, par exemple « linux-20261015-1859 ».
  version text not null,
  -- « iso » ou « virtualbox ». Contraint, parce qu'une colonne libre finit
  -- par porter trois orthographes de la meme chose et ne se compte plus.
  genre text not null check (genre in ('iso', 'virtualbox')),
  cree_le timestamptz not null default now()
);

-- On compte par version : c'est la seule interrogation que la page fait.
create index if not exists telechargements_version_idx
  on public.telechargements (version);

alter table public.telechargements enable row level security;

-- N'IMPORTE QUI PEUT AJOUTER UNE LIGNE, ET PERSONNE NE PEUT EN LIRE LE DETAIL.
--
-- L'insertion est ouverte au visiteur anonyme : c'est le but, le site n'a que
-- la clef anonyme. Mais la lecture ligne a ligne est fermee — sans quoi
-- n'importe qui pourrait lire les horodatages et reconstituer qui a
-- telecharge quoi et quand, ce que cette table est precisement concue pour
-- ne pas permettre.
--
-- La page n'a pas besoin de lire les lignes : elle demande un COMPTE, par la
-- fonction ci-dessous, qui ne rend qu'un nombre.
drop policy if exists telechargements_insert_anon on public.telechargements;
create policy telechargements_insert_anon
  on public.telechargements
  for insert
  to anon, authenticated
  with check (true);

-- Combien de fois une version a ete prise. `security definer` parce que la
-- lecture directe est fermee : la fonction est le seul chemin, et elle ne
-- rend qu'un entier.
create or replace function public.compte_telechargements(p_version text)
returns bigint
language sql
security definer
set search_path = public
as $$
  select count(*) from public.telechargements where version = p_version;
$$;

grant execute on function public.compte_telechargements(text) to anon, authenticated;
