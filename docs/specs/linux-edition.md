# Spec — grenOS, édition Linux (base Debian)

Écrit par Claude le 2026-09-17 au soir, après la décision de l'humain : « on part
de debian alors, prends en compte tout ce que je t'ai demandé ». Le noyau Rust
est mis de côté ; tout ce qui suit concerne `linux/` dans le même dépôt.
En français : cette spec est lue par Claude et par l'humain ; les tâches données
aux agents restent en anglais.

## 0. Ce que l'humain a demandé, mot pour mot

1. Une interface façon Windows, simple à comprendre, complète, impressionnante.
2. Un **mode jour et nuit**.
3. Un navigateur avec **onglets** et **survol**.
4. Des **survols et des animations partout**, « un truc beau et apaisant ».
5. **Alt+Tab**, **Alt+F4**, **Ctrl+Maj+Échap** (tâches et configuration).
6. Fichiers : **ouvrir, renommer, créer, modifier**, depuis l'explorateur comme
   depuis l'éditeur.
7. Un **écran de connexion qui tient** après extinction.
8. Un **fond d'écran moderne**.
9. Des **réglages d'écran** : résolution et rafraîchissement.
10. **Steam** un jour, par un **magasin grenOS**.
11. **Ne jamais parler de VirtualBox dans l'OS**, sauf dans les astuces.
12. Garder, dans les téléchargements, **l'ISO et la machine VirtualBox**.
13. **Des mises à jour depuis l'OS.**

Les points 1 à 9 sont tenus par le bureau KDE Plasma habillé ; 10 et 13 sont la
partie qui reste à écrire (magasin, dépôt APT) ; 12 est déjà dans la CI.

## 1. Ce qui existe déjà (fait par Claude)

| Chemin | Rôle |
|---|---|
| `linux/auto/config` | les options de `lb config` : trixie, KDE, français, AZERTY, Calamares |
| `linux/config/package-lists/grenos.list.chroot` | les paquets, avec la raison de chacun |
| `linux/config/hooks/normal/0100-grenos-look.hook.chroot` | thème sombre, animations, raccourcis, identité (`/etc/os-release`), fonds d'écran en paquet KDE, écran de connexion, français |
| `linux/config/includes.chroot/usr/bin/grenos-theme` | bascule jour/nuit (thème + fond d'écran), raccourci Meta+T |
| `linux/config/includes.chroot/usr/bin/grenos-maj` | met à jour le système depuis l'OS (`apt`), dans une fenêtre |
| `linux/config/includes.chroot/usr/share/grenos/bienvenue.html` | l'accueil : installer, thème, mise à jour, gestes, astuces |
| `linux/tools/wallpaper.py` | les deux fonds d'écran, calculés (numpy + zlib) |
| `.github/workflows/linux.yml` | construit dans un conteneur Debian, démarre dans QEMU, garde les captures, fabrique le zip VirtualBox, publie une Release |

Règles de fabrication apprises tout de suite :

- **Le bit d'exécution** doit être dans Git (`git update-index --chmod=+x`) :
  sans lui, le conteneur refuse `build.sh` et live-build ignore les hooks.
- **Les noms de paquets se vérifient avant** : `spectacle` n'existe pas chez
  Debian, c'est `kde-spectacle`. Télécharger les `Packages.gz` de trixie
  (main, contrib, non-free, non-free-firmware) et comparer la liste entière
  coûte une minute et évite un échec de dix.
- **L'image dépasse 1 Go** : Supabase est plafonné à 1 Go, la publication passe
  par les **Releases GitHub** (2 Go par fichier).

## 2. Ce qui reste, par ordre d'importance

### T1 — Le magasin grenOS (`grenos-magasin`)
Une fenêtre qui installe des applications en un clic, Steam compris. Première
version honnête : une liste écrite à la main (nom, description, icône, paquet
Debian ou dépôt à ajouter), un bouton Installer qui passe par `pkexec apt-get
install`, et l'état (installé, disponible) lu avec `dpkg-query`. Pas de
gestionnaire de paquets maison : Debian le fait déjà.
Steam demande `dpkg --add-architecture i386` puis `steam-installer` (non-free) ;
le magasin doit le dire avant de le faire, et refuser proprement sans réseau.

### T2 — Le dépôt APT de grenOS
Nos propres paquets (`grenos-desktop` : thème, fonds d'écran, accueil, outils)
publiés dans un dépôt APT signé, pour que `grenos-maj` mette à jour **grenOS**
et pas seulement Debian. Un dépôt APT n'est qu'un ensemble de fichiers
statiques : `dists/stable/{Release,InRelease}`, `dists/stable/main/binary-amd64/Packages.gz`,
`pool/main/g/grenos-desktop/*.deb`. Hébergé dans le bucket public Supabase
`apt`, signé avec une clé OpenPGP dont la partie privée reste dans le bucket
privé `signing`, comme la clé Ed25519 du noyau. L'ISO embarque la clé publique
et `/etc/apt/sources.list.d/grenos.list`.

### T3 — L'identité visuelle
Un schéma de couleurs KDE aux couleurs de grenOS (bleu `#2F7DF6`), un thème
SDDM avec notre fond, un thème Plymouth avec le logo, et des icônes : d'abord
Breeze recoloré, plus tard les nôtres. Tout est du texte (`.colors`, `.conf`,
QML) : c'est du travail d'agent.

### T4 — Les réglages d'écran
KDE fournit résolution et rafraîchissement dans Paramètres système ; il reste à
les rendre évidents : une entrée « Écran » dans l'accueil, et vérifier que le
redimensionnement automatique marche dans une machine virtuelle (additions
invité déjà installées).

### T5 — L'installation
Vérifier Calamares de bout en bout dans la CI : installer sur un disque de 25 Go
dans QEMU, redémarrer dessus, arriver sur l'écran de connexion, se connecter.
C'est la preuve que le point 7 de l'humain est tenu.

## 3. Ce que la CI doit prouver

1. L'image se construit (`lb build`) et fait moins de 2 Go.
2. Elle démarre dans QEMU et **montre un bureau** : la capture à cinq minutes
   n'est ni noire ni uniforme (`scripts/ci-screen.py judge`).
3. Les captures sont gardées en artefact : Claude les regarde.
4. Après T5 : l'installation sur disque aboutit et le système installé démarre.

## 4. Ce qu'on ne fait pas

- Pas de noyau, de libc ni de gestionnaire de paquets maison : Debian les
  fournit, et les correctifs de sécurité avec.
- Pas d'image de plus de 2 Go : Steam s'installe depuis le magasin, il n'est pas
  préinstallé.
- Pas de mention de VirtualBox dans l'interface, sauf dans les astuces de
  l'accueil.
