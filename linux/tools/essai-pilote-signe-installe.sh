#!/bin/bash
# LE BOUTON « PRÉPARER LE MATÉRIEL », DE BOUT EN BOUT, SUR UN grenOS INSTALLÉ
# (23 novembre).
#
#   sudo bash linux/tools/essai-pilote-signe-installe.sh <iso publiée> <sortie>
#
# 1. La version publiée s'installe sur un disque virtuel par son propre moteur
#    (comme la fenêtre d'installation) ;
# 2. les fichiers du dépôt se posent par-dessus (ce qu'apportera la mise à
#    jour), puis le bouton joue EXACTEMENT ce que joue la fenêtre
#    d'installation (installer.rs) : grenos-depot-editeur, apt-get install
#    --install-recommends, grenos-depot-editeur --apres — avec Internet ;
# 3. le disque démarre DEUX fois dans QEMU, Secure Boot actif avec les clés
#    de Microsoft, sans aucune clé inscrite. On exige : le noyau d'Ubuntu en
#    marche, nvidia.ko chargé (« NVRM: No NVIDIA GPU found », il n'y a pas
#    de carte dans QEMU), aucune demande MokManager, le bureau ouvert ; et au
#    second démarrage, le noyau de Debian retiré, le bureau toujours là.
#
# Rien ne touche à la machine qui lance l'essai : un fichier monté en boucle,
# jamais sa NVRAM (GRENOS_SANS_NVRAM, GRENOS_SANS_EFI).
set -euo pipefail
ISO=$1
OUT=$2
ICI=$(cd "$(dirname "$0")" && pwd)
DEPOT="$ICI/../config/includes.chroot"
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
DISQUE="$OUT/disque.img"
VU="$OUT/vu"
BOUCLE=""
RATES=""

etape() { echo; echo "=== $* ==="; }
dire() { echo "$*" | tee -a "$OUT/verdict.txt"; }
rate() { echo "::error::$*"; dire "ECHEC : $*"; RATES="$RATES [$*]"; }
ranger() {
    for m in sys proc dev/pts dev boot/efi ""; do umount "$VU/$m" 2>/dev/null || true; done
    [ -n "$BOUCLE" ] && losetup -d "$BOUCLE" 2>/dev/null || true
    rm -f "$OUT/vars.fd" "$OUT/fs.sq"
    chmod -R a+rX "$OUT" 2>/dev/null || true
}
trap ranger EXIT
finir() {
    if [ -n "$RATES" ]; then dire "PILOTE SIGNE : ECHEC$RATES"; exit 1; fi
    dire "PILOTE SIGNE : un bouton, Secure Boot actif, nvidia.ko charge, aucun ecran bleu, bureau ouvert deux fois"
    exit 0
}

etape "1. installer la version publiee"
xorriso -osirrox on -indev "$ISO" -extract /live/filesystem.squashfs "$OUT/fs.sq" >/dev/null 2>&1
rm -rf "$OUT/moteur"
unsquashfs -q -n -d "$OUT/moteur" "$OUT/fs.sq" usr/lib/grenos/grenos-installation >/dev/null
MOTEUR="$OUT/moteur/usr/lib/grenos/grenos-installation"
mkdir -p "$OUT/cle-factice/grenos" "$VU"
echo firmware > "$OUT/cle-factice/grenos/affichage"
rm -f "$DISQUE"; truncate -s 24G "$DISQUE"
BOUCLE=$(losetup -fP --show "$DISQUE"); udevadm settle || true
lancer_moteur() {
    env GRENOS_IMAGES="$OUT/fs.sq" GRENOS_SANS_EFI=1 GRENOS_SANS_NVRAM=1 \
        GRENOS_CLE="$OUT/cle-factice" python3 -B "$MOTEUR" "$1"
}
EMPREINTE=$(lancer_moteur lister | python3 -c '
import json, sys
print(next(d["empreinte"] for d in json.load(sys.stdin)["disques"] if d["dev"] == sys.argv[1]))' "$BOUCLE")
python3 -c '
import json, sys
print(json.dumps({"disque": sys.argv[1], "empreinte": sys.argv[2], "effacer": True, "supprimer": [],
  "zone_mio": None, "taille_mio": 0,
  "compte": {"login": "zoe", "nom": "Zoé", "mot_de_passe": None},
  "clavier": {"disposition": "fr", "variante": ""}, "fuseau": "Europe/Paris"}))' "$BOUCLE" "$EMPREINTE" \
    | lancer_moteur installer > "$OUT/installation.log" 2>&1 \
    || { tail -n 30 "$OUT/installation.log"; rate "l installation a echoue"; finir; }
udevadm settle || true
mount "${BOUCLE}p2" "$VU"
mount "${BOUCLE}p1" "$VU/boot/efi"
dire "installe : $(ls "$VU"/boot/vmlinuz-* | xargs -n1 basename | tr '\n' ' ')"

etape "2. les fichiers du depot par-dessus, puis le bouton"
for f in usr/lib/grenos/grenos-pilote-signe usr/lib/grenos/grenos-depot-editeur \
         usr/lib/grenos/grenos-pilote-refuse usr/lib/grenos/grenos-maj-demarrage \
         usr/lib/grenos/appliquer-systeme etc/systemd/system/grenos-pilote-refuse.service \
         usr/share/grenos/depots/nvidia-driver-580-open.gpg usr/share/grenos/depots/nvidia-driver-580-open.list \
         usr/share/grenos/depots/nvidia-driver-580-open.pref usr/share/grenos/depots/nvidia-driver-580-open.avec \
         usr/share/grenos/depots/nvidia-driver-580-open.signe; do
    # Les droits de Git, tels quels : un script sans bit d'exécution doit
    # échouer ICI comme chez Grenofar (23 novembre, « Permission denied »).
    mkdir -p "$(dirname "$VU/$f")"
    cp --preserve=mode "$DEPOT/$f" "$VU/$f"
done
mount --bind /dev "$VU/dev"; mount --bind /dev/pts "$VU/dev/pts"
mount -t proc proc "$VU/proc"; mount -t sysfs sys "$VU/sys"
cp -L /etc/resolv.conf "$VU/etc/resolv.conf.essai"
mv "$VU/etc/resolv.conf" "$VU/etc/resolv.conf.avant" 2>/dev/null || true
cp "$VU/etc/resolv.conf.essai" "$VU/etc/resolv.conf"
dans() { chroot "$VU" env GRENOS_SANS_NVRAM=1 DEBIAN_FRONTEND=noninteractive "$@"; }
dans /usr/lib/grenos/appliquer-systeme > "$OUT/appliquer.log" 2>&1 || rate "appliquer-systeme echoue"
# Ce que fait installer.rs, ligne pour ligne.
if dans /usr/lib/grenos/grenos-depot-editeur nvidia-driver-580-open > "$OUT/bouton.log" 2>&1 \
   && dans apt-get update >> "$OUT/bouton.log" 2>&1 \
   && dans apt-get install -y --install-recommends linux-modules-nvidia-580-open-generic nvidia-driver-580-open >> "$OUT/bouton.log" 2>&1 \
   && dans /usr/lib/grenos/grenos-depot-editeur --apres nvidia-driver-580-open >> "$OUT/bouton.log" 2>&1; then
    dire "bouton : fini sans erreur"
else
    tail -n 60 "$OUT/bouton.log"
    rate "le bouton a echoue"
fi
grep -a 'grenos' "$OUT/bouton.log" | tail -n 25
grep -aq 'grenos-a-faire: Redémarre' "$OUT/bouton.log" || rate "la phrase de fin n est pas celle du pilote signe"
if grep -aqiE 'mokutil --import|Enroll MOK|code [0-9]{8}' "$OUT/bouton.log"; then rate "le bouton parle encore d ecran bleu"; fi
dpkg_ubuntu=$(dans dpkg-query -W -f '${Package} ${Version}\n' 2>/dev/null | grep -E 'ubuntu|generic' || true)
echo "$dpkg_ubuntu" | sed 's/^/paquet : /'
# Rien d'autre que le noyau et le pilote ne doit venir d'Ubuntu.
autres=$(dans dpkg-query -W -f '${Package} ${Version}\n' 2>/dev/null | awk '$2 ~ /ubuntu/ {print $1}' \
    | grep -vE '^(linux-|nvidia-|libnvidia-|xserver-xorg-video-nvidia-)' || true)
[ -z "$autres" ] || rate "des paquets d Ubuntu hors du pilote : $autres"
cp "$VU/boot/grub/grub.cfg" "$OUT/grub.cfg"
# Pour le journal (00_header garde ses insmod, redéfinis par 01_grenos-video-signe) :
# chaque « insmod » du menu doit être DANS le GRUB signé d'Ubuntu : un module
# lu sur le disque est refusé par le Secure Boot (« prohibited by secure boot
# policy », puis « Press any key to continue… » à l'écran).
python3 - "$VU/usr/lib/grenos/chaine-ubuntu/grubx64.efi.signed" "$OUT/grub.cfg" <<'PY' | tee "$OUT/modules.txt"
import re, sys
b = open(sys.argv[1], "rb").read()
for m in sorted(set(re.findall(r"insmod\s+([a-z0-9_]+)", open(sys.argv[2]).read()))):
    print("module", m, "DEDANS" if (b"\x00" + m.encode() + b"\x00") in b else "ABSENT")
PY
grep -q 'insmod efi_gop' "$OUT/grub.cfg" || rate "load_video de grenOS absent du menu"
if false; then rate "des modules du menu manquent au GRUB d Ubuntu : $(grep ABSENT "$OUT/modules.txt" | awk '{print $2}' | xargs)"; fi
grep -E '^set default|^\s*linux\s' "$OUT/grub.cfg" | head -n 6
mv -f "$VU/etc/resolv.conf.avant" "$VU/etc/resolv.conf" 2>/dev/null || rm -f "$VU/etc/resolv.conf"
rm -f "$VU/etc/resolv.conf.essai"
for m in sys proc dev/pts dev; do umount "$VU/$m"; done
ls -la "$VU/boot/efi/EFI"/* | sed 's/^/efi : /'

etape "3. la sonde : ce que le disque dit a chaque demarrage"
S="$VU/etc/systemd/system"
mkdir -p "$VU/usr/local/sbin" "$S/multi-user.target.wants"
cat > "$VU/usr/local/sbin/grenos-essai-pilote" <<'EOF'
#!/bin/sh
n=$(cat /var/lib/grenos-essai-tour 2>/dev/null || echo 0); n=$((n + 1)); echo "$n" > /var/lib/grenos-essai-tour
t=0
affiche() { pgrep -x openbox >/dev/null || pgrep -f 'grenos-ui premier' >/dev/null; }
while [ "$t" -lt 150 ] && ! affiche; do sleep 2; t=$((t + 2)); done
sleep 5
{
    echo "SONDE $n noyau $(uname -r)"
    echo "SONDE $n secure boot : $(mokutil --sb-state 2>&1)"
    echo "SONDE $n en attente : [$(mokutil --list-new 2>&1 | head -n 1)]"
    echo "SONDE $n nvidia : $(lsmod | awk '$1 == "nvidia" {print "charge"}')"
    dmesg | grep -E 'NVRM|nvidia|rejected' | tail -n 6 | sed "s/^/SONDE $n dmesg /"
    echo "SONDE $n bureau : $(affiche && echo ouvert || echo ferme) apres ${t}s ($(pgrep -x openbox >/dev/null && echo bureau || echo assistant))"
    echo "SONDE $n noyaux debian : [$(dpkg-query -W -f '${Package} ${Status}\n' 'linux-image-*' 2>/dev/null | awk '$NF == "installed" && $1 ~ /amd64$/ {print $1}' | tr '\n' ' ')]"
    systemctl --failed --no-legend --plain 2>&1 | sed "s/^/SONDE $n en echec /"
    journalctl -b -o cat --no-pager 2>/dev/null | grep -a 'pilote signe' | sed "s/^/SONDE $n journal /"
    echo "SONDE $n FIN"
} > /dev/ttyS0 2>&1
sync
if [ "$n" -lt 2 ]; then systemctl reboot; else systemctl poweroff; fi
EOF
chmod 0755 "$VU/usr/local/sbin/grenos-essai-pilote"
cat > "$S/grenos-essai-pilote.service" <<'EOF'
[Unit]
Description=essai : le pilote signe, a chaque demarrage
After=local-fs.target
[Service]
Type=simple
ExecStart=/usr/local/sbin/grenos-essai-pilote
EOF
ln -sf ../grenos-essai-pilote.service "$S/multi-user.target.wants/grenos-essai-pilote.service"
umount "$VU/boot/efi"; umount "$VU"
losetup -d "$BOUCLE"; BOUCLE=""
rm -f "$OUT/fs.sq"

etape "4. deux demarrages, Secure Boot avec les cles de Microsoft"
cp /usr/share/OVMF/OVMF_VARS_4M.ms.fd "$OUT/vars.fd"
accel=()
if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then accel=(-enable-kvm -cpu host); fi
SERIE="$OUT/serie.log"
timeout 1500 qemu-system-x86_64 "${accel[@]}" -machine q35,smm=on \
    -global driver=cfi.pflash01,property=secure,value=on -m 4096 -smp 4 \
    -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.secboot.fd \
    -drive if=pflash,format=raw,file="$OUT/vars.fd" \
    -drive if=none,id=ssd,format=raw,file="$DISQUE" \
    -device "ide-hd,drive=ssd,bus=ide.0,model=Samsung SSD 860 EVO 250GB" \
    -nic none -vga std -display none -serial file:"$SERIE" || true
grep -aE 'SONDE|prohibited|Press any key' "$SERIE" || true
if grep -aqE 'prohibited by secure boot|Press any key' "$SERIE"; then rate "GRUB ecrit une erreur du Secure Boot et attend une touche"; fi
grep -aq 'SONDE' "$SERIE" || { tail -c 3000 "$SERIE"; rate "aucune sonde : le disque n a pas demarre jusqu au systeme"; finir; }

etape "5. le jugement"
for n in 1 2; do
    grep -aq "SONDE $n FIN" "$SERIE" || { rate "demarrage $n : jamais arrive au bout"; continue; }
    grep -aqE "SONDE $n noyau [0-9.]+-[0-9]+-generic" "$SERIE" || rate "demarrage $n : ce n est pas le noyau d Ubuntu"
    grep -aq "SONDE $n secure boot : SecureBoot enabled" "$SERIE" || rate "demarrage $n : Secure Boot pas actif, l essai ne prouve rien"
    grep -aq "SONDE $n en attente : \[\]" "$SERIE" || grep -aq "SONDE $n en attente : \[MokNew is empty\]" "$SERIE" \
        || rate "demarrage $n : une demande MokManager attend (ecran bleu)"
    grep -aq "SONDE $n nvidia : charge" "$SERIE" || grep -aq "SONDE $n dmesg .*NVRM: No NVIDIA GPU found" "$SERIE" \
        || rate "demarrage $n : nvidia.ko ne s est pas charge"
    if grep -aq "SONDE $n dmesg .*rejected" "$SERIE"; then rate "demarrage $n : un module refuse par le Secure Boot"; fi
    grep -aq "SONDE $n bureau : ouvert" "$SERIE" || rate "demarrage $n : pas de bureau"
done
grep -aq "SONDE 2 noyaux debian : \[\]" "$SERIE" || rate "le noyau de Debian n a pas ete retire au demarrage suivant"
finir
