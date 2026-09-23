#!/bin/sh
# Fabrique le paquet grenos-desktop : le bureau de grenOS, celui qui est écrit
# dans ce dépôt — la barre, le bureau, les réglages, GrenPlace, l'accueil, le
# thème des fenêtres et les fonds d'écran.
#
# C'est ce paquet que `grenos-maj` met à jour depuis l'OS : les correctifs de
# sécurité viennent de Debian, notre visage vient de là. Il contient exactement
# les mêmes fichiers que l'image, pris au même endroit — sans quoi une machine
# mise à jour ne ressemblerait plus à une machine fraîchement installée.
#
#   linux/tools/make-deb.sh <version> <dossier de sortie>

set -eu

VERSION="${1:-1.0.0}"
OUT="${2:-dist}"
HERE=$(cd "$(dirname "$0")/.." && pwd)
DEDANS="$HERE/config/includes.chroot"
BUILD=$(mktemp -d)
trap 'rm -rf "$BUILD"' EXIT

mkdir -p "$BUILD/DEBIAN" \
         "$BUILD/usr/bin" \
         "$BUILD/usr/lib/grenos" \
         "$BUILD/usr/share/grenos" \
         "$BUILD/usr/share/xsessions" \
         "$BUILD/usr/share/themes/grenOS/openbox-3" \
         "$BUILD/etc/xdg/openbox"

# Les programmes du bureau, tels quels. `grenos-premier` n'en fait pas partie :
# il ne sert qu'au tout premier démarrage d'une image, et le réinstaller sur
# une machine déjà nommée n'aurait aucun sens.
for outil in grenos-shell grenos-session grenos-menu grenos-fond grenos-veilleur \
             grenos-arret grenos-parametres grenplace grenos-bienvenue grenos-bureau \
             grenos-theme grenos-maj; do
    install -m 0755 "$DEDANS/usr/bin/$outil" "$BUILD/usr/bin/"
done

for module in grenosui ecran son materiel; do
    install -m 0644 "$DEDANS/usr/lib/grenos/$module.py" "$BUILD/usr/lib/grenos/"
done
install -m 0755 "$DEDANS/usr/lib/grenos/grenos-compte" "$BUILD/usr/lib/grenos/"

install -m 0644 "$DEDANS/usr/share/xsessions/grenos.desktop" "$BUILD/usr/share/xsessions/"
install -m 0644 "$DEDANS/usr/share/themes/grenOS/openbox-3/themerc" \
    "$BUILD/usr/share/themes/grenOS/openbox-3/"
install -m 0644 "$DEDANS/etc/xdg/openbox/rc.xml" "$BUILD/etc/xdg/openbox/"

# Les fonds d'écran, calculés ici comme à la construction de l'image.
python3 "$HERE/tools/wallpaper.py" "$BUILD/usr/share/grenos" 2560 1440

SIZE=$(du -ks "$BUILD" | cut -f1)
cat > "$BUILD/DEBIAN/control" <<EOF
Package: grenos-desktop
Version: $VERSION
Section: x11
Priority: optional
Architecture: all
Maintainer: grenOS <grenos@grenos-dev.vercel.app>
Depends: python3, python3-gi, gir1.2-gtk-3.0, gir1.2-wnck-3.0, openbox, feh, x11-utils, x11-xserver-utils, xdg-utils
Recommends: flatpak, lightdm, pcmanfm, xfce4-terminal, xfce4-taskmanager
Installed-Size: $SIZE
Homepage: https://grenos-dev.vercel.app
Description: Le bureau de grenOS
 La barre, le bureau et ses icônes, le menu, les Réglages, GrenPlace, la
 bascule jour et nuit, le thème des fenêtres et les fonds d'écran. Tout ce qui
 se voit dans grenOS est ici ; Debian fournit le noyau, les pilotes et les
 applications. Ce paquet se met à jour depuis le système comme un autre.
EOF

mkdir -p "$OUT"
DEB="$OUT/grenos-desktop_${VERSION}_all.deb"
dpkg-deb --build --root-owner-group "$BUILD" "$DEB"
dpkg-deb --info "$DEB" | head -12
dpkg-deb --contents "$DEB" | awk '{print $6}' | sort
echo "$DEB"
