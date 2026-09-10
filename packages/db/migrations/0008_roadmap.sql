-- =============================================================================
-- grenOS 0008 — la feuille de route, visible et vivante
--
-- docs/ROADMAP.md dit où va le projet, mais un fichier markdown ne dit jamais
-- où on en est. Cette table relie chaque étape à sa mission : l'avancement
-- devient une conséquence de ce que la CI a réellement validé, jamais une case
-- cochée à la main.
--
-- L'ordre des étapes n'est pas administratif, il est imposé par la matière :
-- sans sortie série on ne sait pas pourquoi le kernel a redémarré, sans
-- gestionnaire d'exceptions on ne distingue pas une page fault d'un triple
-- fault, sans pagination on ne mappe pas les registres d'un périphérique.
-- =============================================================================

create table if not exists roadmap (
  position     smallint primary key,
  key          text        not null unique,
  title        text        not null,
  goal         text        not null,
  -- L'agent propriétaire, tel que routé par agents/00-master.md.
  owner_agent  text        references agents(id) on delete set null,
  -- Ce que la CI devra constater. C'est ce qui rend l'étape terminable.
  done_when    jsonb       not null default '[]'::jsonb,
  -- Renseigné quand une mission est ouverte pour cette étape.
  mission_id   uuid        references missions(id) on delete set null,
  created_at   timestamptz not null default now()
);

alter table roadmap enable row level security;

drop policy if exists roadmap_read on roadmap;
create policy roadmap_read on roadmap
  for select to authenticated using (is_operator());

-- Aucune policy d'écriture : l'avancement se déduit des missions, il ne
-- s'édite pas depuis le navigateur.


insert into roadmap (position, key, title, goal, owner_agent, done_when) values
  (1, 'boot-serial', 'Boot et port série',
   'Faire booter un kernel minimal et lui faire écrire son nom sur COM1.',
   'coder',
   '["cargo build --release passe", "une image bootable est produite", "la chaîne grenOS apparaît sur la console série", "aucun panic ni triple fault", "moins de 90 secondes"]'),

  (2, 'gdt-idt', 'GDT, IDT et exceptions',
   'Installer les tables de descripteurs et des gestionnaires d''exception.',
   'kernel',
   '["une page fault volontaire imprime son adresse et son code d''erreur", "le kernel ne redémarre pas"]'),

  (3, 'phys-mem', 'Mémoire physique',
   'Parser la carte mémoire Limine et allouer des frames.',
   'kernel',
   '["la carte mémoire est imprimée sur le port série", "alloc puis free d''une frame, prouvés par des compteurs"]'),

  (4, 'paging', 'Pagination',
   'Mapper et démapper des pages virtuelles.',
   'kernel',
   '["une page fraîchement mappée est lisible et inscriptible", "une page démappée provoque une faute capturée, pas un reset"]'),

  (5, 'heap', 'Tas',
   'Un allocateur global utilisable depuis le kernel.',
   'coder',
   '["Vec et String fonctionnent dans le kernel", "alloc et dealloc répétés ne fuient pas, prouvé par un compteur"]'),

  (6, 'timer-irq', 'Timer et interruptions',
   'Recevoir des interruptions matérielles périodiques.',
   'kernel',
   '["un compteur de ticks progresse sur la console série", "les interruptions sont réactivées sans faute"]'),

  (7, 'pci', 'Énumération PCI',
   'Découvrir les périphériques exposés par QEMU.',
   'drivers',
   '["chaque périphérique est listé avec son vendor id et device id", "le contrôleur VirtIO est trouvé"]'),

  (8, 'virtio-block', 'Driver VirtIO block',
   'Lire un secteur sur un disque virtuel.',
   'drivers',
   '["le secteur 0 d''une image préparée est lu", "les octets lus correspondent exactement au contenu attendu"]'),

  (9, 'keyboard', 'Clavier PS/2',
   'Traduire une touche pressée en caractère.',
   'drivers',
   '["une touche envoyée à QEMU produit le bon caractère sur le port série"]'),

  (10, 'vfs', 'Système de fichiers en lecture',
   'Monter une image et lire un fichier.',
   'filesystem',
   '["un répertoire connu est listé", "le contenu d''un fichier connu est lu et vérifié octet à octet"]')

on conflict (position) do update set
  key = excluded.key,
  title = excluded.title,
  goal = excluded.goal,
  owner_agent = excluded.owner_agent,
  done_when = excluded.done_when;


-- -----------------------------------------------------------------------------
-- L'avancement, déduit et non déclaré
-- -----------------------------------------------------------------------------
-- Une étape est terminée quand sa mission l'est, et une mission ne se termine
-- que sur des verdicts de CI (D-009). Personne ne peut cocher une case.
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
  coalesce((select count(*) from tasks t where t.mission_id = m.id), 0)                            as task_count,
  coalesce((select count(*) from tasks t where t.mission_id = m.id and t.status = 'done'), 0)      as tasks_done
from roadmap r
left join missions m on m.id = r.mission_id
order by r.position;
