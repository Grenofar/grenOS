#!/bin/sh
# Fabrique le paquet grenos-desktop : ce qui fait grenOS par-dessus Debian —
# les fonds d'écran, les outils grenos-*, l'accueil et ses entrées de menu.
#
# C'est ce paquet que `grenos-maj` met à jour depuis l'OS : les correctifs de
# sécurité viennent de Debian, notre visage vient de là.
#
#   linux/tools/make-deb.sh <version> <dossier de sortie>

set -eu

VERSION="${1:-1.0.0}"
OUT="${2:-dist}"
HERE=$(cd "$(dirname "$0")/.." && pwd)
BUILD=$(mktemp -d)
trap 'rm -rf "$BUILD"' EXIT

mkdir -p "$BUILD/DEBIAN" "$BUILD/usr/bin" "$BUILD/usr/share/grenos" \
         "$BUILD/usr/share/applications" "$BUILD/usr/share/wallpapers"

# Les fichiers, exactement ceux que l'image embarque.
install -m 0755 "$HERE/config/includes.chroot/usr/bin/grenos-theme" "$BUILD/usr/bin/"
install -m 0755 "$HERE/config/includes.chroot/usr/bin/grenos-maj" "$BUILD/usr/bin/"
install -m 0644 "$HERE/config/includes.chroot/usr/share/grenos/bienvenue.html" "$BUILD/usr/share/grenos/"
install -m 0644 "$HERE"/config/includes.chroot/usr/share/applications/grenos-*.desktop \
    "$BUILD/usr/share/applications/"

# Les fonds d'écran, calculés ici comme à la construction de l'image.
for moment in nuit jour; do
    mkdir -p "$BUILD/usr/share/wallpapers/grenOS$( [ "$moment" = jour ] && echo -Jour )/contents/images"
done
python3 "$HERE/tools/wallpaper.py" "$BUILD/usr/share/grenos" 2560 1440
mv "$BUILD/usr/share/grenos/grenos-nuit.png" \
   "$BUILD/usr/share/wallpapers/grenOS/contents/images/2560x1440.png"
mv "$BUILD/usr/share/grenos/grenos-jour.png" \
   "$BUILD/usr/share/wallpapers/grenOS-Jour/contents/images/2560x1440.png"
for name in grenOS grenOS-Jour; do
    moment=Nuit
    [ "$name" = grenOS-Jour ] && moment=Jour
    cat > "$BUILD/usr/share/wallpapers/$name/metadata.json" <<EOF
{
    "KPlugin": {
        "Authors": [{"Name": "grenOS"}],
        "Id": "$name",
        "License": "CC-BY-SA-4.0",
        "Name": "grenOS $moment"
    }
}
EOF
done

SIZE=$(du -ks "$BUILD" | cut -f1)
cat > "$BUILD/DEBIAN/control" <<EOF
Package: grenos-desktop
Version: $VERSION
Section: x11
Priority: optional
Architecture: all
Maintainer: grenOS <grenos@grenos-dev.vercel.app>
Depends: plasma-desktop, python3, xdg-utils
Recommends: plasma-discover, flatpak
Installed-Size: $SIZE
Homepage: https://grenos-dev.vercel.app
Description: Le visage de grenOS sur un bureau Plasma
 Les fonds d'écran de grenOS, la bascule jour et nuit, la fenêtre de mise à
 jour et la page d'accueil. Ce paquet est ce que grenOS ajoute à Debian ;
 il se met à jour depuis le système comme n'importe quel autre.
EOF

mkdir -p "$OUT"
DEB="$OUT/grenos-desktop_${VERSION}_all.deb"
dpkg-deb --build --root-owner-group "$BUILD" "$DEB"
dpkg-deb --info "$DEB" | head -12
echo "$DEB"
