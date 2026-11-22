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
# L'installateur se fait À LA SOURIS, sur ce qui a été demandé : SUPPRIMER
# une partition, CRÉER pour grenOS, CHANGER la taille en Gio. Grenofar disait
# « des fois c'est pas cliquable » : on double-clique donc là où, avant, le
# second clic annulait le premier. Les positions sont celles de l'aperçu à
# 1040×696, identiques au pixel près aux captures du run 45015327277 (écran
# 1280×800). Chaque page dit son nom sur le port série : un clic à côté fait
# échouer l'essai, il ne passe pas inaperçu.

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
QMP="$OUT/qmp.sock"
DISQUE="$OUT/disque.img"
WINDOWS="$OUT/windows.img"
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
cliquer() { # x, y
    python3 "$ECRAN" clic "$QMP" "$1" "$2" 1280 800 > /dev/null || faute "le clic en $1,$2 n est pas parti"
}
# `clic` de ci-screen attend 0,5 s entre deux ordres : ce ne serait plus un
# double-clic. (ci-screen lui-même ne change pas : il relance d'autres
# constructions, jusqu'à une publication.)
double_clic() { # x, y
    python3 -B - "$ECRAN" "$QMP" "$1" "$2" <<'EOF' || faute "le double-clic en $1,$2 n est pas parti"
import importlib.util, sys
fiche = importlib.util.spec_from_file_location("ecran", sys.argv[1])
ecran = importlib.util.module_from_spec(fiche)
fiche.loader.exec_module(ecran)
x, y = int(sys.argv[3]) * 0x7FFF // 1280, int(sys.argv[4]) * 0x7FFF // 800
def envoi(*evenements):
    return {"execute": "input-send-event", "arguments": {"events": list(evenements)}}
def bouton(bas):
    return envoi({"type": "btn", "data": {"button": "left", "down": bas}})
bouger = envoi({"type": "abs", "data": {"axis": "x", "value": x}},
               {"type": "abs", "data": {"axis": "y", "value": y}})
reponses = ecran.qmp(sys.argv[2], [bouger, bouton(True), bouton(False), bouton(True), bouton(False)], pause=0.07)
sys.exit(1 if any("error" in r for r in reponses) else 0)
EOF
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
    rm -f "$OUT/vars.fd" "$MON" "$QMP"
}
trap ranger EXIT

etape "la cle FAT32 et un disque de 36 Gio : deux partitions a garder, un ancien grenOS a supprimer"
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
# La ligne du noyau de la PROCHAINE image : l'image publiée garde la sienne.
NOYAU_PLUS=$(grep -e '--bootappend-live' "$RACINE/linux/auto/config" | grep -o 'persistence-storage=[a-z,]*' | head -n 1 || true)
echo "ajoute a la ligne du noyau de la cle : ${NOYAU_PLUS:-rien}"
bash "$ICI/cle-fat.sh" "$ISO" "$OUT/couche.squashfs" "$OUT/cle.img" "$NOYAU_PLUS"
truncate -s 36G "$DISQUE"
# Comme le Samsung de Grenofar avant l'effacement : trois partitions Linux,
# la dernière un grenOS d'avant. 1, 2049 et 5121 Mio, en secteurs de 512.
sfdisk -q "$DISQUE" <<'EOF'
label: gpt
first-lba: 2048
start=2048, size=4194304, type=0FC63DAF-8483-4772-8E79-3D69D8477DE4
start=4196352, size=6291456, type=0FC63DAF-8483-4772-8E79-3D69D8477DE4
start=10487808, size=62914560, type=0FC63DAF-8483-4772-8E79-3D69D8477DE4
EOF
BOUCLE=$(sudo losetup -fP --show "$DISQUE")
sudo udevadm settle || true
sudo mkfs.ext4 -q -F "${BOUCLE}p1"
sudo mkfs.ext4 -q -F "${BOUCLE}p2"
sudo mkfs.ext4 -q -F -L grenOS "${BOUCLE}p3"
sync
GARDE_1=$(sudo sha256sum "${BOUCLE}p1" | cut -d' ' -f1)
GARDE_2=$(sudo sha256sum "${BOUCLE}p2" | cut -d' ' -f1)
sudo losetup -d "$BOUCLE"

etape "un second disque, avec un faux Windows : rien ne doit y ecrire, jamais"
# 22 novembre, run 45017904727 : la CLÉ montait en écriture chaque partition
# de chaque disque au démarrage (live-boot, « persistence »). 3 Gio : trop
# petit pour être proposé, les clics de l'essai ne bougent pas.
truncate -s 3G "$WINDOWS"
sfdisk -q "$WINDOWS" <<'EOF'
label: gpt
first-lba: 2048
start=2048, size=409600, type=C12A7328-F81F-11D2-BA4B-00A0C93EC93B
start=411648, size=5246976, type=EBD0A0A2-B9E5-4433-87C0-68B6B72699C7
start=5658624, size=32768, type=E3C9E316-0B5C-4DB8-817D-F92DF00215AE
EOF
BOUCLE=$(sudo losetup -fP --show "$WINDOWS")
sudo udevadm settle || true
sudo mkfs.vfat -F 32 -n SYSTEM "${BOUCLE}p1" > /dev/null
sudo mkfs.ntfs -Q -q -L Windows "${BOUCLE}p2" > /dev/null
mkdir -p "$OUT/pose"
sudo mount "${BOUCLE}p1" "$OUT/pose"
sudo mkdir -p "$OUT/pose/EFI/Microsoft/Boot"
echo "le gestionnaire de Windows" | sudo tee "$OUT/pose/EFI/Microsoft/Boot/bootmgfw.efi" > /dev/null
# os-prober ne reconnaît Windows qu'avec SON BCD à côté de bootmgfw.efi.
echo "sa configuration" | sudo tee "$OUT/pose/EFI/Microsoft/Boot/BCD" > /dev/null
sudo umount "$OUT/pose"
sudo mount -t ntfs-3g "${BOUCLE}p2" "$OUT/pose"
sudo mkdir -p "$OUT/pose/Windows/System32" "$OUT/pose/Users/Zoe"
sudo umount "$OUT/pose"
sleep 3   # ntfs-3g finit d'écrire après le démontage
sync
WINDOWS_1=$(sudo sha256sum "${BOUCLE}p1" | cut -d' ' -f1)
WINDOWS_2=$(sudo sha256sum "${BOUCLE}p2" | cut -d' ' -f1)
sudo losetup -d "$BOUCLE"
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
    -drive if=none,id=windows,format=raw,file="$WINDOWS" -device ide-hd,drive=windows,bus=ide.1 \
    -device usb-tablet,bus=xhci.0 \
    -nic none -vga std -display none \
    -serial file:"$SERIE" \
    -monitor unix:"$MON",server,nowait \
    -qmp unix:"$QMP",server,nowait &
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

TAILLE=$(head -1 "$OUT/1-bienvenue.txt" 2>/dev/null | cut -d, -f1 || true)
[ "$TAILLE" = 1280x800 ] || faute "ecran de ${TAILLE:-taille inconnue}, les clics visent un ecran de 1280x800"

etape "les pages, a la souris"
double_clic 1060 695                     # Commencer
attendre 'installation : page clavier' 20
sleep 3
[ "$(vu 'installation : page disque')" -eq 0 ] || faute "un double-clic sur Commencer a saute une page"
voir 2-clavier
cliquer 1067 695                         # Continuer
attendre 'installation : page disque' 20
sleep 2
voir 3-disque
double_clic 1079 471                     # Supprimer (l ancien grenOS)
sleep 2
voir 3-disque-supprimee
double_clic 1054 531                     # Creer pour grenOS
sleep 2
voir 3-disque-creee
cliquer 560 578                          # la glissiere bouge
sleep 1
voir 3-disque-glissiere
cliquer 905 578                          # le champ des Gio : on y tape 25
touche end
for _ in 1 2 3; do touche backspace; done
touche shift-2
touche shift-5
sleep 2
voir 3-disque-choisi
cliquer 1067 695
attendre 'installation : page compte' 20
sleep 2
voir 4-compte
double_clic 1067 695                     # Continuer : ne doit PAS installer
attendre 'installation : page resume' 20
sleep 3
voir 5-resume
[ "$(vu 'installation : lancee sur')" -eq 0 ] || faute "un double-clic sur Continuer a lance l installation"
touche ret 4
[ "$(vu 'installation : lancee sur')" -eq 0 ] || faute "Entree au resume a lance l installation"
echo "double-clic sur Continuer, puis Entree au resume : rien n est lance"
double_clic 640 424                      # J ai compris
sleep 2
voir 5-resume-compris
cliquer 1047 695                         # Installer grenOS
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
cliquer 1019 695                         # Redemarrer maintenant
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
sfdisk -J "$DISQUE" > "$OUT/table.json"
python3 - "$OUT/table.json" <<'EOF' || faute "le disque n a pas ce que l ecran montrait (25 Gio dans la place liberee)"
import json, sys
with open(sys.argv[1]) as f:
    table = json.load(f)["partitiontable"]["partitions"]
vu = sorted((p["start"] // 2048, p["size"] // 2048) for p in table)
attendu = [(1, 2048), (2049, 3072), (5121, 512), (5633, 25600)]
print("table :", vu)
sys.exit(0 if vu == attendu else 1)
EOF
CHANGE=""
change() { # peripherique, empreinte d avant, son nom
    if [ "$(sudo sha256sum "$1" | cut -d' ' -f1)" = "$2" ]; then
        echo "intacte : $3"
    else
        echo "A CHANGE : $3"
        CHANGE="$CHANGE [$3]"
        # Qui : ext4 note où et quand il a été monté en écriture.
        sudo dumpe2fs -h "$1" 2>/dev/null | grep -E 'Last mounted on|Last mount time|Last write time|Mount count' || true
    fi
}
change "${BOUCLE}p1" "$GARDE_1" "premiere partition gardee"
change "${BOUCLE}p2" "$GARDE_2" "seconde partition gardee"
BOUCLE_WINDOWS=$(sudo losetup -fP --show "$WINDOWS")
sudo udevadm settle || true
change "${BOUCLE_WINDOWS}p1" "$WINDOWS_1" "EFI de Windows"
change "${BOUCLE_WINDOWS}p2" "$WINDOWS_2" "Windows NTFS"
sudo losetup -d "$BOUCLE_WINDOWS"
[ -z "$CHANGE" ] || faute "des partitions qui devaient rester intactes ont change :$CHANGE"
echo "supprimee, creee, 25 Gio, et pas un octet change ailleurs, Windows compris"
mkdir -p "$OUT/vu"
sudo mount -o ro,noload "${BOUCLE}p4" "$OUT/vu"
sudo journalctl -D "$OUT/vu/var/log/journal" -b -o short-monotonic --no-pager > "$OUT/journal-installe.txt" 2>/dev/null || true
LOGIN=$(sed -n 's/^login=//p' "$OUT/vu/etc/grenos/preinstallation" || true)
# WINDOWS RESTE JOIGNABLE (22 novembre) : grenOS passe premier au démarrage,
# et Windows vit sur un autre disque. Sans son entrée, et sans le menu qui
# s'affiche, il faudrait la touche du BIOS pour le retrouver.
sudo cat "$OUT/vu/boot/grub/grub.cfg" > "$OUT/grub-installe.txt" || true
IDENTITE=$(sudo cat "$OUT/vu/etc/grenos/identite" 2>/dev/null || true)
sudo umount "$OUT/vu"
sudo losetup -d "$BOUCLE"
grep -q "^menuentry 'Windows Boot Manager" "$OUT/grub-installe.txt" \
    || faute "Windows n est pas dans le menu de demarrage du disque installe"
grep -q 'set timeout=5' "$OUT/grub-installe.txt" \
    || faute "avec Windows, le menu de demarrage ne s affiche pas"
echo "Windows est dans le menu de demarrage, affiche 5 s"
[ -n "$LOGIN" ] || faute "le disque ne dit pas qui habite ici"
echo "$IDENTITE" | grep -qx "login=$LOGIN" || faute "l assistant n a pas garde le compte de l installation"
grep -qiE 'secure ?boot enabled|secureboot: Secure boot enabled' "$OUT/journal-installe.txt" \
    || faute "le disque installe n a pas demarre en Secure Boot"
CMD=$(grep -m1 -i 'command line:' "$OUT/journal-installe.txt" || true)
case "$CMD" in *boot=live*) faute "c est la cle qui a redemarre" ;; esac
case "$CMD" in *splash*) ;; *) faute "le disque demarre sans ecran de chargement" ;; esac

echo "FENETRE : supprimer, creer, 25 Gio a la souris ; installe par la fenetre, redemarre sur le disque (cle branchee), bureau ouvert, Secure Boot" \
    | tee -a "$OUT/verdict.txt"
