#!/bin/bash
# NOTRE INSTALLATEUR, de bout en bout, sur des disques virtuels (22 novembre).
#
#   sudo bash linux/correctif/essai-installation-grenos.sh <iso> <correctif.squashfs> <sortie>
#
# essai-installation.sh rejoue les étapes de Calamares ; celui-ci lance le
# VRAI moteur (usr/lib/grenos/grenos-installation de ce dépôt), avec un plan
# comme l'écran l'envoie, sur l'image publiée. Deux machines :
#
#   A. un disque vide, « Utiliser tout le disque », un compte avec mot de
#      passe, le clavier du Québec : le disque démarre SEUL (OVMF, Secure
#      Boot, clés de Microsoft) jusqu'à l'assistant ;
#   B. un disque partagé : partition EFI avec un faux Windows, une partition
#      à garder, une à supprimer. « Supprimer », puis « Créer pour grenOS »
#      (22 Gio). Pas un octet de ce qui reste ne bouge ; et un plan périmé
#      n'écrit rien.
#
# Rien ne touche aux disques de la machine qui lance l'essai : le moteur ne
# reçoit que des fichiers montés en boucle.

set -euo pipefail
ISO=$1
CORRECTIF=$2
OUT=$3
ICI=$(cd "$(dirname "$0")" && pwd)
RACINE="$ICI/../.."
MOTEUR="$RACINE/linux/config/includes.chroot/usr/lib/grenos/grenos-installation"
mkdir -p "$OUT"
etape() { echo; echo "=== $* ==="; }
faute() { echo "::error::$*"; echo "INSTALLATEUR : ECHEC — $*" >> "$OUT/verdict.txt"; exit 1; }

BOUCLES=()
MONTE="$OUT/vu"
ranger() {
    cp /var/log/grenos-installation.log "$OUT/moteur-journal.log" 2>/dev/null || true
    umount -R "$MONTE" 2>/dev/null || true
    for b in "${BOUCLES[@]}"; do losetup -d "$b" 2>/dev/null || true; done
}
trap ranger EXIT

# Le système qui lance l'essai tient le rôle de la clé : le moteur y est
# root, et tout ce qu'il vise de travers s'écrit ICI. Il doit en sortir intact.
hote() {
    local f
    for f in /etc/passwd /etc/shadow /etc/group /etc/fstab /etc/hostname /etc/hosts /etc/timezone \
             /etc/machine-id /etc/locale.gen /etc/locale.conf /etc/default/locale /etc/default/keyboard \
             /etc/default/grub /etc/apt/sources.list /boot/grub/grub.cfg \
             /etc/grenos/preinstallation /etc/grenos/affichage /etc/lightdm/lightdm.conf; do
        if [ -f "$f" ]; then echo "$f $(sha256sum < "$f")"; else echo "$f absent"; fi
    done
    echo "fuseau $(readlink /etc/localtime || true)"
    { find /etc/apt/sources.list.d /etc/sudoers.d /boot/efi -type f -exec sha256sum {} + 2>/dev/null || true; } | sort
    efibootmgr 2>/dev/null || true
}
hote_intact() { # machine
    hote > "$OUT/hote-apres.txt"
    diff -u "$OUT/hote-avant.txt" "$OUT/hote-apres.txt" || faute "$1 : le moteur a ecrit sur le systeme qui tourne"
    echo "$1 : le systeme qui tourne est intact"
}
hote > "$OUT/hote-avant.txt"

etape "les images de la cle"
xorriso -osirrox on -indev "$ISO" -extract /live/filesystem.squashfs "$OUT/fs.sq" >/dev/null 2>&1
mkdir -p "$OUT/cle-factice/grenos"
echo firmware > "$OUT/cle-factice/grenos/affichage"

# Par sudo et sans terminal, le plan sur l'entrée standard : le chemin exact
# de la fenêtre (grenos-ui, systeme::privilegie).
moteur() { # action
    setsid -w sudo -n env GRENOS_IMAGES="$OUT/fs.sq:$CORRECTIF" GRENOS_SANS_EFI=1 GRENOS_SANS_NVRAM=1 \
        GRENOS_CLE="$OUT/cle-factice" python3 -B "$MOTEUR" "$1"
}

champ() { # disque, expression python sur « d » : lu dans ce que voit l'écran
    moteur lister | python3 -c '
import json, sys
d = [x for x in json.load(sys.stdin)["disques"] if x["dev"] == sys.argv[1]]
if not d:
    sys.exit("le moteur ne propose pas " + sys.argv[1])
d = d[0]
print(eval(sys.argv[2]))' "$1" "$2"
}

preparer() { # fichier, taille → BOUCLE
    rm -f "$1"
    truncate -s "$2" "$1"
    BOUCLE=$(losetup -fP --show "$1")
    BOUCLES+=("$BOUCLE")
    udevadm settle || true
}

# ======================================================== A : le disque vide
etape "A : disque vide de 15 Gio, tout le disque, compte avec mot de passe"
# 15 Gio : le plus petit disque accepté (comme Calamares). S'il s'installe et
# démarre là, la limite n'est pas qu'un chiffre.
preparer "$OUT/disque-a.img" 15G
A=$BOUCLE
[ "$(champ "$A" 'd["etat"]')" = vide ] || faute "A : un disque vide n est pas vu vide"
# Un secret tiré au sort pour cet essai seulement : il ne doit se retrouver
# ni dans la sortie du moteur, ni dans un journal, ni en clair sur le disque.
SECRET="essai$(od -An -N8 -tx1 /dev/urandom | tr -d ' \n')"
EMPREINTE=$(champ "$A" 'd["empreinte"]')
SECRET="$SECRET" python3 -c '
import json, os, sys
print(json.dumps({"disque": sys.argv[1], "empreinte": sys.argv[2], "effacer": True, "supprimer": [],
  "zone_mio": None, "taille_mio": 0,
  "compte": {"login": "camille", "nom": "Zoé Essai", "mot_de_passe": os.environ["SECRET"]},
  "clavier": {"disposition": "ca", "variante": ""}, "fuseau": "America/Montreal"}))' "$A" "$EMPREINTE" \
    | moteur installer > "$OUT/moteur-a.log" 2>&1 \
    || { tail -n 20 "$OUT/moteur-a.log"; tail -n 60 /var/log/grenos-installation.log || true; faute "A : le moteur s est arrete"; }
grep -E '^AVANCE (1|3|5|8|76|80|86|92|97|100) |^ERREUR|^FINI' "$OUT/moteur-a.log" || true
grep -q '^FINI$' "$OUT/moteur-a.log" || faute "A : le moteur n a pas dit FINI"
grep -qE '^AVANCE [0-9]+ Copie de grenOS — [0-9]+ %' "$OUT/moteur-a.log" || faute "A : la copie n a donne aucun pourcentage"
if grep -qF "$SECRET" "$OUT/moteur-a.log" /var/log/grenos-installation.log; then
    faute "A : le mot de passe est dans un journal"
fi
[ -e /run/grenos/installation-reussie ] || faute "A : la marque du succes manque (le redemarrage reviendrait sur la cle)"
if findmnt -rn -o TARGET | grep -q calamares-root-grenos; then
    faute "A : des montages de l installation restent"
fi
[ -e /dev/pts/ptmx ] || faute "A : demonter le chroot a emporte le /dev/pts de la machine"
hote_intact A

etape "A : ce que porte le disque"
udevadm settle || true
mkdir -p "$MONTE"
mount -o ro "${A}p2" "$MONTE"
mount -o ro "${A}p1" "$MONTE/boot/efi"
V="$MONTE"
[ "$(awk -F: '$3==1000{print $1}' "$V/etc/passwd")" = camille ] || faute "A : le compte n est pas camille (UID 1000)"
awk -F: '$1=="camille" && length($2)>20 && $2!~/^!/{ok=1} END{exit !ok}' "$V/etc/shadow" \
    || faute "A : le mot de passe n est pas pose"
awk -F: '$1=="root" && $2==""{exit 1}' "$V/etc/shadow" || faute "A : root s ouvre sans mot de passe"
if grep -rqF "$SECRET" "$V/etc" "$V/var/log" 2>/dev/null; then faute "A : le mot de passe est en clair sur le disque"; fi
if grep -rq '^autologin-user=' "$V/etc/lightdm/lightdm.conf" "$V/etc/lightdm/lightdm.conf.d/60-grenos.conf"; then
    faute "A : avec un mot de passe, la session s ouvrirait toute seule"
fi
if grep -q NOPASSWD "$V/etc/sudoers.d/grenos-identite"; then faute "A : administration sans mot de passe"; fi
grep -qx 'greeter-hide-users=false' "$V/etc/lightdm/lightdm.conf" || faute "A : avec un mot de passe, le compte serait cache (nom a taper)"
if grep -q '^gamemode:' "$V/etc/group"; then
    grep -qE '^gamemode:.*[:,]camille(,|$)' "$V/etc/group" || faute "A : le compte n est pas dans gamemode"
fi
grep -q '^camille:x:1000:1000:Zoé Essai' "$V/etc/passwd" || faute "A : le nom accentue n est pas garde"
grep -q '^login=camille$' "$V/etc/grenos/preinstallation" || faute "A : l assistant ne saurait pas qui habite ici"
[ ! -e "$V/etc/grenos/identite" ] || faute "A : l assistant ne se lancerait pas"
grep -q '^XKBLAYOUT="ca"$' "$V/etc/default/keyboard" || faute "A : le clavier choisi n est pas pose"
[ "$(readlink "$V/etc/localtime")" = /usr/share/zoneinfo/America/Montreal ] || faute "A : le fuseau choisi n est pas pose"
[ "$(sed -n 3p "$V/etc/adjtime")" = LOCAL ] || faute "A : horloge en UTC"
[ "$(cat "$V/etc/grenos/affichage")" = firmware ] || faute "A : le choix d affichage de la cle n a pas suivi"
grep -q "$(blkid -s UUID -o value "${A}p2") / ext4" "$V/etc/fstab" || faute "A : fstab ne monte pas la racine"
grep -q '^deb http://deb.debian.org/debian' "$V/etc/apt/sources.list" || faute "A : le disque n a pas de sources apt"
if grep -q '^Package: calamares$' "$V/var/lib/dpkg/status" && \
   awk '/^Package: calamares$/{p=1} p&&/^Status:/{print;exit}' "$V/var/lib/dpkg/status" | grep -q 'ok installed'; then
    faute "A : l installateur reste sur le disque installe"
fi
cmp -s "$V/boot/efi/EFI/BOOT/BOOTX64.EFI" "$V/usr/lib/shim/shimx64.efi.signed" \
    || faute "A : le chemin de secours n est pas shim"
ID_A=$(cat "$V/etc/machine-id")
echo "$ID_A" | grep -qE '^[0-9a-f]{32}$' || faute "A : pas d identifiant de machine"
[ "$ID_A" != "$(cat /etc/machine-id)" ] || faute "A : l identifiant de la cle est passe sur le disque"
[ "$(readlink "$V/var/lib/dbus/machine-id")" = /etc/machine-id ] || faute "A : dbus garde un autre identifiant"
awk -F: '$1=="root" && $2!~/^[!*]/{exit 1}' "$V/etc/shadow" || faute "A : le compte root n est pas ferme"
grep -qE '^[[:space:]]*linux[[:space:]].* splash' "$V/boot/grub/grub.cfg" || faute "A : le disque demarrerait en texte (sans splash)"
grep -q "^menuentry 'grenOS" "$V/boot/grub/grub.cfg" || faute "A : le menu de demarrage ne dit pas grenOS"
echo "compte, mot de passe, clavier, fuseau, horloge, affichage, EFI, GRUB : en place"
umount -R "$MONTE"
# Le QEMU d'après n'a pas de RTX 50 : le choix « firmware » y est sans effet.
losetup -d "$A"

etape "A : le disque demarre seul, Secure Boot"
CODE=/usr/share/OVMF/OVMF_CODE_4M.secboot.fd
cp /usr/share/OVMF/OVMF_VARS_4M.ms.fd "$OUT/vars.fd"
ACCEL=()
if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then ACCEL=(-enable-kvm -cpu host); fi
qemu-system-x86_64 "${ACCEL[@]}" -machine q35,smm=on -global driver=cfi.pflash01,property=secure,value=on \
    -m 4096 -smp 4 \
    -drive if=pflash,format=raw,readonly=on,file="$CODE" -drive if=pflash,format=raw,file="$OUT/vars.fd" \
    -drive if=none,id=ssd,format=raw,file="$OUT/disque-a.img" -device ide-hd,drive=ssd,bus=ide.0 \
    -device qemu-xhci,id=xhci -device usb-tablet,bus=xhci.0 \
    -vga std -display none \
    -serial file:"$OUT/serie.log" \
    -monitor unix:"$OUT/mon.sock",server,nowait &
QEMU=$!
sleep 3
kill -0 "$QEMU" 2>/dev/null || faute "A : QEMU n a pas demarre"
t=0
for cap in 50 100 140; do
    sleep $((cap - t)); t=$cap
    kill -0 "$QEMU" 2>/dev/null || break
    python3 "$RACINE/scripts/ci-screen.py" grab "$OUT/mon.sock" "$OUT/installe-${cap}s.ppm" || true
    python3 "$RACINE/scripts/ci-screen.py" judge "$OUT/installe-${cap}s.ppm" "$OUT/installe-${cap}s.png" > "$OUT/installe-${cap}s.txt" 2>&1 || true
    rm -f "$OUT/installe-${cap}s.ppm"
done
python3 "$RACINE/scripts/ci-screen.py" send "$OUT/mon.sock" "system_powerdown" || true
for _ in $(seq 1 40); do kill -0 "$QEMU" 2>/dev/null || break; sleep 3; done
kill "$QEMU" 2>/dev/null || true
sleep 2

A=$(losetup -fP --show "$OUT/disque-a.img")
BOUCLES+=("$A")
udevadm settle || true
mount -o ro,noload "${A}p2" "$MONTE"
journalctl -D "$MONTE/var/log/journal" -b -o short-monotonic --no-pager > "$OUT/journal-installe.txt" 2>/dev/null || true
grep -E 'Secure boot|secureboot|grenos-ui: premier|Started lightdm' "$OUT/journal-installe.txt" | head -20 || true
PREUVE=$( { cat "$OUT/serie.log" "$OUT/journal-installe.txt" 2>/dev/null || true; } \
    | grep -acE 'premier : ecran bienvenue|premier ecran : le programme demarre|Starting grenos-premier' || true)
[ "${PREUVE:-0}" -gt 0 ] || faute "A : le disque installe n a pas lance l assistant"
grep -qiE 'secure ?boot enabled|secureboot: Secure boot enabled' "$OUT/journal-installe.txt" \
    || faute "A : le journal ne prouve pas le demarrage en Secure Boot"
# Le rail de l'assistant, quand l'image connait deja l'installateur : il ne
# redemande ni langue, ni clavier, ni compte (12 ecrans au lieu de 15).
grep -ao 'premier : ecran bienvenue ([0-9]*/[0-9]*)' "$OUT/journal-installe.txt" | head -1 || true
umount "$MONTE"
losetup -d "$A"
rm -f "$OUT/disque-a.img"
echo "INSTALLATEUR A : le disque demarre seul en Secure Boot jusqu a l assistant" | tee -a "$OUT/verdict.txt"

# ===================================================== B : le disque partage
etape "B : un faux Windows, supprimer, creer pour grenOS"
preparer "$OUT/disque-b.img" 40G
B=$BOUCLE
sfdisk -q "$B" <<'EOF'
label: gpt
start=1MiB, size=300MiB, type=C12A7328-F81F-11D2-BA4B-00A0C93EC93B, name="EFI"
start=301MiB, size=2048MiB, type=0FC63DAF-8483-4772-8E79-3D69D8477DE4, name="a garder"
start=2349MiB, size=10240MiB, type=0FC63DAF-8483-4772-8E79-3D69D8477DE4, name="a supprimer"
EOF
partx -u "$B" 2>/dev/null || true
udevadm settle || true
mkfs.vfat -F 32 -n SYSTEM "${B}p1" >/dev/null
mkfs.ext4 -q -F -L fichiers "${B}p2"
mkfs.ext4 -q -F -L ancien "${B}p3"
mount "${B}p1" "$MONTE"
mkdir -p "$MONTE/EFI/Microsoft/Boot" "$MONTE/EFI/Boot"
echo "le gestionnaire de Windows" > "$MONTE/EFI/Microsoft/Boot/bootmgfw.efi"
echo "le secours de Windows" > "$MONTE/EFI/Boot/bootx64.efi"
umount "$MONTE"
mount "${B}p2" "$MONTE"
head -c 5000000 /dev/urandom > "$MONTE/souvenirs.bin"
umount "$MONTE"
sync
udevadm settle || true
TEMOIN=$(sha256sum < "${B}p2")
TABLE_AVANT=$(sfdisk -d "$B")
EFI_UUID=$(blkid -s UUID -o value "${B}p1")

plan_b() { # empreinte
    python3 -c '
import json, sys
print(json.dumps({"disque": sys.argv[1], "empreinte": sys.argv[2], "effacer": False,
  "supprimer": [sys.argv[1] + "p3"], "zone_mio": 2349, "taille_mio": 22 * 1024,
  "compte": {"login": "camille", "nom": "Camille", "mot_de_passe": None},
  "clavier": {"disposition": "fr", "variante": ""}, "fuseau": "Europe/Paris"}))' "$B" "$1"
}

if plan_b perimee | moteur installer > "$OUT/moteur-b-refus.log" 2>&1; then
    faute "B : un plan perime a ete accepte"
fi
[ "$(sfdisk -d "$B")" = "$TABLE_AVANT" ] || faute "B : un refus a modifie la table"
[ "$(blkid -p -s LABEL -o value "${B}p3")" = ancien ] || faute "B : un refus a touche une partition"
[ ! -e /run/grenos/installation-reussie ] || rm -f /run/grenos/installation-reussie
echo "plan perime : refuse, rien d ecrit"

EMPREINTE=$(champ "$B" 'd["empreinte"]')
plan_b "$EMPREINTE" | moteur installer > "$OUT/moteur-b.log" 2>&1 \
    || { tail -n 20 "$OUT/moteur-b.log"; tail -n 60 /var/log/grenos-installation.log || true; faute "B : le moteur s est arrete"; }
grep -q '^FINI$' "$OUT/moteur-b.log" || faute "B : le moteur n a pas dit FINI"
udevadm settle || true

[ "$(sha256sum < "${B}p2")" = "$TEMOIN" ] || faute "B : la partition a garder a change"
[ "$(blkid -p -s UUID -o value "${B}p1")" = "$EFI_UUID" ] || faute "B : la partition EFI de Windows a ete formatee"
mount -o ro "${B}p1" "$MONTE"
[ "$(cat "$MONTE/EFI/Microsoft/Boot/bootmgfw.efi")" = "le gestionnaire de Windows" ] || faute "B : Windows a perdu son gestionnaire"
[ "$(cat "$MONTE/EFI/Boot/bootx64.efi")" = "le secours de Windows" ] || faute "B : le secours de Windows a ete remplace"
[ -f "$MONTE/EFI/debian/shimx64.efi" ] || faute "B : grenOS n est pas sur la partition EFI"
umount "$MONTE"
sfdisk -J "$B" | python3 -c '
import json, sys
p = json.load(sys.stdin)["partitiontable"]["partitions"]
vu = sorted((x["start"] // 2048, x["size"] // 2048) for x in p)
attendu = [(1, 300), (301, 2048), (2349, 22528)]
sys.exit(0 if vu == attendu else f"table : {vu} au lieu de {attendu}")' || faute "B : la table n est pas celle montree"
mount -o ro "${B}p3" "$MONTE"
[ "$(cat "$MONTE/etc/machine-id")" != "$ID_A" ] || faute "B : deux installations ont le meme identifiant"
grep -q '^autologin-user=camille$' "$MONTE/etc/lightdm/lightdm.conf" || faute "B : sans mot de passe, la session doit s ouvrir seule"
umount "$MONTE"
hote_intact B
echo "INSTALLATEUR B : Windows et la partition gardee intacts, grenOS 22 Gio dans l espace libere" | tee -a "$OUT/verdict.txt"
echo "INSTALLATEUR : notre installateur est prouve de bout en bout" | tee -a "$OUT/verdict.txt"
