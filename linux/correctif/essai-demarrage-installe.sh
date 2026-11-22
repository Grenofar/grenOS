#!/bin/bash
# LE DISQUE INSTALLÉ DÉMARRE COMME CHEZ GRENOFAR (22 novembre, 19 h).
#
#   sudo [PAQUETS=dossier] [MOT_DE_PASSE=mot] \
#        bash linux/correctif/essai-demarrage-installe.sh <iso> <couche.squashfs | -> <sortie>
#
# Après l'installation de la 1719 sur son Samsung : « l'OOBE apparaît 3 s,
# pas le temps de le compléter, puis un chargement de grenOS qui ne se
# termine pas ». Aucun essai ne le voyait : dans QEMU la carte a toujours
# son pilote (bochs). Sur son PC, en mode simple, il n'y a QUE l'image du
# firmware — Plymouth attend alors DeviceTimeout (8 s chez Debian) avant
# d'ouvrir son moteur fbdev, et ce moteur RAPPELLE le terminal 1 (« on wrong
# vt, changing vts », plugins/renderers/frame-buffer/plugin.c). Un SSD a
# déjà affiché le lobby avant ces 8 s ; la clé, plus lente, jamais. Et au
# démarrage suivant, X attendait sans fin le terminal que Plymouth mourant
# ne rendait plus (run 45053914956 : écran noir, aucun bureau).
#
# Trois façons de le lancer :
#   - « - », sans PAQUETS : l'image publiée telle quelle (la panne) ;
#   - une couche : l'installation que fera la PROCHAINE image ;
#   - « - » et PAQUETS : le disque de la 1719, RÉPARÉ PAR UNE MISE À JOUR,
#     sans réinstaller (avec son entrée « debian » et son secours fbx64).
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
# montés en boucle, et jamais sa NVRAM (GRENOS_SANS_NVRAM). Les entrées du
# micrologiciel s'écrivent dans celui de QEMU, par le disque qui démarre.

set -euo pipefail
ISO=$1
COUCHE=$2
OUT=$3
PAQUETS=${PAQUETS:-}
MOT=${MOT_DE_PASSE:-}                    # minuscules aux mêmes places en AZERTY
ICI=$(cd "$(dirname "$0")" && pwd)
RACINE="$ICI/../.."
ECRAN="$RACINE/scripts/ci-screen.py"
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
MON="$OUT/mon.sock"
QMP="$OUT/qmp.sock"
DISQUE="$OUT/disque.img"
WINDOWS="$OUT/windows.img"
VU="$OUT/vu"
QEMU=""
QEMU_VU=""
SERIE=""
RATES=""
BOUCLES=()
CORRIGE=oui
if [ "$COUCHE" = - ] && [ -z "$PAQUETS" ] && [ -z "${JUGER:-}" ]; then CORRIGE=non; fi

etape() { echo; echo "=== $* ==="; }
dire() { echo "$*" | tee -a "$OUT/verdict.txt"; }
rate() { echo "::error::$*"; dire "ECHEC : $*"; RATES="$RATES [$*]"; }
fin() {
    relever || true
    if [ -n "$RATES" ]; then dire "DEMARRAGE : ECHEC$RATES"; exit 1; fi
    dire "DEMARRAGE : le lobby reste, se finit, et le disque rouvre le bureau au demarrage suivant"
}
# Les journaux du disque : AUSSI quand l'essai échoue tôt (45067830100
# n'avait rien laissé pour expliquer le lobby jamais affiché).
RELEVE=""
relever() {
    local j p
    [ -z "$RELEVE" ] && [ -n "$QEMU_VU" ] || return 0
    RELEVE=oui
    boucler "$DISQUE" || return 0
    p=$BOUCLE
    if mount -o ro,noload "${p}p2" "$VU"; then
        j="$VU/var/log/journal"
        journalctl -D "$j" -o short-monotonic --no-pager > "$OUT/journal-installe.txt" 2>/dev/null || true
        # Ce que « quiet » laisse passer à l'écran : le « une erreur evo » de Grenofar.
        journalctl -D "$j" -k -p err -o short-monotonic --no-pager > "$OUT/noyau-erreurs.txt" 2>/dev/null || true
        for j in lightdm/lightdm.log lightdm/x-0.log Xorg.0.log; do
            cp "$VU/var/log/$j" "$OUT/x-$(basename "$j")" 2>/dev/null || true
        done
        umount "$VU"
    fi
    losetup -d "$p" 2>/dev/null || true
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
    rm -f "$MON" "$QMP" "$OUT/vars.fd"
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

# Du TEXTE ou un MENU à l'écran : peu de couleurs, et de l'allumé. Le lobby,
# le chargement et le bureau en ont des milliers ; un curseur ne pèse pas
# un millième de l'écran.
du_texte() { # capture.ppm
    python3 -B - "$ECRAN" "$1" <<'EOF'
import importlib.util, sys
fiche = importlib.util.spec_from_file_location("ecran", sys.argv[1])
ecran = importlib.util.module_from_spec(fiche)
fiche.loader.exec_module(ecran)
with open(sys.argv[2], "rb") as f:
    largeur, hauteur, points = ecran.read_ppm(f.read())
n, couleurs, allumes = largeur * hauteur, set(), 0
for i in range(0, n * 3, 3):
    c = bytes(points[i:i + 3])
    if len(couleurs) <= 16:
        couleurs.add(c)
    else:
        break
    if max(c) > 0x40:
        allumes += 1
sys.exit(0 if len(couleurs) <= 16 and 0.001 < allumes / n < 0.9 else 1)
EOF
}

# La session : seule, ou derrière le mot de passe choisi à l'installation.
ouvrir_session() { # nom de la capture, tours
    local i lettre
    if [ -z "$MOT" ]; then
        attendre 'session de bureau ouverte' "$2"
        return
    fi
    if attendre 'session de bureau ouverte' 20; then
        rate "compte protege, et la session s est ouverte sans mot de passe"
        return 0
    fi
    photo "$1-connexion"
    for i in 1 2 3 4; do
        for lettre in $(echo "$MOT" | sed 's/./& /g'); do touche "$lettre" 0.4; done
        touche ret
        if attendre 'session de bureau ouverte' 25; then
            dire "connexion avec le mot de passe de l installation : ouverte (essai $i)"
            return 0
        fi
    done
    return 1
}

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
  "compte": {"login": "zoe", "nom": "Zoé", "mot_de_passe": sys.argv[3] or None},
  "clavier": {"disposition": "fr", "variante": ""}, "fuseau": "Europe/Paris"}))' "$D" "$EMPREINTE" "$MOT" \
    | lancer_moteur installer > "$OUT/installation.log" 2>&1 \
    || { tail -n 20 "$OUT/installation.log"; rate "l installation a echoue"; fin; }
udevadm settle || true

mount "${D}p2" "$VU"
mount "${D}p1" "$VU/boot/efi"
if [ "$COUCHE" = - ]; then
    # Sur un PC, la 1719 écrivait la NVRAM : grub-install copiait alors aussi
    # fbx64 au chemin de secours. Le disque de Grenofar est ainsi.
    cp "$VU/boot/efi/EFI/debian/fbx64.efi" "$VU/boot/efi/EFI/BOOT/fbx64.efi"
fi

dans_le_disque() {
    mount --bind /dev "$VU/dev"
    mount --bind /dev/pts "$VU/dev/pts"
    mount -t proc proc "$VU/proc"
    mount -t sysfs sys "$VU/sys"
}
hors_du_disque() {
    for m in sys proc dev/pts dev; do umount "$VU/$m"; done
}
if [ -n "$PAQUETS" ]; then
    etape "la mise a jour, sans reinstaller"
    dans_le_disque
    mkdir -p "$VU/var/tmp/grenos-maj"
    cp "$PAQUETS"/*.deb "$VU/var/tmp/grenos-maj/"
    if chroot "$VU" env DEBIAN_FRONTEND=noninteractive sh -c 'dpkg -i /var/tmp/grenos-maj/*.deb' > "$OUT/mise-a-jour.log" 2>&1; then
        dire "paquets de ce depot poses sur le disque de la version publiee"
    else
        tail -n 25 "$OUT/mise-a-jour.log"
        rate "la mise a jour a echoue"
    fi
    rm -rf "$VU/var/tmp/grenos-maj"
    hors_du_disque
elif [ "$COUCHE" != - ]; then
    # Ce que la CONSTRUCTION joue, et qu'une couche de fichiers ne peut pas
    # porter (unités désarmées, 25_bli sans son bit d'exécution).
    etape "ce que la construction de l image fait"
    dans_le_disque
    chroot "$VU" /usr/lib/grenos/appliquer-systeme > "$OUT/construction.log" 2>&1 \
        || rate "appliquer-systeme echoue"
    hors_du_disque
fi

etape "ce que le disque porte"
cp "$VU/boot/grub/grub.cfg" "$OUT/grub-installe.txt"
[ "$(cat "$VU/etc/grenos/affichage" 2>/dev/null)" = firmware ] || rate "le choix firmware de la cle n a pas suivi"
if [ "$CORRIGE" = oui ]; then
    grep -q 'set timeout_style=hidden' "$OUT/grub-installe.txt" || rate "le menu s afficherait"
    if grep -qE 'set timeout=[1-9]' "$OUT/grub-installe.txt"; then rate "le demarrage attendrait devant un menu"; fi
    grep -q 'color_normal=black/black' "$OUT/grub-installe.txt" || rate "GRUB ecrirait a l ecran"
    if grep -q 'insmod bli' "$OUT/grub-installe.txt"; then rate "GRUB chargerait un module refuse par le Secure Boot (bli)"; fi
    for f in "$VU"/etc/systemd/system/*.wants/clamav-freshclam*; do
        if [ -L "$f" ]; then rate "antivirus encore arme : ${f##*/}"; fi
    done
    grep -qE '^[[:space:]]*linux[[:space:]].* systemd.show_status=false' "$OUT/grub-installe.txt" \
        || rate "la ligne du noyau laisse passer des messages"
fi

etape "la sonde, et une carte sans pilote"
# La SEULE différence de matériel : pas de pilote DRM. C'est son mode simple.
sed -i -E '/^[[:space:]]*linux[[:space:]]/ s/$/ modprobe.blacklist=bochs/' "$VU/boot/grub/grub.cfg"
S="$VU/etc/systemd/system"
mkdir -p "$VU/usr/local/sbin" "$S/sysinit.target.wants" "$S/multi-user.target.wants"
cat > "$VU/usr/local/sbin/grenos-sonde" <<'EOF'
#!/bin/sh
while :; do
    read -r depuis _ < /proc/uptime
    echo "sonde $depuis vt=$(cat /sys/class/tty/tty0/active) plymouth=$(pidof plymouthd >/dev/null && echo oui || echo non) fb=$(cat /sys/class/graphics/fb0/name 2>/dev/null | tr ' ' '_')" > /dev/ttyS0
    sleep 1
done
EOF
cat > "$S/grenos-sonde.service" <<'EOF'
[Unit]
Description=essai : quel terminal est a l ecran
DefaultDependencies=no
[Service]
ExecStart=/usr/local/sbin/grenos-sonde
EOF
ln -sf ../grenos-sonde.service "$S/sysinit.target.wants/grenos-sonde.service"
# Le PC de Grenofar a une entrée Windows. Créée HORS de l'ordre (-C), pour
# ne pas décider à la place du service qu'on juge.
cat > "$VU/usr/local/sbin/grenos-essai-windows" <<'EOF'
#!/bin/sh
efibootmgr | grep -q 'Windows Boot Manager' && exit 0
p=$(blkid -L SYSTEM) || exit 0
efibootmgr -q -C -d "/dev/$(lsblk -dno PKNAME "$p")" -p 1 -L "Windows Boot Manager" -l '\EFI\Microsoft\Boot\bootmgfw.efi'
EOF
cat > "$S/grenos-essai-windows.service" <<'EOF'
[Unit]
Description=essai : une entree Windows au micrologiciel
Before=grenos-demarrage.service
After=local-fs.target
[Service]
Type=oneshot
ExecStart=/usr/local/sbin/grenos-essai-windows
EOF
cat > "$VU/usr/local/sbin/grenos-essai-nvram" <<'EOF'
#!/bin/sh
efibootmgr | sed 's/^/nvram: /' > /dev/ttyS0
echo "nvram: fin" > /dev/ttyS0
EOF
cat > "$S/grenos-essai-nvram.service" <<'EOF'
[Unit]
Description=essai : ce que le micrologiciel retient
After=grenos-demarrage.service grenos-essai-windows.service
[Service]
Type=oneshot
ExecStart=/usr/local/sbin/grenos-essai-nvram
EOF
for u in windows nvram; do ln -sf "../grenos-essai-$u.service" "$S/multi-user.target.wants/grenos-essai-$u.service"; done
chmod 0755 "$VU"/usr/local/sbin/grenos-sonde "$VU"/usr/local/sbin/grenos-essai-*
umount "$VU/boot/efi"
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
        -monitor unix:"$MON",server,nowait \
        -qmp unix:"$QMP",server,nowait &
    QEMU=$!
    QEMU_VU=oui
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
rien_d_ecrit() { # prefixe des captures, combien
    local f sales=""
    [ "$CORRIGE" = oui ] || return 0
    for f in $(seq -w 1 "$2"); do
        [ -f "$OUT/$1-$f.ppm" ] || continue
        if du_texte "$OUT/$1-$f.ppm"; then sales="$sales $1-$f"; fi
    done
    if [ -n "$sales" ]; then
        rate "du texte ou un menu a l ecran avant l interface :$sales"
    else
        dire "demarrage $1 : ni menu ni texte a l ecran"
    fi
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
rien_d_ecrit 1 "$(printf '%02d' "$n")"
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

etape "le nom au micrologiciel"
if [ "$CORRIGE" = oui ]; then
    attendre 'nvram: fin' 60 || rate "le micrologiciel n a pas ete relu"
    NVRAM=$(grep -a '^nvram: Boot[0-9A-F][0-9A-F][0-9A-F][0-9A-F]' "$SERIE" | tr -d '\r' || true)
    echo "$NVRAM"
    if echo "$NVRAM" | grep -qE '^nvram: Boot[0-9A-F]+\* grenOS'; then
        dire "micrologiciel : l entree s appelle grenOS"
    else
        rate "aucune entree grenOS au micrologiciel"
    fi
    if echo "$NVRAM" | grep -qiE '^nvram: Boot[0-9A-F]+\*? +debian'; then rate "une entree debian reste au micrologiciel"; fi
    echo "$NVRAM" | grep -q 'Windows Boot Manager' || rate "l entree de Windows a disparu"
fi

etape "finir le lobby"
fini=non
if [ "$(terminal)" = tty7 ]; then
    for _ in $(seq 1 45); do
        touche ret 3
        if [ "$(combien 'nom choisi, la machine est a')" -gt 0 ]; then fini=oui; break; fi
    done
fi
if [ "$fini" = oui ] && ouvrir_session 1 120; then
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
ecran_x=0
for _ in $(seq 1 60); do
    n=$((n + 1))
    photo "2-$(printf '%02d' "$n")"
    if [ "$(combien 'session de bureau ouverte')" -gt 0 ]; then bureau=oui; break; fi
    # Protégé : l'écran de connexion n'annonce rien. Sept secondes de vt7.
    if [ "$(terminal)" = tty7 ]; then ecran_x=$((ecran_x + 1)); fi
    if [ -n "$MOT" ] && [ "$ecran_x" -ge 7 ]; then break; fi
    sleep 1
done
rien_d_ecrit 2 "$(printf '%02d' "$n")"
if [ "$CORRIGE" = oui ]; then
    if grep -aq 'BdsDxe: starting Boot[0-9A-F]* "grenOS"' "$SERIE"; then
        dire "le micrologiciel demarre « grenOS »"
    else
        rate "le micrologiciel n a pas demarre l entree grenOS"
    fi
fi
if [ "$bureau" = non ] && [ "$ecran_x" -gt 0 ] && ouvrir_session 2 30; then bureau=oui; fi
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
    etape "secours du second demarrage : Ctrl+Alt+F7"
    touche ctrl-alt-f7 5
    if ouvrir_session 2-secours 45; then
        sleep 8
        photo 2-secours-f7
        dire "SECOURS Ctrl+Alt+F7 au second demarrage : le bureau s ouvre"
    else
        photo 2-secours-f7
        dire "SECOURS Ctrl+Alt+F7 au second demarrage : sans effet ($(terminal))"
    fi
fi

# ================================================ Windows, sans le menu
# La vraie souris, la vraie fenêtre, polkit, et le micrologiciel qui essaie
# Windows — un faux, refusé : il doit alors rouvrir grenOS de lui-même.
if [ "$CORRIGE" = oui ] && [ "$bureau" = oui ]; then
    etape "Fin de session : Redemarrer sous Windows"
    ouverte=non
    for _ in 1 2 3; do
        touche esc 2
        touche ctrl-esc 4
        POINT=$(grep -a 'bouton eteindre en' "$SERIE" | tail -n 1 | tr -d '\r\n' | sed 's/.*en //' || true)
        if [ -n "$POINT" ]; then
            python3 "$ECRAN" clic "$QMP" "${POINT%,*}" "${POINT#*,}" 1280 800 > /dev/null || true
            if attendre 'arret : [0-9]* geste(s)' 20; then ouverte=oui; break; fi
        fi
    done
    if [ "$ouverte" = oui ]; then
        sleep 4
        photo 2-fin-de-session
        # « sendkey » parle en positions QWERTY : le W d'un clavier AZERTY
        # est la touche z (45069872841 : la fenêtre a reçu un Z).
        touche z 1
        if attendre 'BdsDxe: .*Boot[0-9A-F]* "Windows Boot Manager"' 120; then
            dire "Redemarrer sous Windows : le micrologiciel lance l entree de Windows"
            if attendre 'BdsDxe: starting Boot[0-9A-F]* "grenOS"' 90 2; then
                dire "et grenOS reste le demarrage habituel"
            else
                rate "apres Windows, grenOS n est plus le demarrage habituel"
            fi
            sleep 25
            photo 3-retour
        else
            photo 2-windows-rate
            rate "Redemarrer sous Windows ne mene pas a Windows"
        fi
    else
        photo 2-sans-fin-de-session
        rate "Fin de session ne s est pas ouverte"
    fi
fi
eteindre

etape "le journal du disque"
relever
if [ "$CORRIGE" = oui ]; then
    if grep -aq 'prohibited by secure boot policy' "$OUT"/serie-*.log; then
        rate "GRUB affiche une erreur de Secure Boot"
    else
        dire "GRUB : aucune erreur de Secure Boot"
    fi
    # Antivirus : rien au demarrage, et la session graphique n attend plus.
    if [ ! -s "$OUT/journal-installe.txt" ]; then
        rate "journal illisible : l antivirus n est pas juge"
    elif grep -aqE 'systemd\[1\]: (Starting|Started) clamav-freshclam' "$OUT/journal-installe.txt"; then
        rate "l antivirus telecharge encore au demarrage"
    else
        dire "antivirus : ni telechargeur ni minuteur au demarrage"
        ATTEINT=$(grep -a 'Reached target graphical.target' "$OUT/journal-installe.txt" | tail -n 1 | sed 's/^\[ *//; s/\..*//' || true)
        if [ -n "$ATTEINT" ] && [ "$ATTEINT" -lt 25 ]; then
            dire "dernier demarrage complet en ${ATTEINT} s, sans reseau"
        else
            rate "dernier demarrage pas complet en 25 s sans reseau (${ATTEINT:-jamais})"
        fi
    fi
fi
boucler "$DISQUE"; D=$BOUCLE
if [ "$CORRIGE" = oui ] && mount -o ro "${D}p1" "$VU"; then
    if [ -f "$VU/EFI/BOOT/fbx64.efi" ]; then rate "le secours garde son ecran bleu (fbx64)"; fi
    umount "$VU"
fi
losetup -d "$D"
fin
