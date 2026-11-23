#!/bin/bash
# LE PILOTE NVIDIA SANS ÉCRAN BLEU, AVEC LE SECURE BOOT (23 novembre).
#
# Grenofar : « garde secure boot et fais tout, débrouille-toi », « ça
# fonctionne sur Windows ». Windows n'a pas d'écran bleu parce que MICROSOFT
# signe le pilote NVIDIA, et que le PC fait déjà confiance à Microsoft. Le
# pilote que grenOS compile (DKMS) est signé par une clé à NOUS, inconnue du
# PC : il faut l'inscrire une fois (MokManager), et ça, aucun logiciel ne peut
# le faire à la place de la personne — c'est le Secure Boot lui-même.
#
# La seule voie qui reste : un pilote DÉJÀ signé par quelqu'un que le PC
# connaît. Ubuntu le fait : son shim est signé par Microsoft et contient la
# clé de Canonical, son noyau est signé par Canonical, et Canonical publie le
# pilote NVIDIA 580 « open » COMPILÉ ET SIGNÉ pour chaque noyau
# (linux-modules-nvidia-580-open-<noyau>, lu sur packages.ubuntu.com le 23
# novembre : 26.04, noyau 7.0.0-43, nvidia.ko dans le paquet).
#
# Cet essai prouve, ou réfute, AVANT de toucher à grenOS : un système Debian
# trixie, démarré par la chaîne d'Ubuntu (shim → GRUB → noyau) dans QEMU en
# Secure Boot avec les clés de Microsoft, charge-t-il nvidia.ko SANS aucune
# inscription ? Témoin contraire : le même noyau refuse un module dont on a
# retiré la signature — sinon l'essai ne prouverait rien.
#
# Sans carte NVIDIA dans QEMU, un module accepté s'initialise puis répond
# « No such device » (NVRM: No NVIDIA GPU found) ; un module refusé ne
# s'initialise pas du tout (« Key was rejected by service »).
#
# Usage (root, sur un Ubuntu avec docker) : essai-pilote-signe.sh <dossier>
set -euo pipefail

OUT="${1:?dossier de travail}"
SERIE_UBUNTU="${GRENOS_UBUNTU:-26.04}"
mkdir -p "$OUT"
OUT=$(readlink -f "$OUT")
DEBS="$OUT/debs"
RACINE="$OUT/racine"
DISQUE="$OUT/disque.img"
mkdir -p "$DEBS"

rate() { echo "ESSAI ROUGE : $*" >&2; exit 1; }

echo "== 1. Les paquets signés d'Ubuntu $SERIE_UBUNTU"
docker run --rm -v "$DEBS:/debs" "ubuntu:$SERIE_UBUNTU" sh -euc '
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    meta=linux-modules-nvidia-580-open-generic
    nv=$(apt-cache depends "$meta" | sed -n "s/.*Depends: \(linux-modules-nvidia-580-open-[0-9].*-generic\)$/\1/p" | head -1)
    [ -n "$nv" ] || { echo "paquet nvidia du noyau introuvable" >&2; exit 1; }
    abi=${nv#linux-modules-nvidia-580-open-}
    echo "$abi" > /debs/abi
    cd /debs
    apt-get download -qq "$nv" "linux-image-$abi" "linux-modules-$abi" shim-signed grub-efi-amd64-signed
    ls -la /debs
'
ABI=$(cat "$DEBS/abi")
echo "noyau d Ubuntu : $ABI"

echo "== 2. Un Debian trixie minimal"
if [ ! -x "$RACINE/usr/bin/systemctl" ]; then
    debootstrap --variant=minbase --include=systemd-sysv,udev,kmod,initramfs-tools,zstd,mokutil \
        trixie "$RACINE" http://deb.debian.org/debian
fi

echo "== 3. Le noyau et le pilote d'Ubuntu, posés dans ce Debian"
# Extraits à part, puis fondus : trixie a /lib -> usr/lib, que tar
# remplacerait par un vrai dossier.
rm -rf "$OUT/ub"; mkdir -p "$OUT/ub"
for p in "$DEBS"/linux-image-*.deb "$DEBS"/linux-modules-[0-9]*.deb "$DEBS"/linux-modules-nvidia-*.deb; do
    dpkg-deb -x "$p" "$OUT/ub"
done
rsync -aK "$OUT/ub/" "$RACINE/"
[ -L "$RACINE/lib" ] || rate "/lib n est plus un lien : le systeme d essai est abime"
[ -f "$RACINE/boot/vmlinuz-$ABI" ] || rate "vmlinuz-$ABI absent du paquet d Ubuntu"
ls "$RACINE/lib/modules/$ABI/kernel/nvidia-580-open/nvidia.ko" || rate "nvidia.ko absent"
for m in proc sys dev; do mount --bind "/$m" "$RACINE/$m"; done
chroot "$RACINE" depmod "$ABI"
chroot "$RACINE" update-initramfs -c -k "$ABI" >/dev/null
for m in dev sys proc; do umount "$RACINE/$m"; done
echo "signataire de nvidia.ko : $(chroot "$RACINE" modinfo -F signer nvidia -k "$ABI" 2>/dev/null || echo ?)"

# Le témoin contraire : un petit module du même noyau, SANS sa signature.
temoin=$(find "$RACINE/lib/modules/$ABI/kernel/drivers/net" -name 'dummy.ko*' | head -1)
[ -n "$temoin" ] || rate "dummy.ko absent"
case "$temoin" in
    *.zst) zstd -dc "$temoin" > "$OUT/dummy.ko" ;;
    *.xz) xz -dc "$temoin" > "$OUT/dummy.ko" ;;
    *) cp "$temoin" "$OUT/dummy.ko" ;;
esac
python3 - "$OUT/dummy.ko" "$RACINE/root/dummy-sans-signature.ko" <<'PY'
import struct, sys
d = open(sys.argv[1], "rb").read()
magie = b"~Module signature appended~\n"
assert d.endswith(magie), "dummy.ko n'est pas signe : le temoin ne vaut rien"
longueur = struct.unpack(">I", d[-len(magie) - 4:-len(magie)])[0]
open(sys.argv[2], "wb").write(d[:-len(magie) - 12 - longueur])
PY

cat > "$RACINE/usr/local/bin/essai" <<'ESSAI'
#!/bin/sh
exec > /dev/ttyS0 2>&1
echo "ESSAI noyau $(uname -r)"
echo "ESSAI secure boot : $(mokutil --sb-state 2>&1)"
echo "ESSAI verrou : $(cat /sys/kernel/security/lockdown 2>&1)"
echo "ESSAI cles en attente : $(mokutil --list-new 2>&1 | head -1)"
if insmod /root/dummy-sans-signature.ko 2>/tmp/t; then
    echo "ESSAI temoin : CHARGE (le Secure Boot ne verifie rien)"
else
    echo "ESSAI temoin : refuse ($(cat /tmp/t))"
fi
modprobe nvidia 2>/tmp/n
echo "ESSAI nvidia : code $? ($(cat /tmp/n))"
dmesg | grep -iE 'nvrm|nvidia|rejected|signature|lockdown' | tail -n 15 | sed 's/^/ESSAI dmesg /'
echo "ESSAI FIN"
ESSAI
chmod +x "$RACINE/usr/local/bin/essai"

echo "== 4. Le disque : EFI d'Ubuntu (shim de Microsoft, GRUB de Canonical) + Debian"
work="$OUT/x"; rm -rf "$work"; mkdir -p "$work"
for p in "$DEBS"/shim-signed_*.deb "$DEBS"/grub-efi-amd64-signed_*.deb; do dpkg-deb -x "$p" "$work"; done
shim=$(ls "$work"/usr/lib/shim/shimx64.efi.signed.latest "$work"/usr/lib/shim/shimx64.efi.signed 2>/dev/null | head -1)
grub=$(ls "$work"/usr/lib/grub/x86_64-efi-signed/grubx64.efi.signed | head -1)
[ -n "$shim" ] && [ -n "$grub" ] || rate "shim ou GRUB signes introuvables"
rm -f "$DISQUE"
truncate -s 3G "$DISQUE"
sfdisk -q "$DISQUE" <<'TABLE'
label: gpt
size=200MiB, type=uefi
type=linux
TABLE
boucle=$(losetup -fP --show "$DISQUE")
trap 'umount -R "$OUT/m" 2>/dev/null || true; losetup -d "$boucle" 2>/dev/null || true' EXIT
mkfs.vfat -n ESP "${boucle}p1" >/dev/null
mkfs.ext4 -q -L grenos-essai "${boucle}p2"
mkdir -p "$OUT/m"
mount "${boucle}p2" "$OUT/m"
cp -a "$RACINE/." "$OUT/m/"
mkdir -p "$OUT/m/boot/efi"
mount "${boucle}p1" "$OUT/m/boot/efi"
for dossier in BOOT ubuntu; do
    mkdir -p "$OUT/m/boot/efi/EFI/$dossier"
    cp "$grub" "$OUT/m/boot/efi/EFI/$dossier/grubx64.efi"
    cat > "$OUT/m/boot/efi/EFI/$dossier/grub.cfg" <<CFG
search --no-floppy --label grenos-essai --set root
linux /boot/vmlinuz-$ABI root=LABEL=grenos-essai ro console=ttyS0 systemd.run=/usr/local/bin/essai systemd.run_success_action=poweroff systemd.run_failure_action=poweroff
initrd /boot/initrd.img-$ABI
boot
CFG
done
cp "$shim" "$OUT/m/boot/efi/EFI/BOOT/BOOTX64.EFI"
cp "$shim" "$OUT/m/boot/efi/EFI/ubuntu/shimx64.efi"
umount -R "$OUT/m"
losetup -d "$boucle"
trap - EXIT

echo "== 5. QEMU, Secure Boot avec les clés de Microsoft"
cp /usr/share/OVMF/OVMF_VARS_4M.ms.fd "$OUT/vars.fd"
accel=()
if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then accel=(-enable-kvm -cpu host); fi
timeout 600 qemu-system-x86_64 "${accel[@]}" -machine q35,smm=on \
    -global driver=cfi.pflash01,property=secure,value=on -m 2048 -smp 2 \
    -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.secboot.fd \
    -drive if=pflash,format=raw,file="$OUT/vars.fd" \
    -drive file="$DISQUE",format=raw,if=virtio \
    -nic none -display none -serial file:"$OUT/serie.log" || true
grep -a 'ESSAI' "$OUT/serie.log" || { tail -c 4000 "$OUT/serie.log"; rate "le systeme n a pas demarre jusqu a l essai"; }

echo "== 6. Le jugement"
s="$OUT/serie.log"
grep -aq 'ESSAI secure boot : SecureBoot enabled' "$s" || rate "le Secure Boot n etait pas actif : l essai ne prouve rien"
grep -aq 'ESSAI temoin : refuse' "$s" || rate "le module SANS signature a ete charge : le Secure Boot ne verifiait rien"
grep -aqiE 'ESSAI nvidia : .*(rejected|not permitted)|ESSAI dmesg .*nvidia.*rejected' "$s" \
    && rate "nvidia.ko signe par Canonical REFUSE : la voie sans ecran bleu ne marche pas ainsi"
grep -aqE 'ESSAI dmesg .*NVRM' "$s" || rate "nvidia.ko ne s est pas initialise (aucune ligne NVRM)"
echo "ESSAI VERT : Secure Boot actif, module sans signature refuse, nvidia.ko de Canonical charge SANS ecran bleu"
