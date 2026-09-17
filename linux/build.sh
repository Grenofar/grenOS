#!/bin/sh
# Construit l'image grenOS. À lancer en root dans un conteneur Debian trixie
# privilégié (live-build monte des systèmes de fichiers) :
#
#   docker run --privileged -v "$PWD:/work" -w /work debian:trixie \
#       sh -c "apt-get update && apt-get install -y sudo && linux/build.sh"
#
# Résultat : linux/grenos-amd64.hybrid.iso

set -eu

cd "$(dirname "$0")"
export LC_ALL=C LANG=C DEBIAN_FRONTEND=noninteractive

echo "--- outils ---"
apt-get update -qq
apt-get install -y -qq --no-install-recommends \
    live-build debootstrap xorriso squashfs-tools isolinux syslinux-common \
    grub-efi-amd64-bin grub-pc-bin mtools dosfstools ca-certificates \
    python3 python3-numpy imagemagick fonts-noto-core >/dev/null

echo "--- fonds d'écran et logo ---"
mkdir -p config/includes.chroot/usr/share/grenos
python3 tools/wallpaper.py config/includes.chroot/usr/share/grenos 2560 1440
# Le logo : le nom, dans la police du système, sur fond transparent. Il sert à
# l'écran de démarrage et à l'accueil.
convert -size 900x260 xc:none \
    -font /usr/share/fonts/truetype/noto/NotoSans-Bold.ttf -pointsize 150 \
    -fill '#e6ebf5' -gravity center -annotate +0+0 'grenOS' \
    PNG32:config/includes.chroot/usr/share/grenos/logo.png

echo "--- configuration ---"
lb clean --purge >/dev/null 2>&1 || true
lb config

echo "--- construction ---"
lb build 2>&1 | tail -n 60

ls -la *.iso
echo "--- taille ---"
du -h ./*.iso
