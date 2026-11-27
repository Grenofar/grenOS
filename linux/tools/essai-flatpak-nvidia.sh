#!/bin/sh
# grenos-flatpak-nvidia, sur un faux flatpak et un faux dpkg (27 novembre).
#
#   sh linux/tools/essai-flatpak-nvidia.sh
#
# Rocket League (Heroic, en Flatpak) plantait sur la GT 1030 de Grenofar : le
# pilote NVIDIA tournait, mais l'extension Flatpak de sa version manquait.
# Chaque cas ci-dessous est une situation qu'une machine rencontre vraiment.
set -u
ICI=$(cd "$(dirname "$0")" && pwd)
SCRIPT="$ICI/../config/includes.chroot/usr/lib/grenos/grenos-flatpak-nvidia"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
FAUTES=0
ok() { echo "  [ok] $*"; }
faute() { echo "  [FAUTE] $*"; FAUTES=$((FAUTES + 1)); }

mkdir -p "$T/bin"
cat > "$T/bin/flatpak" <<'EOF'
#!/bin/sh
E="$ETAT"
case "$1" in
    list)
        case "$*" in *--app*) cat "$E/apps" 2>/dev/null ;; *) cat "$E/runtimes" 2>/dev/null ;; esac ;;
    info) shift; [ "$1" = --system ] && shift; grep -qx "$1" "$E/runtimes" 2>/dev/null ;;
    remote-add) exit 0 ;;
    remote-ls) [ -e "$E/hors-ligne" ] && exit 1; cat "$E/flathub" ;;
    install)
        [ -e "$E/hors-ligne" ] && exit 1
        for a in "$@"; do case "$a" in org.*) echo "$a" >> "$E/runtimes"; echo "pose $a" >> "$E/journal" ;; esac; done ;;
    uninstall)
        for a in "$@"; do case "$a" in org.*) grep -vx "$a" "$E/runtimes" > "$E/r"; mv "$E/r" "$E/runtimes"; echo "retire $a" >> "$E/journal" ;; esac; done ;;
esac
EOF
cat > "$T/bin/dpkg-query" <<'EOF'
#!/bin/sh
cat "$ETAT/dpkg" 2>/dev/null
EOF
chmod 0755 "$T/bin/flatpak" "$T/bin/dpkg-query"

neuf() { # nouvelle machine
    rm -rf "$T/e" "$T/sys"; mkdir -p "$T/e" "$T/sys/module"
    printf '%s\n' org.freedesktop.Platform.GL.default org.freedesktop.Platform.GL32.default > "$T/e/runtimes"
    printf '%s\n' org.freedesktop.Platform.GL.nvidia-580-178-04 org.freedesktop.Platform.GL32.nvidia-580-178-04 \
        org.freedesktop.Platform.GL.nvidia-580-190-01 org.freedesktop.Platform.GL32.nvidia-580-190-01 \
        org.freedesktop.Platform.GL.default > "$T/e/flathub"
}
module() { mkdir -p "$T/sys/module/nvidia"; echo "$1" > "$T/sys/module/nvidia/version"; }
lancer() { ETAT="$T/e" GRENOS_ESSAI_BIN="$T/bin" GRENOS_SYSFS="$T/sys" sh "$SCRIPT" > "$T/sortie" 2>&1; echo $?; }
a() { grep -qx "$1" "$T/e/runtimes"; }

echo "flatpak nvidia :"
neuf; c=$(lancer)
[ "$c" = 0 ] && ! grep -q nvidia "$T/e/runtimes" && ok "sans pilote NVIDIA : rien" || faute "sans pilote NVIDIA (code $c)"

neuf; module 580.178.04; c=$(lancer)
[ "$c" = 0 ] && ! grep -q nvidia "$T/e/runtimes" && ok "pilote mais aucune application Flatpak : rien (Flatpak la posera avec la premiere)" || faute "sans application (code $c)"

neuf; module 580.178.04; echo com.heroicgameslauncher.hgl > "$T/e/apps"; c=$(lancer)
[ "$c" = 0 ] && a org.freedesktop.Platform.GL.nvidia-580-178-04 && a org.freedesktop.Platform.GL32.nvidia-580-178-04 \
    && ok "la GT 1030 de Grenofar : GL et GL32 de 580.178.04 poses" || faute "pose (code $c) : $(cat "$T/sortie")"
: > "$T/e/journal"; c=$(lancer)
[ "$c" = 0 ] && [ ! -s "$T/e/journal" ] && ok "relance : rien a faire, rien touche" || faute "relance : $(cat "$T/e/journal")"

module 580.190.01; c=$(lancer)
[ "$c" = 0 ] && a org.freedesktop.Platform.GL.nvidia-580-190-01 && a org.freedesktop.Platform.GL32.nvidia-580-190-01 \
    && ! a org.freedesktop.Platform.GL.nvidia-580-178-04 && ! a org.freedesktop.Platform.GL32.nvidia-580-178-04 \
    && ok "pilote mis a jour : les extensions suivent, les anciennes partent" || faute "mise a jour du pilote : $(cat "$T/e/runtimes" | tr '\n' ' ')"
a org.freedesktop.Platform.GL.default && a org.freedesktop.Platform.GL32.default \
    && ok "les extensions de Mesa ne sont jamais touchees" || faute "Mesa retire"

neuf; module 580.178.04; echo com.valvesoftware.Steam > "$T/e/apps"; touch "$T/e/hors-ligne"; c=$(lancer)
[ "$c" = 1 ] && ok "Flathub injoignable : code 1, le service reessaiera" || faute "hors ligne (code $c)"

neuf; module 595.10.01; echo com.valvesoftware.Steam > "$T/e/apps"; c=$(lancer)
[ "$c" = 0 ] && grep -q "pas (encore) sur Flathub" "$T/sortie" && ok "version pas encore sur Flathub : dit, sans reessayer en boucle" || faute "absente de Flathub (code $c)"

neuf; echo com.valvesoftware.Steam > "$T/e/apps"
echo "ii  580.178.04-0ubuntu0.26.04.1" > "$T/e/dpkg"; c=$(lancer)
[ "$c" = 0 ] && a org.freedesktop.Platform.GL.nvidia-580-178-04 \
    && ok "juste apres « Preparer le materiel » (module pas encore charge) : la version des bibliotheques" || faute "avant redemarrage (code $c)"

neuf; module "580.178.04; rm -rf /"; echo com.valvesoftware.Steam > "$T/e/apps"; c=$(lancer)
[ "$c" = 0 ] && ! grep -q nvidia "$T/e/runtimes" && ok "version illisible : rien" || faute "version illisible (code $c)"

if [ "$FAUTES" -gt 0 ]; then
    echo "flatpak nvidia : $FAUTES faute(s)"
    exit 1
fi
echo "flatpak nvidia : 10 cas, 0 faute"
