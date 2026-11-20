#!/bin/sh
# Essai de grenos-nouveau-ou-pas sur des machines factices (20 novembre).
#   sh linux/tools/essai-nouveau-ou-pas.sh
set -eu
ICI=$(cd "$(dirname "$0")" && pwd)
S="$ICI/../config/includes.chroot/usr/lib/grenos/grenos-nouveau-ou-pas"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
fautes=0

carte() { # machine, nom, vendeur, classe, identifiant
    mkdir -p "$T/$1/bus/pci/devices/$2"
    printf '%s\n' "$3" > "$T/$1/bus/pci/devices/$2/vendor"
    printf '%s\n' "$4" > "$T/$1/bus/pci/devices/$2/class"
    printf '%s\n' "$5" > "$T/$1/bus/pci/devices/$2/device"
}
cas() { # machine, ligne du noyau, attendu, titre
    printf '%s\n' "$2" > "$T/ligne"
    obtenu=$(GRENOS_SYSFS="$T/$1" GRENOS_LIGNE="$T/ligne" sh "$S")
    if [ "$obtenu" = "$3" ]; then echo "ok   $4"; else echo "FAUX $4 : $obtenu au lieu de $3"; fautes=$((fautes + 1)); fi
}

carte rtx5060 0000:01:00.0 0x10de 0x030000 0x2d05
carte rtx5060 0000:01:00.1 0x10de 0x040300 0x22eb   # sa sortie son HDMI
carte rtx4070 0000:01:00.0 0x10de 0x030000 0x2786
carte rtx5090 0000:01:00.0 0x10de 0x030000 0x2b85
carte amd     0000:03:00.0 0x1002 0x030000 0x744c
carte qemu    0000:00:02.0 0x1234 0x030000 0x1111

cas rtx5060 "quiet splash" "pas charge" "RTX 5060 (Blackwell) : nouveau ecarte"
cas rtx5090 "quiet"        "pas charge" "RTX 5090 (Blackwell) : nouveau ecarte"
cas rtx5060 "quiet grenos.nouveau=1" "charge" "RTX 5060 avec grenos.nouveau=1 : nouveau charge"
cas rtx4070 "quiet"        "charge"     "RTX 4070 (Ada) : nouveau charge"
cas amd     "quiet"        "charge"     "carte AMD : nouveau charge (il ne s'y attachera pas)"
cas qemu    "quiet"        "charge"     "machine virtuelle : nouveau charge"

[ "$fautes" -eq 0 ] && echo "nouveau-ou-pas : 6 cas justes" || { echo "nouveau-ou-pas : $fautes faute(s)"; exit 1; }
