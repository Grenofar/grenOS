#!/bin/sh
# Les signatures de l'antivirus ne se téléchargent qu'à la demande (22 novembre).
#
#   sh linux/tools/essai-antivirus.sh
#
# Le disque installé de la 1719 lançait `clamav-freshclam-once` à chaque
# démarrage (journal du run 45053914956) : 40 s de retard sans réseau, des
# centaines de Mo avec, alors que la page Sécurité dit « elle se télécharge
# ici ». Debian (1.4.6, dh_installsystemd) arme TROIS unités ; nous n'en
# désarmions qu'une. La section d'appliquer-systeme est JOUÉE ici, avec un
# faux systemctl : rien de cette machine n'est lu ni changé.
set -eu
ICI=$(cd "$(dirname "$0")" && pwd)
I="$ICI/../config/includes.chroot"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
mkdir -p "$T/bin"
JOURNAL="$T/journal"
export JOURNAL
: > "$JOURNAL"
cat > "$T/bin/systemctl" <<'FAUX'
#!/bin/sh
echo "systemctl $*" >> "$JOURNAL"
FAUX
chmod +x "$T/bin/systemctl"
fautes=0
cas=0
juge() { # 0 ou 1, ce qui est juge
    cas=$((cas + 1))
    if [ "$1" = 0 ]; then echo "ok   $2"; else echo "FAUX $2"; fautes=$((fautes + 1)); fi
}

sed -n "/^# Les signatures de l'antivirus/,/^# L'écran de chargement reste/p" \
    "$I/usr/lib/grenos/appliquer-systeme" > "$T/section"
if [ -s "$T/section" ]; then
    PATH="$T/bin:$PATH" sh -e "$T/section"
else
    juge 1 "la section antivirus d appliquer-systeme est introuvable"
fi

UNITES="clamav-freshclam.service clamav-freshclam-once.service clamav-freshclam-once.timer"
for unite in $UNITES; do
    for geste in disable stop; do
        if grep -qxF "systemctl $geste $unite" "$JOURNAL"; then r=0; else r=1; fi
        juge "$r" "$geste $unite"
    done
done
if grep -qE '^systemctl (enable|start|unmask) ' "$JOURNAL"; then r=1; else r=0; fi
juge "$r" "rien n est rallume"
# « mask » casserait le bouton de qui veut les rallumer ; desarmer suffit :
# deb-systemd-helper respecte une unite desarmee aux mises a jour.
if grep -q '^systemctl mask ' "$JOURNAL"; then r=1; else r=0; fi
juge "$r" "rien n est interdit"

if grep -q 'arguments.push("freshclam")' "$ICI/../ui/src/reglages.rs"; then r=0; else r=1; fi
juge "$r" "« Telecharger les signatures » lance freshclam lui-meme, sans ces unites"
if grep -q 'clamav-freshclam\*' "$ICI/../config/hooks/normal/0100-grenos-look.hook.chroot"; then r=0; else r=1; fi
juge "$r" "la construction refuse une image qui en rearme un"

echo "antivirus : $cas cas, $fautes faute(s)"
[ "$fautes" -eq 0 ]
