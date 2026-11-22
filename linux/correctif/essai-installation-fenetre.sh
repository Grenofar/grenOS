#!/bin/bash
# « INSTALLER grenOS » PAR SA VRAIE FENÊTRE, comme sur un PC (22 novembre).
#
#   bash linux/correctif/essai-installation-fenetre.sh <iso> <couche.squashfs> <sortie>
#
# essai-installation-grenos.sh lance le moteur seul. Il manquait ce que la
# personne fait : la clé démarre, l'assistant, le bureau, « Installer
# grenOS », les sept pages (WebKitGTK, sudo, la vraie NVRAM), « Redémarrer
# maintenant ». La clé reste branchée et PRIORITAIRE, comme chez Grenofar :
# seul le redémarrage posé par grenOS peut mener au disque.
#
# La couche porte ce que la prochaine image portera (grenos-ui, moteur,
# lanceur) : on voit tout cela AVANT de publier quoi que ce soit.
#
# Tout se fait au clavier : chaque page dit son nom sur le port série, on
# n'appuie jamais à l'aveugle.

set -euo pipefail
ISO=$1
COUCHE=$2
OUT=$3
ICI=$(cd "$(dirname "$0")" && pwd)
RACINE="$ICI/../.."
ECRAN="$RACINE/scripts/ci-screen.py"
mkdir -p "$OUT"
SERIE="$OUT/serie.log"
MON="$OUT/mon.sock"
DISQUE="$OUT/disque.img"
QEMU=""
: > "$SERIE"

etape() { echo; echo "=== $* ==="; }
vu() { grep -ac -- "$1" "$SERIE" || true; }
voir() { # nom
    python3 "$ECRAN" grab "$MON" "$OUT/vue.ppm" > /dev/null 2>&1 || return 0
    python3 "$ECRAN" judge "$OUT/vue.ppm" "$OUT/$1.png" > "$OUT/$1.txt" 2>&1 || true
    rm -f "$OUT/vue.ppm"
}
faute() {
    voir panne
    echo "::error::$*"
    echo "FENETRE : ECHEC — $*" >> "$OUT/verdict.txt"
    echo "--- les dernieres lignes de la machine"
    tr -d '\r' < "$SERIE" | tail -n 40
    exit 1
}
touche() { # touche, [secondes apres]
    python3 "$ECRAN" send "$MON" "sendkey $1" > /dev/null 2>&1 || true
    sleep "${2:-1}"
}
attendre() { # motif, secondes, [combien de fois]
    local fin=$(( SECONDS + $2 ))
    while [ "$(vu "$1")" -lt "${3:-1}" ]; do
        kill -0 "$QEMU" 2>/dev/null || faute "la machine s est eteinte en attendant « $1 »"
        [ "$SECONDS" -lt "$fin" ] || faute "pas vu en $2 s : « $1 »"
        sleep 2
    done
}
finir_assistant() { # combien de « nom choisi » attendus
    local i
    for i in $(seq 1 60); do
        [ "$(vu 'nom choisi, la machine est a')" -lt "$1" ] || return 0
        touche ret 4
    done
    faute "l assistant ne va pas au bout"
}
ecrans() {
    grep -ao 'premier : ecran bienvenue ([0-9]*/[0-9]*)' "$SERIE" | tail -n 1 | sed 's#.*/##; s#)##' || true
}
ranger() {
    if [ -n "$QEMU" ]; then kill "$QEMU" 2>/dev/null || true; fi
    # Le rapport que la clé écrit toute seule, pour comprendre un échec.
    MTOOLS_SKIP_CHECK=1 mcopy -s -n -i "$OUT/cle.img@@1048576" ::/grenos-rapports "$OUT/" > /dev/null 2>&1 || true
    rm -f "$OUT/vars.fd" "$MON"
}
trap ranger EXIT

etape "la cle FAT32 et un disque vide de 32 Gio"
# LA CLÉ PRIORITAIRE, SANS `bootindex` (22 novembre, run 45015327277). Avec
# lui, OVMF réécrit l'ordre à chaque démarrage et SUPPRIME toute entrée HD()
# (QemuBootOrderLib : « drop HD() », puis PruneBootVariables) : l'entrée du
# disque disparaissait avant que BootNext la lise, ce qu'aucun PC ne fait.
# À la place, un service d'essai remet la clé EN TÊTE de l'ordre juste avant
# que grenOS pose son BootNext : sans lui, le PC repartirait sur la clé.
PLUS="$OUT/couche"
rm -rf "$PLUS"
unsquashfs -q -n -d "$PLUS" "$COUCHE" > /dev/null
S="$PLUS/etc/systemd/system"
mkdir -p "$PLUS/usr/lib/grenos-essai" "$S/reboot.target.wants"
cat > "$PLUS/usr/lib/grenos-essai/cle-en-tete" <<'EOF'
#!/bin/sh
PATH=/usr/sbin:/usr/bin:/sbin:/bin
etat=$(efibootmgr) || exit 0
cle=$(printf '%s
' "$etat" | sed -n 's/^BootCurrent: *//p')
ordre=$cle
for n in $(printf '%s
' "$etat" | sed -n 's/^BootOrder: *//p' | tr ',' ' '); do
    [ "$n" = "$cle" ] || ordre="$ordre,$n"
done
efibootmgr -q -o "$ordre" && echo "grenos: essai : la cle en tete de l ordre ($ordre)" > /dev/ttyS0
EOF
chmod 0755 "$PLUS/usr/lib/grenos-essai/cle-en-tete"
cat > "$S/grenos-essai-priorite.service" <<'EOF'
[Unit]
Description=essai : la cle en tete de l ordre de demarrage
DefaultDependencies=no
ConditionKernelCommandLine=boot=live
Before=grenos-revenir-ici.service systemd-reboot.service

[Service]
Type=oneshot
ExecStart=/usr/lib/grenos-essai/cle-en-tete
TimeoutStartSec=20
EOF
ln -sf ../grenos-essai-priorite.service "$S/reboot.target.wants/grenos-essai-priorite.service"
mksquashfs "$PLUS" "$OUT/couche.squashfs" -all-root -comp xz -noappend -quiet
rm -rf "$PLUS"
bash "$ICI/cle-fat.sh" "$ISO" "$OUT/couche.squashfs" "$OUT/cle.img"
truncate -s 32G "$DISQUE"
cp /usr/share/OVMF/OVMF_VARS_4M.ms.fd "$OUT/vars.fd"
ACCEL=()
if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then ACCEL=(-enable-kvm -cpu host); fi
qemu-system-x86_64 "${ACCEL[@]}" -machine q35,smm=on -global driver=cfi.pflash01,property=secure,value=on \
    -m 4096 -smp 4 \
    -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.secboot.fd \
    -drive if=pflash,format=raw,file="$OUT/vars.fd" \
    -device qemu-xhci,id=xhci \
    -drive if=none,id=cle,format=raw,file="$OUT/cle.img" \
    -device usb-storage,bus=xhci.0,drive=cle,removable=on \
    -drive if=none,id=ssd,format=raw,file="$DISQUE" -device ide-hd,drive=ssd,bus=ide.0 \
    -device usb-tablet,bus=xhci.0 \
    -nic none -vga std -display none \
    -serial file:"$SERIE" \
    -monitor unix:"$MON",server,nowait &
QEMU=$!
sleep 3
kill -0 "$QEMU" 2>/dev/null || faute "QEMU n a pas demarre"

etape "la cle : l assistant"
attendre 'premier : ecran bienvenue' 300
SUR_LA_CLE=$(ecrans)
for _ in $(seq 1 30); do
    [ "$(vu 'premier ecran affiche')" -eq 0 ] || break
    touche ret 4
done
attendre 'premier ecran affiche' 10
for t in c i ret; do touche "$t"; done
finir_assistant 1
attendre 'session de bureau ouverte' 180
sleep 25

etape "le menu : Installer grenOS"
# « install » sur un clavier AZERTY : sendkey parle en positions QWERTY.
for _ in 1 2 3; do
    touche esc 2
    touche ctrl-esc 4
    for t in i n s t q l l ret; do touche "$t"; done
    for _ in $(seq 1 20); do
        [ "$(vu 'installation : ecran ouvert, ')" -eq 0 ] || break 2
        sleep 2
    done
done
attendre 'installation : ecran ouvert, ' 10
[ "$(vu 'installation : ecran ouvert, 1 disque')" -gt 0 ] \
    || faute "un seul disque devait etre propose (jamais la cle)"
sleep 4
voir 1-bienvenue

etape "les pages"
touche ret
attendre 'installation : page clavier' 20
sleep 2
voir 2-clavier
touche ret
attendre 'installation : page disque' 20
sleep 2
voir 3-disque
# La carte du disque, « Actualiser », « Utiliser tout le disque ».
for t in tab tab tab ret; do touche "$t"; done
sleep 2
voir 3-disque-choisi
touche ret
attendre 'installation : page compte' 20
sleep 2
for t in c i; do touche "$t"; done
sleep 1
voir 4-compte
touche ret
attendre 'installation : page resume' 20
sleep 2
voir 5-resume
touche ret 4
[ "$(vu 'installation : lancee sur')" -eq 0 ] || faute "Entree au resume a lance l installation"
echo "Entree au resume : rien n est lance"
for t in tab tab ret; do touche "$t"; done
attendre 'installation : lancee sur /dev/sda' 30

etape "l installation"
sleep 15
voir 6-installation
# Fermer en pleine copie : la fenêtre doit rester, donc dire la fin plus bas.
touche alt-f4 3
voir 6-apres-alt-f4
debut=$SECONDS
n=0
while [ "$(vu 'installation : terminee')" -eq 0 ]; do
    [ "$(vu 'installation : echec')" -eq 0 ] \
        || faute "l installation a echoue : $(grep -a 'installation : echec' "$SERIE" | tail -n 1)"
    kill -0 "$QEMU" 2>/dev/null || faute "la machine s est eteinte pendant l installation"
    [ $(( SECONDS - debut )) -lt 1800 ] || faute "l installation n a pas fini en 30 minutes"
    sleep 5
    n=$(( n + 1 ))
    if [ $(( n % 18 )) -eq 0 ]; then voir "6-installation-$(( n * 5 ))s"; fi
done
echo "installe en $(( SECONDS - debut )) s"
attendre 'installation : page termine' 20
echo "Alt+F4 pendant la copie : la fenetre est restee jusqu au bout"
sleep 2
voir 7-termine

etape "Redemarrer maintenant, la cle restee branchee et prioritaire"
AVANT=$(vu 'premier : ecran bienvenue')
SESSIONS=$(vu 'session de bureau ouverte')
touche tab
touche ret
attendre 'grenos: essai : la cle en tete de l ordre' 120
# La ligne du micrologiciel lui-même : il démarre l'entrée du disque.
attendre 'BdsDxe: starting Boot[0-9A-F]* "debian"' 120
attendre 'premier : ecran bienvenue' 420 $(( AVANT + 1 ))
sleep 4
voir 8-assistant-du-disque
SUR_LE_DISQUE=$(ecrans)
echo "assistant : $SUR_LA_CLE ecrans sur la cle, $SUR_LE_DISQUE apres le redemarrage"
[ "$SUR_LE_DISQUE" -eq $(( SUR_LA_CLE - 3 )) ] \
    || faute "ce n est pas le disque installe, ou l assistant redemande langue, clavier et compte"
finir_assistant 2
attendre 'premier : compte, clavier et fuseau repris de l installation' 10
attendre 'session de bureau ouverte' 240 $(( SESSIONS + 1 ))
sleep 25
voir 9-bureau-installe
python3 "$ECRAN" send "$MON" "system_powerdown" > /dev/null 2>&1 || true
for _ in $(seq 1 50); do kill -0 "$QEMU" 2>/dev/null || break; sleep 3; done
kill "$QEMU" 2>/dev/null || true
wait "$QEMU" 2>/dev/null || true
QEMU=""

etape "ce que le disque a garde"
BOUCLE=$(sudo losetup -fP --show "$DISQUE")
sudo udevadm settle || true
mkdir -p "$OUT/vu"
sudo mount -o ro,noload "${BOUCLE}p2" "$OUT/vu"
sudo journalctl -D "$OUT/vu/var/log/journal" -b -o short-monotonic --no-pager > "$OUT/journal-installe.txt" 2>/dev/null || true
LOGIN=$(sed -n 's/^login=//p' "$OUT/vu/etc/grenos/preinstallation" || true)
IDENTITE=$(sudo cat "$OUT/vu/etc/grenos/identite" 2>/dev/null || true)
sudo umount "$OUT/vu"
sudo losetup -d "$BOUCLE"
[ -n "$LOGIN" ] || faute "le disque ne dit pas qui habite ici"
echo "$IDENTITE" | grep -qx "login=$LOGIN" || faute "l assistant n a pas garde le compte de l installation"
grep -qiE 'secure ?boot enabled|secureboot: Secure boot enabled' "$OUT/journal-installe.txt" \
    || faute "le disque installe n a pas demarre en Secure Boot"
CMD=$(grep -m1 -i 'command line:' "$OUT/journal-installe.txt" || true)
case "$CMD" in *boot=live*) faute "c est la cle qui a redemarre" ;; esac
case "$CMD" in *splash*) ;; *) faute "le disque demarre sans ecran de chargement" ;; esac

echo "FENETRE : installe par la fenetre, redemarre sur le disque (cle branchee), bureau ouvert, Secure Boot" \
    | tee -a "$OUT/verdict.txt"
