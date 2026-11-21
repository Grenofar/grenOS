#!/bin/sh
# Essai de grenos-revenir-ici : un redémarrage demandé depuis grenOS revient
# à l'entrée UEFI qui a démarré grenOS (21 novembre), sans jamais écraser un
# prochain démarrage déjà choisi.
#   sh linux/tools/essai-revenir-ici.sh
set -eu
ICI=$(cd "$(dirname "$0")" && pwd)
S="$ICI/../config/includes.chroot/usr/lib/grenos/grenos-revenir-ici"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
fautes=0
mkdir -p "$T/efivars"

# Un faux efibootmgr : sans argument, il affiche l'état de $T/etat ; avec
# -n, il note l'entrée demandée dans $T/bootnext.
cat > "$T/efibootmgr" <<EOF
#!/bin/sh
if [ "\$#" -eq 0 ]; then cat "$T/etat"; exit 0; fi
while [ "\$#" -gt 0 ]; do
    case "\$1" in -n) shift; echo "\$1" > "$T/bootnext" ;; esac
    shift
done
EOF
chmod +x "$T/efibootmgr"

cas() { # titre, état (fichier), BootNext attendu (- : aucun), efivars (oui/non)
    printf '%b' "$2" > "$T/etat"
    rm -f "$T/bootnext"
    vars="$T/efivars"
    [ "$4" = non ] && vars="$T/absent"
    GRENOS_EFIBOOTMGR="$T/efibootmgr" GRENOS_EFIVARS="$vars" sh "$S" > "$T/sortie" 2>&1
    obtenu=$(cat "$T/bootnext" 2>/dev/null || echo -)
    if [ "$obtenu" = "$3" ]; then
        echo "ok   $1"
    else
        echo "FAUX $1 : BootNext $obtenu au lieu de $3 ($(tr '\n' ' ' < "$T/sortie"))"
        fautes=$((fautes + 1))
    fi
}

CLE='BootCurrent: 0003\nTimeout: 1 seconds\nBootOrder: 0000,0001,0003\nBoot0000* Windows Boot Manager\tHD(1,GPT)\nBoot0001* debian\tHD(1,GPT)\nBoot0003* UEFI: USB DISK 2.0 PMAP, Partition 1\tPciRoot(0x0)\n'
DISQUE='BootCurrent: 0001\nTimeout: 1 seconds\nBootOrder: 0000,0001\nBoot0000* Windows Boot Manager\tHD(1,GPT)\nBoot0001* debian\tHD(1,GPT)\n'
DEJA='BootCurrent: 0001\nBootNext: 0000\nBootOrder: 0001,0000\nBoot0000* Windows Boot Manager\nBoot0001* debian\n'
INCONNU='Timeout: 1 seconds\nBootOrder: 0000\nBoot0000* Windows Boot Manager\n'

cas "clé démarrée par F12 : le redémarrage revient à la clé"        "$CLE"     0003 oui
cas "disque installé, Windows en tête de l'ordre : retour à debian"   "$DISQUE"  0001 oui
cas "un prochain démarrage déjà choisi : on n'y touche pas"          "$DEJA"    -    oui
cas "entrée courante inconnue : rien de forcé"                       "$INCONNU" -    oui
cas "machine démarrée en BIOS (pas d'efivars) : rien"                "$CLE"     -    non

# Le nom de l'entrée est dit dans le journal (c'est ce qu'on lira).
printf '%b' "$CLE" > "$T/etat"; rm -f "$T/bootnext"
GRENOS_EFIBOOTMGR="$T/efibootmgr" GRENOS_EFIVARS="$T/efivars" sh "$S" > "$T/sortie" 2>&1
if grep -q 'retour a l entree 0003 (UEFI: USB DISK 2.0 PMAP, Partition 1)' "$T/sortie"; then
    echo "ok   le journal nomme l'entrée reprise"
else
    echo "FAUX le journal ne nomme pas l'entrée : $(cat "$T/sortie")"
    fautes=$((fautes + 1))
fi

echo "revenir-ici : $fautes faute(s)"
[ "$fautes" = 0 ]
