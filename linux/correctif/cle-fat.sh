#!/bin/bash
# Une clé USB comme celle de Grenofar : une partition FAT32 où l'on a COPIÉ
# les fichiers de l'ISO (c'est ainsi qu'il fait sa clé depuis Windows), plus
# le correctif dans /live. Pas de droits root : sfdisk, mkfs.fat et mtools
# travaillent sur un simple fichier.
#
#   bash linux/correctif/cle-fat.sh <iso> <correctif.squashfs | -> <sortie.img> ["paramètres du noyau en plus"]
set -euo pipefail

ISO=$1
CORR=$2
IMG=$3
PLUS=${4:-}
export MTOOLS_SKIP_CHECK=1

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

xorriso -osirrox on -indev "$ISO" -extract / "$T/iso" >/dev/null 2>&1
chmod -R u+w "$T/iso"
# Windows recopie le CONTENU des liens : on fait pareil.
cp -rL "$T/iso" "$T/plat"
rm -rf "$T/iso"

taille=$(( $(du -sm "$T/plat" | cut -f1) + 700 ))
rm -f "$IMG"
truncate -s "${taille}M" "$IMG"
printf 'label: dos\nstart=2048, type=c, bootable\n' | sfdisk -q "$IMG"
mkfs.fat -F 32 -n GRENOS --offset 2048 "$IMG" $(( taille * 1024 - 1024 )) >/dev/null
P="$IMG@@1048576"

if [ -n "$PLUS" ]; then
    # Seulement l'entrée par défaut (« Live system »), à la fin de sa ligne.
    sed -i "0,/findiso=\${iso_path}/s//& $PLUS/" "$T/plat/boot/grub/grub.cfg"
    grep -m1 'findiso' "$T/plat/boot/grub/grub.cfg" | sed 's/^[[:space:]]*/grub : /'
fi

for f in "$T/plat"/* "$T/plat"/.disk; do
    [ -e "$f" ] || continue
    mcopy -s -i "$P" "$f" ::/
done
if [ "$CORR" != "-" ]; then
    mcopy -i "$P" "$CORR" ::/live/grenos-correctif.squashfs
fi
echo "cle : $(du -h "$IMG" | cut -f1), fichiers :"
mdir -i "$P" ::/live | sed 's/^/  /'
