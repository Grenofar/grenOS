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

echo "--- catalogue du magasin ---"
mkdir -p config/includes.chroot/usr/share/grenos
# GrenPlace demande son catalogue à notre backend ; cette copie n'est là que
# pour le premier lancement et pour les machines sans réseau.
cp data/catalogue.json config/includes.chroot/usr/share/grenos/catalogue.json
python3 -c "import json,sys; d=json.load(open('data/catalogue.json')); print(len(d['applications']), 'applications')"

echo "--- nos icones ---"
# GrenPlace et l'explorateur portaient l'icone generique d'un theme : on leur
# en dessine une, aux couleurs du systeme.
mkdir -p config/includes.chroot/usr/share/icons/hicolor/256x256/apps
python3 tools/icones.py config/includes.chroot/usr/share/icons/hicolor/256x256/apps 256

echo "--- fonds d'écran et logo ---"
python3 tools/wallpaper.py config/includes.chroot/usr/share/grenos 2560 1440
# Le logo : le nom, dans la police du système, sur fond transparent. Il sert à
# l'écran de démarrage et à l'accueil.
convert -size 900x260 xc:none \
    -font /usr/share/fonts/truetype/noto/NotoSans-Bold.ttf -pointsize 150 \
    -fill '#e6ebf5' -gravity center -annotate +0+0 'grenOS' \
    PNG32:config/includes.chroot/usr/share/grenos/logo.png

# Le dépôt de mises à jour de grenOS. La source va dans config/archives avec
# le suffixe .binary : live-build ne l'ajoute qu'à l'image produite, et jamais
# aux dépôts qu'il consulte pour construire — sinon l'apt de la construction
# s'arrête sur un dépôt qui n'existe pas encore.
rm -f config/archives/grenos.list.binary config/archives/grenos.key.binary
if [ -f grenos-apt.gpg ] && [ -f grenos-apt.asc ]; then
    mkdir -p config/archives
    install -D -m 0644 grenos-apt.gpg \
        config/includes.chroot/usr/share/keyrings/grenos-apt.gpg
    cp grenos-apt.asc config/archives/grenos.key.binary
    cat > config/archives/grenos.list.binary <<'EOF'
deb [signed-by=/usr/share/keyrings/grenos-apt.gpg] https://tpqzhzuoyqpfairatdrw.supabase.co/storage/v1/object/public/apt stable main
EOF
    echo "depot grenOS ajoute a l'image"
else
    echo "pas de cle de depot : l'image n'aura que les depots Debian"
fi

echo "--- configuration ---"
lb clean --purge >/dev/null 2>&1 || true
lb config

echo "--- construction ---"
lb build 2>&1 | tail -n 60

ls -la *.iso
echo "--- taille ---"
du -h ./*.iso
