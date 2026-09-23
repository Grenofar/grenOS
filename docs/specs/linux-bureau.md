# Le bureau de grenOS

> Réécrit par Claude le 2026-09-23, **en remplacement d'une version d'agent
> presque entièrement inventée** (barre en haut, programmes « en TypeScript »,
> compte renommé d'office en `grenofar`, catalogue servi par le dépôt APT :
> rien de tout cela n'existait). Chaque fait ci-dessous a été relu dans le
> fichier qu'il décrit. Si tu modifies le code, corrige cette page — un
> document faux est pire que pas de document, parce que les agents le lisent
> comme une source.

## 1. Ce qui démarre, dans l'ordre

```
lightdm  →  /usr/share/xsessions/grenos.desktop  →  /usr/bin/grenos-session
                                                        ├── grenos-bureau   (le fond et les icônes)
                                                        ├── grenos-shell    (la barre et le menu)
                                                        └── exec openbox    (les fenêtres)
```

`grenos-session` (un script shell) fait, dans cet ordre : les variables
d'environnement, la langue lue dans `/etc/default/locale`, l'écran qui ne
s'éteint pas, la définition d'écran retenue, la présence ou non de
l'installateur selon qu'on est sur une vraie machine, les services discrets
(presse-papiers des machines virtuelles, agent d'autorisations), le bureau, la
preuve de session, la barre, et l'accueil au premier lancement. Il finit par
`exec openbox` : **c'est openbox qui tient la session**, et quand il s'arrête,
la session se ferme.

`grenos-bureau` et `grenos-shell` sont lancés par `grenos-veilleur`, qui les
relance s'ils tombent — un bureau sans barre est un bureau perdu.

**Rien n'est composé par la carte graphique.** C'est le choix fondateur : Plasma
a été retiré le 18 septembre 2026 parce qu'il tombait à trois images par
seconde dans une machine virtuelle sans accélération 3D.

## 2. Les programmes

Tous sont écrits en **Python 3 avec GTK 3**, dans `linux/config/includes.chroot/usr/bin/`.

| Programme | Ce qu'il fait |
|---|---|
| `grenos-shell` | La barre **en bas**, icônes au centre : bouton grenOS, épinglées, fenêtres ouvertes, heure. Le menu s'ouvre sur le bouton ou par Ctrl+Échap. Elle réserve sa bande par `_NET_WM_STRUT`, posé avec `xprop` (GTK ne l'écrit pas). |
| `grenos-bureau` | Le fond d'écran **et** les icônes du dossier `~/Bureau`. La même fenêtre peint le papier peint : personne ne se dispute la racine de l'écran. |
| `grenos-premier` | Le tout premier écran : il demande un nom. Voir §4. |
| `grenos-parametres` | Sept pages : apparence, écran, son, réseau, compte, mises à jour, matériel. |
| `grenos-taches` | Le gestionnaire de tâches : jauges processeur et mémoire, liste triable, fin de tâche. Tout est lu dans `/proc`. |
| `grenplace` | Le magasin. Voir §5. |
| `grenos-maj` | La mise à jour, dans une fenêtre à barre de progression — jamais un terminal. |
| `grenos-bienvenue` | L'accueil, au premier lancement puis dans le menu. |
| `grenos-arret` | Fermer la session, redémarrer, éteindre. |
| `grenos-theme` | Bascule jour/nuit (Meta+T), et prévient la barre par `SIGUSR1`. |
| `grenos-fond` | Pose le fond d'écran (utile hors session et pour l'écran de connexion). |
| `grenos-menu` | Ouvre le menu : envoie `SIGUSR2` à la barre. |
| `grenos-dire` | Dépose un billet que `grenos-preuve` relaie sur le port série. |
| `grenos-preuve` | Service root : relaie les billets, et annonce l'ouverture de session. |

Les modules partagés sont dans `/usr/lib/grenos/` : `grenosui.py` (**toute la
charte graphique et la seule palette**), `ecran.py` (RandR), `son.py`
(PipeWire par `pactl`), `materiel.py` (`/proc`, `lspci`), `verif-style.py`
(contrôle des feuilles de style à la construction).

## 3. Où vivent les choix de la personne

Dans `~/.config/grenos/` :

| Fichier | Contenu réel |
|---|---|
| `theme` | `jour` ou `nuit` — **pas** `light`/`dark` |
| `barre.json` | La liste des applications épinglées : `[{nom, commande, icone}]` |
| `bureau.json` | La position des icônes : `{"nom du fichier": [x, y]}` |
| `ecran` | Trois lignes : sortie, définition, fréquence (ce que `xrandr` doit réappliquer) |
| `fond` | Le chemin de l'image de fond choisie — **c'est ce fichier, pas `ecran`** |
| `accueil-vu` | Marque que l'accueil a déjà été montré |
| `installer-pose` | Marque que le raccourci d'installation a déjà été posé |

Et à l'échelle de la machine : `/etc/grenos/identite` (`login`, `nom`,
`machine`), écrit une seule fois par `grenos-premier`.

## 4. Le premier démarrage : le nom

`grenos-premier.service` s'exécute **avant lightdm**, une seule fois
(`ConditionPathExists=!/etc/grenos/identite`), sur son propre serveur X
(vt7, par `grenos-premier-x`).

Il **demande un nom à la personne** — il n'en impose aucun. « Léa Martin »
devient le compte `lea`, la maison `/home/lea`, et l'adresse `lea@grenos`,
affichée en direct pendant la frappe. Puis il renomme le compte d'uid 1000
(`usermod -l -d -m`, `groupmod -n`), écrit `/etc/grenos/identite`,
`/etc/lightdm/lightdm.conf` (l'ouverture automatique) et
`/etc/sudoers.d/grenos-identite`.

**Deux pièges déjà payés**, à ne pas réintroduire :

- LightDM lit `lightdm.conf` **après** `lightdm.conf.d/` : c'est donc lui qui
  décide, et live-config y écrit le compte du live. Écrire seulement dans
  `lightdm.conf.d/` laissait une machine où personne ne pouvait entrer.
- Si le renommage échoue, on **garde l'ancien nom** plutôt que d'ouvrir la
  session vers un compte qui n'existe pas.

## 5. GrenPlace et son catalogue

GrenPlace **ne connaît aucune application**. Au lancement il demande le
catalogue à notre backend :

```
https://tpqzhzuoyqpfairatdrw.supabase.co/storage/v1/object/public/catalogue/magasin.json
```

Trois sources, dans cet ordre : en ligne → la copie gardée dans
`~/.cache/grenos/catalogue.json` → celle livrée dans
`/usr/share/grenos/catalogue.json`.

La source du catalogue est `linux/data/catalogue.json`, publiée par
`linux/tools/publier-catalogue.py` (qui valide chaque fiche avant de déposer).
**Cela n'a aucun rapport avec le dépôt APT** : ce sont deux mécanismes
distincts.

Une fiche porte : `slug`, `nom`, `rayon`, `editeur`, `description`, `logo`
(une **adresse**, l'image n'est pas stockée chez nous), `source`
(`flatpak` ou `apt`), `identifiant`, `site`.

**La fiche dit quoi installer, jamais comment.** La commande est construite
par le client, et tout identifiant qui ne ressemble pas à un nom de paquet est
refusé (`^[A-Za-z0-9][A-Za-z0-9._+-]*$`). Sans cela, modifier le catalogue
reviendrait à faire exécuter n'importe quoi sur toutes les machines.

## 6. Les mises à jour

Le dépôt APT de grenOS est un ensemble de fichiers statiques dans le bucket
public Supabase `apt`, signé par une clé OpenPGP que la CI fabrique une seule
fois et garde dans le bucket **privé** `signing`.

L'image embarque la clé publique dans `/usr/share/keyrings/grenos-apt.gpg` et
la source dans `/etc/apt/sources.list.d/`, avec
`signed-by=/usr/share/keyrings/grenos-apt.gpg` — **pas** dans
`trusted.gpg.d`, qui ferait confiance à cette clé pour tous les dépôts.

Le paquet `grenos-desktop` contient exactement les fichiers de l'image : les
programmes `grenos-*`, les modules de `/usr/lib/grenos/`, le thème des
fenêtres, la configuration d'openbox, les fonds d'écran et le catalogue de
secours. `grenos-maj` lance `apt-get update` puis `apt-get upgrade` : les
nouveautés de grenOS et les correctifs Debian arrivent ensemble.

## 7. Ce que la CI prouve, et ce qu'elle ne prouve pas

`.github/workflows/linux.yml` construit l'image, la démarre dans QEMU, tape un
nom au premier écran, puis **parle au bureau** et exige sur le port série :

```
grenos: premier ecran affiche, il demande un nom
grenos: nom choisi, la machine est a <nom>@grenos
grenos: session de bureau ouverte
grenos: barre prete, N epinglees
grenos: menu ouvert, N applications
grenos: taches ouvert, N processus
```

Si l'une manque, **rien n'est publié**.

Ce qu'elle ne prouve **pas**, à ce jour : l'installation sur disque
(Calamares), le son sur une vraie carte, le clic de souris (seuls des
raccourcis clavier sont envoyés), et la mise à jour d'une machine installée.
