# grenOS, édition Linux

Une image live Debian **trixie** avec **notre propre bureau** — barre, menu,
Réglages, magasin et accueil sont écrits pour grenOS —, le français et le
clavier AZERTY, et l'installateur **Calamares** pour la poser sur un disque.

> KDE Plasma a été retiré le 18 septembre 2026. Il composait l'écran par le
> GPU, ce qui donnait trois images par seconde dans une machine virtuelle sans
> accélération 3D. À la place : X11 **sans composition**, openbox pour placer
> les fenêtres, et nos programmes par-dessus.

## Construire

Tout se construit dans un conteneur Debian privilégié — `live-build` monte des
systèmes de fichiers, ce qu'une machine hôte ne prête pas volontiers :

```sh
docker run --privileged -v "$PWD:/work" -w /work debian:trixie linux/build.sh
```

Résultat : `linux/grenos-amd64.hybrid.iso`, à démarrer en BIOS comme en UEFI,
copiable telle quelle sur une clé USB.

## Ce que contient le dépôt

| Chemin | Rôle |
|---|---|
| `auto/config` | les options de `lb config` : Debian, KDE, français, Calamares |
| `config/package-lists/grenos.list.chroot` | ce qui est installé, et pourquoi |
| `config/hooks/normal/` | le visage de grenOS : thème, raccourcis, identité, écran de connexion |
| `config/includes.chroot/` | les fichiers ajoutés tels quels : accueil, outils `grenos-*` |
| `tools/wallpaper.py` | les deux fonds d'écran, calculés à la construction |

## Ce que la CI vérifie

`.github/workflows/linux.yml` construit l'image, la démarre dans QEMU, prend des
captures d'écran du bureau, et ne publie que si le bureau apparaît vraiment.
