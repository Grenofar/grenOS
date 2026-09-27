#!/bin/sh
# Le disque livré avec la machine VirtualBox : 25 Go, une seule partition ext4
# étiquetée « persistence », avec le fichier que live-boot y cherche.
#
# C'est ce qui fait que grenOS n'est pas un système jetable : dès le premier
# démarrage, tout ce que la personne écrit — ses fichiers, ses réglages, les
# applications qu'elle installe, son mot de passe — est écrit sur ce disque et
# retrouvé au démarrage suivant. L'installation sur disque reste possible, et
# reprendra la même place.
#
#   sudo linux/tools/make-disk.sh <sortie.vdi> [taille en Go] [marqueur]
#
# Le troisième argument, s'il est donné, est un chemin **dans le volume** :
# un fichier vide y est créé, dossiers parents compris. La persistance étant
# montée en union sur toute la racine, ce fichier apparaît tel quel dans le
# système qui démarre. C'est ainsi que la machine d'intégration se fait
# reconnaître pour ses essais — voir `/etc/grenos/essai-charge`, qui déclenche
# la mesure du son sous charge et n'existe sur aucune machine réelle.

set -eu

OUT="$1"
SIZE="${2:-25}"
MARQUE="${3:-}"
RAW=$(mktemp -u).raw
trap 'rm -f "$RAW"' EXIT

echo "--- disque brut de ${SIZE} Go ---"
qemu-img create -f raw "$RAW" "${SIZE}G" >/dev/null

# Une table de partitions GPT et une seule partition, qui prend tout.
sgdisk --zap-all "$RAW" >/dev/null 2>&1 || true
sgdisk --new=1:2048:0 --typecode=1:8300 --change-name=1:persistence "$RAW" >/dev/null

LOOP=$(losetup --find --show --partscan "$RAW")
trap 'losetup -d "$LOOP" 2>/dev/null || true; rm -f "$RAW"' EXIT
echo "--- systeme de fichiers ---"
mkfs.ext4 -q -L persistence "${LOOP}p1"

MOUNT=$(mktemp -d)
mount "${LOOP}p1" "$MOUNT"
# La ligne que live-boot lit : « / union » veut dire que tout le système de
# fichiers est superposé, donc tout ce qui change est gardé ici.
echo "/ union" > "$MOUNT/persistence.conf"
if [ -n "$MARQUE" ]; then
    mkdir -p "$MOUNT/$(dirname "$MARQUE")"
    : > "$MOUNT/$MARQUE"
    echo "--- marqueur $MARQUE pose dans le volume ---"
fi
sync
umount "$MOUNT"
rmdir "$MOUNT"
losetup -d "$LOOP"
trap 'rm -f "$RAW"' EXIT

echo "--- conversion en VDI ---"
qemu-img convert -f raw -O vdi "$RAW" "$OUT"
qemu-img info "$OUT" | head -6
