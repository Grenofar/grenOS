# Développer grenOS

> Première version écrite par un agent Codeur le 15 octobre, puis **corrigée** :
> elle décrivait quatre des cinq gardes de travers, et se terminait par la
> phrase « aucune commande ou chemin n'est inventé dans ce document ». Une CI
> verte dit que le fichier se lit ; elle ne dit pas qu'il dit vrai.
>
> Règle qui en découle, et qui vaut pour toute la documentation de ce dépôt :
> **on ne décrit pas un outil sans l'ouvrir.** Chaque description ci-dessous
> est tirée de l'en-tête de l'outil lui-même.

## Ce qu'est grenOS

Une distribution Linux **à base Debian trixie**, assemblée par `live-build`.
Ce qui est à nous, c'est tout le visible : X11 sans composition, `openbox`
pour placer les fenêtres, et notre bureau. Depuis le 18 novembre, il est en
**Rust** pour l'essentiel :

| Partie | Langage | Où | Pourquoi ce choix |
|---|---|---|---|
| la barre, le bureau, le panneau du son | Rust + gtk-rs (natif) | `linux/natif/` | ils tournent en permanence : ils doivent être légers |
| les fenêtres (Réglages, Fichiers, GrenPlace, Jeux, grenHub, grenPerf, Tâches, Mises à jour, l'assistant…) | Rust + Tauri 2, pages HTML/JS | `linux/ui/` | UN processus pour toutes, un seul thème (`web/style.css`) |
| les bibliothèques de mesure et de réglage (`sante`, `jeux`, `magasin`, `ecran`, `heure`, `son`…) | Python 3 | `linux/config/includes.chroot/usr/lib/grenos/` | éprouvées par nos essais ; `pont.py` les appelle en JSON pour grenos-ui |

Chaque `/usr/bin/grenos-*` en Python est resté le **point d'entrée** : il passe
la main à la version Rust dès sa première ligne (`exec grenos-ui <page>`, ou
`natif.passer_la_main`), et `--python` force l'ancienne fenêtre en repli.

Le noyau Rust de `kernel/` est **arrêté depuis le 17 septembre**. Il reste dans
le dépôt, ses images restent téléchargeables, et il n'est plus développé.

## Où vit le produit

Tout est sous `linux/` :

| Chemin | Ce qu'on y trouve |
|---|---|
| `linux/build.sh` | construit l'image, dans un conteneur |
| `linux/auto/config` | les options de `live-build`, dont `--apt-recommends false` |
| `linux/config/package-lists/grenos.list.chroot` | chaque paquet, **chaque ligne justifiée** |
| `linux/config/hooks/normal/` | les gardes de construction : ils refusent l'image |
| `linux/config/includes.chroot/` | **notre bureau** : `usr/bin/grenos-*`, `usr/lib/grenos/*.py`, thèmes, services |
| `linux/data/catalogue.json` | le catalogue de GrenPlace |
| `linux/tools/` | `make-deb.sh`, `make-apt-repo.sh`, `essai-maj.sh`, les `verif-*.py` |

Et pour le Rust :

| Chemin | Ce qu'on y trouve |
|---|---|
| `linux/ui/` | `grenos-ui` : `src/<page>.rs` (les commandes), `web/<page>.js` et `.css` (la page) |
| `linux/natif/` | la barre (`src/bin/barre.rs`), le bureau, le panneau du son |
| `scripts/construire-binaires.sh` | compile les trois caisses Rust, Rust figé, `Cargo.lock` exigé |
| `linux/debian-instantane` | la date où Debian est figé pour les constructions reproductibles (K3) |

`usr/lib/grenos/grenosui.py` tient **la seule palette** du système : la barre
native la lit (`linux/natif/src/theme.rs`), et `grenos-ui` lit les mêmes
réglages (`~/.config/grenos/`).

### Ajouter une fenêtre à grenos-ui

1. `linux/ui/src/<page>.rs` : les commandes `#[tauri::command]`. Ce qui touche
   au système passe par une **clé** ou une **liste blanche**, jamais par une
   commande libre venue de la page (`systeme::ouvrir_programme`, `PERMISES`
   dans `reglages.rs`).
2. `linux/ui/web/<page>.js` et `<page>.css` ; appeler `grenos.pret()` une fois
   la page dessinée — c'est ce qui dit sa taille et son temps d'ouverture à la
   CI (« fenetre … : tient », « ouverture : … »).
3. Inscrire la page : `PAGES` et `invoke_handler!` dans `src/main.rs`, la liste
   de `web/demarrage.js`, la feuille dans `web/index.html`.
4. Des données factices dans `web/apercu.js` : la page s'ouvre alors dans un
   navigateur (`python -m http.server --directory linux/ui/web`, puis `#page`).
5. Les lignes série que la CI attend : `systeme::dire`, au mot près.

## Construire

La construction se fait **dans la CI**, pas sur une machine de développement :
`.github/workflows/linux.yml` construit, démarre l'image dans QEMU avec KVM,
garde les captures, et publie la Release (sur `main` seulement). Une
construction complète prend une heure environ.

Trois raccourcis :

- `.github/workflows/natif.yml` compile et teste `linux/natif` **et**
  `linux/ui` dans Debian trixie en quelques minutes. `cargo test` tourne aussi
  sous Windows pour `linux/ui`, mais sans GIO ni GTK (dépendances Linux
  seulement) : c'est natif.yml qui dit si le code Linux compile.
- `.github/workflows/reproductible.yml` construit l'image **deux fois**, sur
  deux machines, et compare les deux ISO à l'octet (K3). À la main et chaque
  lundi.
- les gardes locales, avant de pousser :
  `for f in linux/tools/verif-*.py; do python3 "$f"; done` depuis la racine.

Deux règles qui ont chacune coûté du temps :

- **Ne jamais pousser sous `linux/` pendant qu'une construction tourne.** Le
  workflow a `cancel-in-progress` : le push l'annule.
- Le **bit d'exécution doit être dans Git** (`git update-index --chmod=+x`),
  sinon `live-build` ignore un hook sans un mot.

## Les gardes du dépôt, et ce que chacun refuse vraiment

Tous tournent au début de `linux.yml`, avant toute construction.

| Outil | Ce qu'il refuse |
|---|---|
| `scripts/verif-flux.py` | un fichier de flux abîmé par une réécriture : un `\n` littéral hors commentaire, un retour chariot isolé, une rafale d'espaces (continuation mangée) ; puis le **schéma GitHub** que PyYAML ignore — identifiants d'étape en double, clés inventées, `${{` jamais fermée, un `run:` de plus de 20 000 caractères. Et depuis le 15 octobre : une porte de la Release qui porte `continue-on-error` sans être nommée dans le verdict honnête |
| `linux/tools/verif-accents.py` | un **texte affiché** sans ses accents (étiquette, infobulle, bouton, titre, champ), et un nombre à l'anglaise — « 2.9 Go » au lieu de « 2,9 Go ». Il ne regarde ni les commentaires ni les noms de classes |
| `linux/tools/verif-shell.py` | une **barre oblique inverse égarée hors guillemets** dans nos scripts shell. Il connaît `$( )` comme un nouveau contexte de citation |
| `linux/tools/verif-recherches.py` | une recherche du parcours d'essai qui **désigne plus d'une application**. Il rejoue la règle exacte du menu sur les séquences déclarées par `# recherche-menu:` dans `scripts/ci-demarrage.sh`, avec la correspondance AZERTY |
| `linux/tools/verif-recommandes.py` | **rien.** Il ne refuse pas : il *liste* les `Recommends` qui seront absents à cause de `--apt-recommends false`, avec un registre `AVIS` où chaque verdict porte sa raison. Sa propre conclusion le dit : « Ce contrôle ne refuse rien » |
| `linux/tools/verif-fenetres.py` | un **appel lent pendant qu'une fenêtre se dessine** — `vulkaninfo` a déjà laissé la page Jeux vide vingt-cinq secondes |

Et pendant la construction, les hooks de `linux/config/hooks/normal/` refusent
l'image : icônes manquantes en 24×24, autorisation polkit qui ne désigne
aucune action déclarée, système de fichiers offert par l'installateur qu'on ne
sait pas formater, décodeur vidéo absent, `nofail` manquant dans les options
de montage.

## La règle qui tient tout le reste

**Une preuve n'est acquise que si la machine l'a vue**, et trois états, jamais
mélangés : *prouvé*, *écrit* (le code existe, personne ne l'a essayé), *à faire*.
Une preuve neuve est **rapportée** jusqu'à deux passages verts, puis devient
bloquante.

Corollaires appris à nos dépens :

- **Un garde qu'on n'a pas vu refuser n'est pas un garde.** Chaque garde neuf
  est exercé dans les deux sens, et sur le **vrai** fichier — une copie de sa
  logique dans l'essai finit par diverger, et c'est alors la copie qu'on croit.
- **Un contournement qui marche ferme la question sans y répondre.**
- **Une mesure fausse ne se contredit jamais** : elle est stable, cohérente, et
  survit à toutes les corrections. C'est son signe distinctif, pas sa fiabilité.
- **Une promesse à l'écran sans rien derrière** est le défaut capital de ce
  dépôt : `partition.conf` offrant btrfs sans `mkfs.btrfs`, une règle polkit
  nommant une action inexistante, `va-driver-all` sans `libavcodec`.

## Déboguer

- **Le port série est la source.** Nos programmes écrivent par `grenos-dire` ;
  la CI les relit dans `serial.log`, gardé en artefact.
- **Regarder les captures, pas seulement les lire.** Une capture qu'on n'ouvre
  pas ne prouve que l'absence d'écran noir. Les `.ppm` se convertissent avec
  `scripts/ci-screen.py`.
- **Lire à la source.** Un `.deb` s'ouvre sans rien installer ; une page de
  manuel Debian dit ce qu'une option fait vraiment. `APT::Update::Error-Mode`,
  qu'on allait poser le 15 octobre, n'existe dans aucune des deux.

## Suivre le travail

`todo.md`, à la racine, porte les demandes numérotées et leur état. `CLAUDE.md`
porte le journal détaillé — il n'est pas versionné.
