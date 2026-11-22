#!/bin/bash
# LE DISQUE INSTALLÉ DÉMARRE COMME CHEZ GRENOFAR (22 novembre, 19 h).
#
#   sudo bash linux/correctif/essai-demarrage-installe.sh <iso> <couche.squashfs | -> <sortie>
#
# Après l'installation de la 1719 sur son Samsung : « l'OOBE apparaît 3 s,
# pas le temps de le compléter, puis un chargement de grenOS qui ne se
# termine pas ». Aucun essai ne le voyait : dans QEMU la carte a toujours
# son pilote (bochs). Sur son PC, en mode simple, il n'y a QUE l'image du
# firmware — Plymouth attend alors DeviceTimeout (8 s chez Debian) avant
# d'ouvrir son moteur fbdev, et ce moteur RAPPELLE le terminal 1 (« on wrong
# vt, changing vts », plugins/renderers/frame-buffer/plugin.c). Un SSD a
# déjà affiché le lobby avant ces 8 s ; la clé, plus lente, jamais.
#
# La machine d'ici :
#   - installée par le moteur (celui de l'image avec « - », sinon celui du
#     dépôt), avec le choix « firmware » de la clé et un Windows à côté ;
#   - démarrée seule, Secure Boot, disque nommé « Samsung SSD 860 EVO »,
#     sans pilote d'écran (modprobe.blacklist=bochs) ;
#   - une sonde dit chaque seconde quel terminal est À L'ÉCRAN : une capture
#     peut tromper, pas /sys/class/tty/tty0/active.
#
# Rien ne touche aux disques de la machine qui lance l'essai : des fichiers
# montés en boucle.

set -euo pipefail
ISO=$1
COUCHE=$2
OUT=$3
ICI=$(cd "$(dirname "$0")" && pwd)
RACINE="$ICI/../.."
ECRAN="$RACINE/scripts/ci-screen.py"
mkdir -p "$OUT"
MON="$OUT/mon.sock"
DISQUE="$OUT/disque.img"
WINDOWS="$OUT/windows.img"
VU="$OUT/vu"
QEMU=""
SERIE=""
RATES=""
BOUCLES=()

etape() { echo; echo "=== $* ==="; }
dire() { echo "$*" | tee -a "$OUT/verdict.txt"; }
rate() { echo "::error::$*"; dire "ECHEC : $*"; RATES="$RATES [$*]"; }
fin() {
    if [ -n "$RATES" ]; then dire "DEMARRAGE : ECHEC$RATES"; exit 1; fi
    dire "DEMARRAGE : le lobby reste, se finit, et le disque rouvre le bureau au demarrage suivant"
}
ranger() {
    if [ -n "$QEMU" ]; then kill "$QEMU" 2>/dev/null || true; fi
    umount -R "$VU" 2>/dev/null || true
    for b in "${BOUCLES[@]}"; do losetup -d "$b" 2>/dev/null || true; done
    for f in "$OUT"/*.ppm; do
        [ -f "$f" ] || continue
        python3 "$ECRAN" judge "$f" "${f%.ppm}.png" > "${f%.ppm}.txt" 2>&1 || true
        rm -f "$f"
    done
    cp /var/log/grenos-installation.log "$OUT/moteur.log" 2>/dev/null || true
    rm -f "$MON" "$OUT/vars.fd"
    chmod -R a+rX "$OUT" 2>/dev/null || true
}
trap ranger EXIT

boucler() { # image → BOUCLE
    BOUCLE=$(losetup -fP --show "$1")
    BOUCLES+=("$BOUCLE")
    udevadm settle || true
}
combien() { grep -ac -- "$1" "$SERIE" || true; }
attendre() { # motif, tours d'une seconde, [combien de fois]
    local i
    for i in $(seq 1 "$2"); do
        [ "$(combien "$1")" -lt "${3:-1}" ] || return 0
        kill -0 "$QEMU" 2>/dev/null || return 1
        sleep 1
    done
    return 1
}
touche() { python3 "$ECRAN" send "$MON" "sendkey $1" > /dev/null 2>&1 || true; sleep "${2:-1}"; }
photo() { python3 "$ECRAN" grab "$MON" "$OUT/$1.ppm" > /dev/null 2>&1 || true; }
terminal() { grep -ao 'sonde [0-9.]* vt=tty[0-9]*' "$SERIE" | tail -n 1 | sed 's/.*vt=//' || true; }

# ================================================================ installer
etape "installer, comme la fenetre le fait"
xorriso -osirrox on -indev "$ISO" -extract /live/filesystem.squashfs "$OUT/fs.sq" > /dev/null 2>&1
IMAGES="$OUT/fs.sq"
if [ "$COUCHE" = - ]; then
    rm -rf "$OUT/moteur"
    unsquashfs -q -n -d "$OUT/moteur" "$OUT/fs.sq" usr/lib/grenos/grenos-installation > /dev/null
    MOTEUR="$OUT/moteur/usr/lib/grenos/grenos-installation"
    dire "moteur et image : ceux de la version publiee, tels quels"
else
    IMAGES="$IMAGES:$COUCHE"
    MOTEUR="$RACINE/linux/config/includes.chroot/usr/lib/grenos/grenos-installation"
    dire "moteur du depot, couche du depot par-dessus l image publiee"
fi
mkdir -p "$OUT/cle-factice/grenos" "$VU"
echo firmware > "$OUT/cle-factice/grenos/affichage"

rm -f "$DISQUE" "$WINDOWS"
truncate -s 20G "$DISQUE"
truncate -s 2G "$WINDOWS"
sfdisk -q "$WINDOWS" <<'EOF'
label: gpt
first-lba: 2048
start=2048, size=409600, type=C12A7328-F81F-11D2-BA4B-00A0C93EC93B
start=411648, size=32768, type=E3C9E316-0B5C-4DB8-817D-F92DF00215AE
EOF
boucler "$WINDOWS"; W=$BOUCLE
mkfs.vfat -F 32 -n SYSTEM "${W}p1" > /dev/null
mount "${W}p1" "$VU"
mkdir -p "$VU/EFI/Microsoft/Boot"
echo "le gestionnaire" > "$VU/EFI/Microsoft/Boot/bootmgfw.efi"
echo "sa configuration" > "$VU/EFI/Microsoft/Boot/BCD"
umount "$VU"
udevadm settle || true

boucler "$DISQUE"; D=$BOUCLE
lancer_moteur() {
    env GRENOS_IMAGES="$IMAGES" GRENOS_SANS_EFI=1 GRENOS_SANS_NVRAM=1 \
        GRENOS_CLE="$OUT/cle-factice" python3 -B "$MOTEUR" "$1"
}
EMPREINTE=$(lancer_moteur lister | python3 -c '
import json, sys
print(next(d["empreinte"] for d in json.load(sys.stdin)["disques"] if d["dev"] == sys.argv[1]))' "$D")
python3 -c '
import json, sys
print(json.dumps({"disque": sys.argv[1], "empreinte": sys.argv[2], "effacer": True, "supprimer": [],
  "zone_mio": None, "taille_mio": 0,
  "compte": {"login": "zoe", "nom": "Zoé", "mot_de_passe": None},
  "clavier": {"disposition": "fr", "variante": ""}, "fuseau": "Europe/Paris"}))' "$D" "$EMPREINTE" \
    | lancer_moteur installer > "$OUT/installation.log" 2>&1 \
    || { tail -n 20 "$OUT/installation.log"; rate "l installation a echoue"; fin; }
udevadm settle || true

etape "la sonde, et une carte sans pilote"
mount "${D}p2" "$VU"
cp "$VU/boot/grub/grub.cfg" "$OUT/grub-installe.txt"
[ "$(cat "$VU/etc/grenos/affichage" 2>/dev/null)" = firmware ] || rate "le choix firmware de la cle n a pas suivi"
# La SEULE différence de matériel : pas de pilote DRM. C'est son mode simple.
sed -i -E '/^[[:space:]]*linux[[:space:]]/ s/$/ modprobe.blacklist=bochs/' "$VU/boot/grub/grub.cfg"
mkdir -p "$VU/usr/local/sbin" "$VU/etc/systemd/system/sysinit.target.wants"
cat > "$VU/usr/local/sbin/grenos-sonde" <<'EOF'
#!/bin/sh
while :; do
    read -r depuis _ < /proc/uptime
    echo "sonde $depuis vt=$(cat /sys/class/tty/tty0/active) plymouth=$(pidof plymouthd >/dev/null && echo oui || echo non) fb=$(cat /sys/class/graphics/fb0/name 2>/dev/null | tr ' ' '_')" > /dev/ttyS0
    sleep 1
done
EOF
chmod 0755 "$VU/usr/local/sbin/grenos-sonde"
cat > "$VU/etc/systemd/system/grenos-sonde.service" <<'EOF'
[Unit]
Description=essai : quel terminal est a l ecran
DefaultDependencies=no
[Service]
ExecStart=/usr/local/sbin/grenos-sonde
EOF
ln -sf ../grenos-sonde.service "$VU/etc/systemd/system/sysinit.target.wants/grenos-sonde.service"
umount "$VU"
losetup -d "$D"
losetup -d "$W"
rm -f "$OUT/fs.sq"

demarrer() { # numero
    SERIE="$OUT/serie-$1.log"
    : > "$SERIE"
    local accel=()
    if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then accel=(-enable-kvm -cpu host); fi
    [ -f "$OUT/vars.fd" ] || cp /usr/share/OVMF/OVMF_VARS_4M.ms.fd "$OUT/vars.fd"
    qemu-system-x86_64 "${accel[@]}" -machine q35,smm=on -global driver=cfi.pflash01,property=secure,value=on \
        -m 4096 -smp 4 \
        -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.secboot.fd \
        -drive if=pflash,format=raw,file="$OUT/vars.fd" \
        -drive if=none,id=ssd,format=raw,file="$DISQUE" \
        -device "ide-hd,drive=ssd,bus=ide.0,model=Samsung SSD 860 EVO 250GB,ver=RVT04B6Q" \
        -drive if=none,id=windows,format=raw,file="$WINDOWS" -device ide-hd,drive=windows,bus=ide.1 \
        -device qemu-xhci,id=xhci -device usb-tablet,bus=xhci.0 \
        -nic none -vga std -display none \
        -serial file:"$SERIE" \
        -monitor unix:"$MON",server,nowait &
    QEMU=$!
    sleep 1
    kill -0 "$QEMU" 2>/dev/null || { rate "QEMU n a pas demarre"; fin; }
}
eteindre() {
    python3 "$ECRAN" send "$MON" "system_powerdown" > /dev/null 2>&1 || true
    for _ in $(seq 1 45); do kill -0 "$QEMU" 2>/dev/null || break; sleep 2; done
    kill "$QEMU" 2>/dev/null || true
    wait "$QEMU" 2>/dev/null || true
    QEMU=""
}

# ======================================================== premier demarrage
etape "premier demarrage : ce qu on voit, seconde par seconde"
demarrer 1
n=0
lobby=non
for _ in $(seq 1 90); do
    n=$((n + 1))
    photo "1-$(printf '%02d' "$n")"
    if [ "$(combien 'premier : ecran bienvenue')" -gt 0 ]; then lobby=oui; break; fi
    sleep 1
done
if [ "$lobby" = non ]; then
    rate "le lobby ne s est jamais affiche"
    eteindre
    fin
fi
LIGNE=$(grep -an 'premier : ecran bienvenue' "$SERIE" | head -n 1 | cut -d: -f1)
for _ in $(seq 1 25); do
    n=$((n + 1))
    sleep 1
    photo "1-$(printf '%02d' "$n")"
done
APRES=$(tail -n +"$LIGNE" "$SERIE" | grep -ao 'sonde [0-9.]* vt=tty[0-9]* plymouth=[a-z]*' || true)
echo "$APRES" > "$OUT/terminaux-apres-lobby.txt"
PARTI=$(echo "$APRES" | grep -v 'vt=tty7 ' | head -n 1 || true)
if [ -n "$PARTI" ]; then
    rate "le lobby a quitte l ecran : $PARTI"
    # Ce que Grenofar peut faire SANS réinstaller, mesuré plutôt que promis.
    etape "secours : Ctrl+Alt+F7"
    touche ctrl-alt-f7 5
    photo 1-secours-f7
    if [ "$(terminal)" = tty7 ]; then
        dire "SECOURS Ctrl+Alt+F7 : le lobby revient a l ecran"
    else
        dire "SECOURS Ctrl+Alt+F7 : sans effet ($(terminal))"
    fi
else
    dire "le lobby est reste a l ecran 25 s de suite (tty7)"
fi

etape "finir le lobby"
fini=non
if [ "$(terminal)" = tty7 ]; then
    for _ in $(seq 1 45); do
        touche ret 3
        if [ "$(combien 'nom choisi, la machine est a')" -gt 0 ]; then fini=oui; break; fi
    done
fi
if [ "$fini" = oui ] && attendre 'session de bureau ouverte' 120; then
    sleep 15
    photo 1-bureau
    dire "lobby fini, bureau ouvert"
else
    photo 1-bloque
    rate "le lobby ne peut pas etre fini jusqu au bureau"
fi
eteindre

# ========================================================= second demarrage
etape "second demarrage"
demarrer 2
n=0
bureau=non
for _ in $(seq 1 80); do
    n=$((n + 1))
    photo "2-$(printf '%02d' "$n")"
    if [ "$(combien 'session de bureau ouverte')" -gt 0 ]; then bureau=oui; break; fi
    sleep 1
done
if [ "$bureau" = oui ]; then
    sleep 12
    photo 2-bureau
    if [ "$(terminal)" = tty7 ]; then
        dire "second demarrage : bureau ouvert, a l ecran"
    else
        rate "second demarrage : bureau ouvert mais pas a l ecran ($(terminal))"
    fi
else
    photo 2-bloque
    rate "second demarrage : pas de bureau"
fi
eteindre

etape "le journal du disque"
boucler "$DISQUE"; D=$BOUCLE
if mount -o ro,noload "${D}p2" "$VU"; then
    J="$VU/var/log/journal"
    journalctl -D "$J" -o short-monotonic --no-pager > "$OUT/journal-installe.txt" 2>/dev/null || true
    # Ce que « quiet » laisse passer à l'écran : le « une erreur evo » de Grenofar.
    journalctl -D "$J" -k -p err -o short-monotonic --no-pager > "$OUT/noyau-erreurs.txt" 2>/dev/null || true
    umount "$VU"
fi
losetup -d "$D"
fin
