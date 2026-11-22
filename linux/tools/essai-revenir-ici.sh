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

MARQUE="$T/pas-de-marque"
echo "quiet splash" > "$T/ligne"
LIGNE="$T/ligne"
cas() { # titre, état (fichier), BootNext attendu (- : aucun), efivars (oui/non), [journal de Calamares]
    printf '%b' "$2" > "$T/etat"
    rm -f "$T/bootnext" "$T/calamares.log"
    [ -n "${5:-}" ] && printf '%s\n' "$5" > "$T/calamares.log"
    vars="$T/efivars"
    [ "$4" = non ] && vars="$T/absent"
    GRENOS_EFIBOOTMGR="$T/efibootmgr" GRENOS_EFIVARS="$vars" GRENOS_CALAMARES_JOURNAL="$T/calamares.log" GRENOS_MARQUE="$MARQUE" GRENOS_LIGNE="$LIGNE" \
        sh "$S" > "$T/sortie" 2>&1
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

# Après l'installateur (21 novembre au soir) : la clé démarrée par F12, et
# grub-install qui vient de poser « debian » (0004) en tête de l'ordre.
APRES='BootCurrent: 0003\nTimeout: 1 seconds\nBootOrder: 0004,0000,0003\nBoot0000* Windows Boot Manager\tHD(1,GPT)\nBoot0003* UEFI: USB DISK 2.0 PMAP, Partition 1\tPciRoot(0x0)\nBoot0004* debian\tHD(1,GPT,ssd)\n'
SANS_DEBIAN='BootCurrent: 0003\nBootOrder: 0000,0003\nBoot0000* Windows Boot Manager\nBoot0003* UEFI: USB DISK 2.0 PMAP, Partition 1\n'
REUSSIE='2026-11-21 - 21:40:00 [6]:     Notification not sent; completion: succeeded'
RATEE='2026-11-21 - 20:32:17 [6]:     Notification not sent; completion: failed'
cas "installation réussie : « Redémarrer maintenant » mène au disque installé" "$APRES"       0004 oui "$REUSSIE"
cas "installation ratée : on revient sur la clé (pour réessayer)"              "$APRES"       0003 oui "$RATEE"
cas "installation réussie sans entrée debian : rien de forcé"                  "$SANS_DEBIAN" -    oui "$REUSSIE"
# Notre installateur (22 novembre) : sa marque, sans journal de Calamares.
MARQUE="$T/installation-reussie"
echo /dev/sda > "$MARQUE"
cas "notre installateur a réussi : le redémarrage mène au disque installé"     "$APRES"       0004 oui
MARQUE="$T/pas-de-marque"

# Le 22 novembre, l'entrée s'appelle grenOS.
APRES_NOM='BootCurrent: 0003\nBootOrder: 0004,0000,0003\nBoot0000* Windows Boot Manager\tHD(1,GPT)\nBoot0003* UEFI: USB DISK 2.0 PMAP, Partition 1\tPciRoot(0x0)\nBoot0004* grenOS\tHD(1,GPT,ssd)\n'
RENOMMEE='BootCurrent: 0002\nBootOrder: 0000,0009\nBoot0000* Windows Boot Manager\tHD(1,GPT)\nBoot0009* grenOS\tHD(1,GPT,ssd)\n'
cas "installation réussie, entrée « grenOS » : mène au disque installé"        "$APRES_NOM"   0004 oui "$REUSSIE"
cas "entrée renommée pendant la session : retour à grenOS, pas à Windows"      "$RENOMMEE"    0009 oui
echo "boot=live quiet splash" > "$T/ligne"
cas "sur la clé, une entrée disparue ne mène pas au disque"                    "$RENOMMEE"    0002 oui
echo "quiet splash" > "$T/ligne"

# Le nom de l'entrée est dit dans le journal (c'est ce qu'on lira).
printf '%b' "$CLE" > "$T/etat"; rm -f "$T/bootnext"
GRENOS_EFIBOOTMGR="$T/efibootmgr" GRENOS_EFIVARS="$T/efivars" GRENOS_CALAMARES_JOURNAL="$T/aucun-journal" GRENOS_MARQUE="$T/pas-de-marque" sh "$S" > "$T/sortie" 2>&1
if grep -q 'retour a l entree 0003 (UEFI: USB DISK 2.0 PMAP, Partition 1)' "$T/sortie"; then
    echo "ok   le journal nomme l'entrée reprise"
else
    echo "FAUX le journal ne nomme pas l'entrée : $(cat "$T/sortie")"
    fautes=$((fautes + 1))
fi

echo "revenir-ici : 13 cas, $fautes faute(s)"
[ "$fautes" = 0 ]
