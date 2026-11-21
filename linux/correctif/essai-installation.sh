#!/bin/bash
# L'INSTALLATION DE BOUT EN BOUT, puis le premier démarrage du système
# installé, en UEFI Secure Boot (21 novembre).
#
#   sudo bash linux/correctif/essai-installation.sh <iso> <correctif.squashfs> <sortie>
#
# Pourquoi : quatre installations de suite ont échoué sur le PC de Grenofar,
# chacune à une étape plus loin (unsquashfs absent, apt dans le chroot, la
# clé prise pour partition EFI → grub-install code 1). Aucun essai n'allait
# jusqu'au disque installé. Celui-ci le fait.
#
# Ce qu'il fait, dans l'ordre de la suite de Calamares (settings.conf de
# calamares-settings-debian 13.0.13, modules relus dans Calamares 3.3.14) :
#   partition   un disque SATA vierge, GPT : partition EFI + racine ext4
#               (ce que « Effacer le disque » crée) ;
#   unpackfs    l'image de la clé, puis le correctif par-dessus — les deux
#               sources de notre unpackfs.conf ;
#   les montages de notre mount.conf, dans un dossier dont le nom contient
#               « calamares » : nos scripts y retrouvent le chroot comme sous
#               Calamares ;
#   machineid, fstab, users, dpkg-unsafe-io, bootloader-config (le NÔTRE),
#   bootloader (grenos-grub-install avec les arguments exacts de Calamares,
#               puis grub-mkconfig), packages (try_remove de NOTRE
#               packages.conf), initramfs, sources-final.
# Puis le disque démarre SEUL dans QEMU : OVMF en Secure Boot, clés de
# Microsoft, aucune entrée de démarrage — c'est le chemin de secours posé par
# grub-install (shim) qui doit démarrer grenOS jusqu'à l'assistant.
#
# Ce qui n'est PAS Calamares ici : l'interface, et `--no-nvram` (le runner n'a
# pas de variables EFI ; sur un vrai PC, grub-install pose aussi l'entrée du
# micrologiciel). Le reste est le même code, sur la même image.

set -euo pipefail
ISO=$1
CORRECTIF=$2
OUT=$3
ICI=$(cd "$(dirname "$0")" && pwd)
RACINE="$ICI/../.."
mkdir -p "$OUT"
T=/tmp/calamares-root-essai
DISQUE="$OUT/disque.img"
etape() { echo; echo "=== $* ==="; }
faute() { echo "::error::$*"; echo "INSTALLATION : ECHEC — $*" >> "$OUT/verdict.txt"; exit 1; }

# ---------------------------------------------------------------- partition
etape "partition : GPT, EFI 512 Mio + racine ext4 (Effacer le disque)"
rm -f "$DISQUE"
truncate -s 16G "$DISQUE"
LOOP=$(losetup -fP --show "$DISQUE")
echo "disque : $LOOP"
sfdisk -q "$LOOP" <<'EOF'
label: gpt
size=512MiB, type=C12A7328-F81F-11D2-BA4B-00A0C93EC93B, name="EFI"
type=0FC63DAF-8483-4772-8E79-3D69D8477DE4, name="grenOS"
EOF
partx -u "$LOOP" 2>/dev/null || true
udevadm settle 2>/dev/null || true
mkfs.vfat -F 32 -n EFI "${LOOP}p1" >/dev/null
mkfs.ext4 -q -L grenOS "${LOOP}p2"
mkdir -p "$T"
mount "${LOOP}p2" "$T"
mkdir -p "$T/boot/efi"
mount "${LOOP}p1" "$T/boot/efi"

# ----------------------------------------------------------------- unpackfs
etape "unpackfs : l'image, puis le correctif"
xorriso -osirrox on -indev "$ISO" -extract /live/filesystem.squashfs "$OUT/fs.sq" >/dev/null 2>&1
unsquashfs -q -f -d "$T" "$OUT/fs.sq" >/dev/null
rm -f "$OUT/fs.sq"
unsquashfs -q -f -d "$T" "$CORRECTIF" >/dev/null
df -h "$T" | tail -1

# Les services de grenOS ARMÉS sur le disque (21 novembre : au premier
# passage complet, le système installé n'en a démarré AUCUN — ni l'assistant,
# ni l'affichage, ni Windows). On les compte à chaque étape pour voir où ils
# se perdent.
armes() { # moment
    n=$(find "$T/etc/systemd/system" -path '*.wants/grenos-*' 2>/dev/null | wc -l)
    echo "services grenOS armes ($1) : $n — $(find "$T/etc/systemd/system" -path '*.wants/grenos-*' -printf '%f ' 2>/dev/null | head -c 400)"
}
armes "apres la copie de l image"

# ---------------------------------------------------- les montages (mount.conf)
etape "montages du chroot (mount.conf)"
mount -t proc proc "$T/proc"
mount -t sysfs sys "$T/sys"
mount --bind /dev "$T/dev"
mount --bind /dev/pts "$T/dev/pts"
mount -t tmpfs tmpfs "$T/run"
mkdir -p "$T/run/udev"
mount --bind /run/udev "$T/run/udev"

dedans() { chroot "$T" "$@"; }

# Les scripts « process » de calamares-settings-debian tournent depuis la CLÉ
# sous Calamares (le système en direct), pas depuis le disque installé — où
# l'étape packages retire justement ce paquet. On les met donc de côté
# maintenant, comme s'ils restaient sur la clé (premier passage, run
# 44910579224 : « calamares-dpkg-unsafe-io: No such file or directory »).
AIDES="$OUT/aides"
mkdir -p "$AIDES"
cp "$T"/usr/share/calamares/helpers/calamares-* "$AIDES/"

# ------------------------------------------- machineid, fstab, users (Calamares)
etape "machineid, fstab, compte"
rm -f "$T/etc/machine-id"
dedans systemd-machine-id-setup
U_RACINE=$(blkid -s UUID -o value "${LOOP}p2")
U_EFI=$(blkid -s UUID -o value "${LOOP}p1")
cat > "$T/etc/fstab" <<EOF
UUID=$U_RACINE / ext4 defaults,nofail,noatime,errors=remount-ro,x-systemd.device-timeout=10s 0 1
UUID=$U_EFI /boot/efi vfat defaults,nofail,umask=0077,x-systemd.device-timeout=10s 0 2
EOF
dedans useradd -m -s /bin/bash -G sudo,audio,video essai
echo 'essai:essai' | dedans chpasswd
echo "grenos-essai" > "$T/etc/hostname"

# ------------------------------------------------------- les étapes « process »
etape "dpkg-unsafe-io, puis bootloader-config (le nôtre)"
bash "$AIDES/calamares-dpkg-unsafe-io"
if grep -v '^[[:space:]]*#' "$AIDES/calamares-bootloader-config" | grep -q 'apt-get'; then
    faute "bootloader-config est encore celui de Debian (apt-get)"
fi
bash "$AIDES/calamares-bootloader-config" || faute "bootloader-config sort en erreur"

# ---------------------------------------------------------------- bootloader
etape "bootloader : grenos-grub-install avec les arguments de Calamares, puis grub-mkconfig"
[ -x "$T/usr/lib/grenos/grenos-grub-install" ] || faute "grenos-grub-install absent du systeme installe"
# Le nom du dossier EFI : celui que NOTRE bootloader.conf donne à Calamares
# (efiBootloaderId), sinon le nom de l'habillage (bootloaderEntryName).
ID=$(sed -n 's/^efiBootloaderId:[[:space:]]*"\{0,1\}\([^"]*\)"\{0,1\}[[:space:]]*$/\1/p' "$T/usr/share/grenos/bootloader.conf" 2>/dev/null | head -n 1)
[ -n "$ID" ] || ID=$(sed -n 's/^[[:space:]]*bootloaderEntryName:[[:space:]]*//p' "$T/usr/share/calamares/branding/grenos/branding.desc" | head -n 1)
echo "dossier EFI : $ID"
dedans /usr/lib/grenos/grenos-grub-install --target=x86_64-efi --efi-directory=/boot/efi \
    --bootloader-id="$ID" --force --no-nvram || faute "grub-install sort en erreur"
dedans grub-mkconfig -o /boot/grub/grub.cfg || faute "grub-mkconfig sort en erreur"
echo "partition EFI :"
( cd "$T/boot/efi" && find . -type f | sort )
for f in EFI/BOOT/BOOTX64.EFI EFI/BOOT/grubx64.efi "EFI/$ID/shimx64.efi" "EFI/$ID/grubx64.efi" "EFI/$ID/grub.cfg"; do
    [ -f "$T/boot/efi/$f" ] || faute "$f absent de la partition EFI"
done
cmp -s "$T/boot/efi/EFI/BOOT/BOOTX64.EFI" "$T/usr/lib/shim/shimx64.efi.signed" \
    || faute "le chemin de secours n est pas shim : un PC en Secure Boot le refuserait"
echo "chemin de secours : shim signe (comme EFI/$ID/shimx64.efi)"

# ----------------------------------------------------------------- packages
etape "packages : try_remove de notre packages.conf"
CONF="$T/etc/calamares/modules/packages.conf"
[ -f "$CONF" ] || faute "packages.conf absent"
sed -n '/try_remove:/,$p' "$CONF" | tail -n +2 | sed -n 's/^[[:space:]]*- //p' | while read -r p; do
    if dedans apt-get --purge -q -y remove "$p" </dev/null >/dev/null 2>&1; then echo "  retire (ou absent) : $p"
    else echo "  refus d apt (sans gravite) : $p"; fi
done

armes "apres le retrait des paquets de la cle"

# ---------------------------------------------------------------- initramfs
etape "initramfs"
dedans update-initramfs -k all -c -t || faute "update-initramfs sort en erreur"
ls -la "$T"/boot/initrd.img-* "$T"/boot/vmlinuz-*

armes "a la fin de l installation"
if [ "$(find "$T/etc/systemd/system" -path '*.wants/grenos-*' 2>/dev/null | wc -l)" -eq 0 ]; then
    echo "::warning::le systeme installe n a AUCUN service grenOS arme (assistant, affichage, Windows...)"
fi

etape "dpkg-unsafe-io -u, sources-final, demontage"
bash "$AIDES/calamares-dpkg-unsafe-io" -u
bash "$AIDES/calamares-sources-final"
sync
umount -R "$T"
losetup -d "$LOOP"

# ------------------------------------------- le premier démarrage, Secure Boot
etape "premier demarrage du disque installe : OVMF Secure Boot, cles Microsoft"
CODE=/usr/share/OVMF/OVMF_CODE_4M.secboot.fd
VARS=/usr/share/OVMF/OVMF_VARS_4M.ms.fd
cp "$VARS" "$OUT/vars.fd"
ACCEL=""
if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then ACCEL="-enable-kvm -cpu host"; fi
# shellcheck disable=SC2086
qemu-system-x86_64 $ACCEL -machine q35,smm=on -global driver=cfi.pflash01,property=secure,value=on \
    -m 4096 -smp 4 \
    -drive if=pflash,format=raw,readonly=on,file="$CODE" -drive if=pflash,format=raw,file="$OUT/vars.fd" \
    -drive if=none,id=ssd,format=raw,file="$DISQUE" -device ide-hd,drive=ssd,bus=ide.0 \
    -device qemu-xhci,id=xhci -device usb-tablet,bus=xhci.0 \
    -vga std -display none \
    -serial file:"$OUT/serie.log" \
    -monitor unix:"$OUT/mon.sock",server,nowait &
QEMU=$!
sleep 3
kill -0 "$QEMU" 2>/dev/null || faute "QEMU n a pas demarre"
t=0
for cap in 40 80 130; do
    sleep $((cap - t)); t=$cap
    kill -0 "$QEMU" 2>/dev/null || { echo "QEMU s est arrete seul a ~${t}s"; break; }
    python3 "$RACINE/scripts/ci-screen.py" grab "$OUT/mon.sock" "$OUT/installe-${cap}s.ppm" || true
    python3 "$RACINE/scripts/ci-screen.py" judge "$OUT/installe-${cap}s.ppm" "$OUT/installe-${cap}s.png" > "$OUT/installe-${cap}s.txt" 2>&1 || true
    echo "${cap} s : $(head -1 "$OUT/installe-${cap}s.txt" 2>/dev/null)"
    rm -f "$OUT/installe-${cap}s.ppm"
done
python3 "$RACINE/scripts/ci-screen.py" send "$OUT/mon.sock" "system_powerdown" || true
for _ in $(seq 1 40); do kill -0 "$QEMU" 2>/dev/null || break; sleep 3; done
kill "$QEMU" 2>/dev/null || true
sleep 2

# --------------------------------------------------------------- le verdict
etape "ce que le systeme installe a dit"
echo "--- port serie"
cat "$OUT/serie.log" 2>/dev/null | tr -d '\r' | tail -n 30 || true
LOOP=$(losetup -fP --show "$DISQUE")
mkdir -p "$OUT/racine"
mount -o ro "${LOOP}p2" "$OUT/racine"
JOURNAL=""
if [ -d "$OUT/racine/var/log/journal" ]; then
    journalctl -D "$OUT/racine/var/log/journal" -b -o short-monotonic --no-pager > "$OUT/journal-installe.txt" 2>/dev/null || true
    JOURNAL="$OUT/journal-installe.txt"
    grep -E 'Secure boot|secureboot|grenos-ui: premier|Started lightdm|Reached target graphical|Failed to start' "$JOURNAL" | head -30 || true
fi
umount "$OUT/racine"
losetup -d "$LOOP"

PREUVE=$( { cat "$OUT/serie.log" 2>/dev/null; cat "${JOURNAL:-/dev/null}"; } | grep -cE 'premier : ecran bienvenue|premier ecran : le programme demarre|Started lightdm' || true)
if [ "${PREUVE:-0}" -eq 0 ]; then
    faute "le systeme installe n a atteint ni l assistant ni lightdm (captures dans la sortie)"
fi
if [ -n "$JOURNAL" ] && ! grep -qiE 'secure ?boot enabled|secureboot: Secure boot enabled' "$JOURNAL"; then
    echo "::warning::le journal ne dit pas que le Secure Boot etait actif"
fi
echo "INSTALLATION : le systeme installe demarre en Secure Boot jusqu a l assistant" | tee -a "$OUT/verdict.txt"
