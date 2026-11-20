#!/bin/bash
# Fabriquer grenos-correctif.squashfs : un correctif qu'on DÉPOSE sur une clé
# grenOS déjà faite, sans regraver l'image (20 novembre).
#
# Pourquoi ça marche, lu dans live-boot (components/9990-overlay.sh) : au
# démarrage, live-boot monte TOUS les /live/*.squashfs de la clé, triés par
# nom, et les empile ; le dernier de la liste passe au-dessus. Donc
# « grenos-correctif.squashfs » (g > f) recouvre « filesystem.squashfs » :
# ses fichiers remplacent ceux de l'image, le reste ne bouge pas.
#
# Pourquoi c'est utile : une construction complète prend deux heures. Sur la
# RTX 5060 de Grenofar, chaque essai coûtait deux heures pour une phrase
# (« écran noir »). Un correctif se fabrique en une minute, s'essaie en dix
# dans la CI (cle.yml) et se copie sur sa clé à côté des autres fichiers.
#
# Limite, et elle se dit : l'INSTALLATEUR recopie filesystem.squashfs seul.
# Un système installé depuis une clé corrigée n'a pas le correctif ; il
# l'aura par la construction suivante, qui contient les mêmes fichiers.
#
#   bash linux/correctif/fabriquer.sh <dossier de travail> <sortie.squashfs>
set -euo pipefail

ICI=$(cd "$(dirname "$0")" && pwd)
SRC="$ICI/../config/includes.chroot"
ARBRE=$1
SORTIE=$2

rm -rf "$ARBRE"
mkdir -p "$ARBRE"
n=0
while read -r genre a b; do
    case "$genre" in ''|'#'*) continue ;; esac
    case "$genre" in
        fichier)
            [ -f "$SRC/$a" ] || { echo "correctif : $a introuvable dans includes.chroot"; exit 1; }
            install -D -m "$(stat -c %a "$SRC/$a")" "$SRC/$a" "$ARBRE/$a"
            ;;
        lien)
            mkdir -p "$ARBRE/$(dirname "$a")"
            ln -sfn "$b" "$ARBRE/$a"
            ;;
        seul)
            [ -f "$ICI/seul/$a" ] || { echo "correctif : seul/$a introuvable"; exit 1; }
            install -D -m 0644 "$ICI/seul/$a" "$ARBRE/$a"
            ;;
        paquet)
            # Un fichier d'un paquet Debian trixie qui manque à l'image
            # publiée : le même que la construction suivante installera.
            CACHE="${CACHE_PAQUETS:-$ARBRE.paquets}"
            mkdir -p "$CACHE"
            if ! ls "$CACHE/${a}_"*.deb >/dev/null 2>&1; then
                docker run --rm -v "$CACHE:/sortie" debian:trixie sh -c \
                    "apt-get update -qq >/dev/null && cd /sortie && apt-get download -q $a >/dev/null && chmod a+r /sortie/*.deb" < /dev/null
            fi
            deb=$(ls "$CACHE/${a}_"*.deb | head -1)
            rm -rf "$CACHE/x"
            dpkg-deb -x "$deb" "$CACHE/x"
            [ -e "$CACHE/x/$b" ] || { echo "correctif : $b absent de $(basename "$deb")"; exit 1; }
            install -D -m 0644 "$CACHE/x/$b" "$ARBRE/$b"
            echo "correctif : $b pris dans $(basename "$deb")"
            ;;
        *)
            echo "correctif : ligne inconnue « $genre $a $b »"
            exit 1
            ;;
    esac
    n=$((n + 1))
done < "$ICI/liste"

# Un script sans le droit d'exécution ne tournerait pas, et systemd le dirait
# trop tard (sur la clé de Grenofar). On le refuse ici.
for f in "$ARBRE"/usr/lib/grenos/* "$ARBRE"/usr/bin/*; do
    [ -f "$f" ] || continue
    # Les modules Python (.py) sont IMPORTÉS ou lancés par `python3 …` :
    # ils n'ont pas à être exécutables (ecran.py, pont.py sont en 0644 dans
    # l'image publiée — refusés à tort le 21 novembre, run 44838424517).
    case "$f" in *.py) continue ;; esac
    [ -x "$f" ] || { echo "correctif : $f n'est pas exécutable (git update-index --chmod=+x)"; exit 1; }
done

mkdir -p "$ARBRE/usr/share/grenos"
{
    echo "correctif $(git -C "$ICI" rev-parse --short HEAD 2>/dev/null || echo inconnu)"
    echo "fabriqué le $(date -u '+%Y-%m-%d %H:%M UTC')"
} > "$ARBRE/usr/share/grenos/correctif"

mksquashfs "$ARBRE" "$SORTIE" -all-root -comp xz -noappend -quiet
echo "correctif : $n entrées, $(du -h "$SORTIE" | cut -f1) — $SORTIE"
