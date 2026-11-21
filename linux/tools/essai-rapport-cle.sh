#!/bin/sh
# Essai du rapport de démarrage écrit sur la clé, face à un écran FIGÉ
# (21 novembre).
#
# Écran vert de Grenofar à 18 h 50 : aucun rapport sur la clé. Le filet de
# l'écran avait redémarré la machine avant le premier instantané, et celui de
# l'arrêt interrogeait X et l'écran figés — il attendait, systemd l'a tué.
# Ici, une fausse clé, un X qui ne répond plus (xdpyinfo ignore SIGTERM) et
# un lspci coincé ; on vérifie que :
#   - le PREMIER TEMPS (résumé, journal…) est sur la clé AVANT qu'on parle
#     à X (xdpyinfo le constate lui-même) ;
#   - les commandes coincées sont abandonnées, et le rapport arrive au bout
#     en un temps borné ;
#   - le résumé porte le lien PCIe.
#
#   sh linux/tools/essai-rapport-cle.sh
set -eu
ICI=$(cd "$(dirname "$0")" && pwd)
S="$ICI/../config/includes.chroot/usr/lib/grenos/grenos-rapport-cle"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
fautes=0
faute() { echo "FAUX $*"; fautes=$((fautes + 1)); }
bon() { echo "ok   $*"; }

mkdir -p "$T/cle/live" "$T/run" "$T/bin"
echo "BOOT_IMAGE=/live/vmlinuz boot=live quiet splash" > "$T/ligne"

# La clé est en FAT32, montée en écriture : pas de remontage à faire.
cat > "$T/bin/findmnt" <<'EOF'
#!/bin/sh
case "$*" in
    *FSTYPE*) echo vfat ;;
    *OPTIONS*) echo rw,noatime ;;
esac
EOF
# flock n'existe pas partout où l'essai tourne ; le verrou n'est pas l'objet.
printf '#!/bin/sh\nexit 0\n' > "$T/bin/flock"
# X figé : xdpyinfo ne répond pas et ignore SIGTERM. Il note d'abord si le
# premier temps était déjà sur la clé quand on l'a appelé.
cat > "$T/bin/xdpyinfo" <<'EOF'
#!/bin/sh
if ls "$GRENOS_CLE"/grenos-rapports/*/00-resume.txt >/dev/null 2>&1; then
    echo oui > "$GRENOS_CLE/../premier-temps-avant-x"
else
    echo non > "$GRENOS_CLE/../premier-temps-avant-x"
fi
trap '' TERM INT
sleep 30
EOF
# lspci répond pendant le premier temps (la ligne du son), puis se coince
# une fois le premier temps écrit : le second temps doit l'abandonner.
cat > "$T/bin/lspci" <<'EOF'
#!/bin/sh
if ls "$GRENOS_CLE"/grenos-rapports/*/00-resume.txt >/dev/null 2>&1; then
    trap '' TERM INT
    sleep 60
fi
echo "00:02.0 VGA compatible controller [0300]: carte factice"
EOF
chmod +x "$T/bin/"*

debut=$(date +%s)
GRENOS_CHEMIN_ESSAI="$T/bin" GRENOS_CLE="$T/cle" GRENOS_RUN="$T/run" GRENOS_LIGNE="$T/ligne" \
    ${GRENOS_SH:-sh} "$S" maintenant
duree=$(( $(date +%s) - debut ))

d=$(ls -d "$T/cle/grenos-rapports"/*/ 2>/dev/null | head -1)
if [ -n "$d" ] && [ -s "$d/00-resume.txt" ]; then
    bon "le rapport est sur la clé ($(basename "$d"))"
else
    faute "aucun rapport sur la clé"
    d=$T/vide
fi

case "$(cat "$T/premier-temps-avant-x" 2>/dev/null)" in
    oui) bon "le premier temps était sur la clé avant qu'on interroge X" ;;
    non) faute "X a été interrogé avant que le premier temps soit écrit" ;;
    *) faute "X n'a pas été interrogé du tout" ;;
esac

if grep -q 'ne répond plus' "$d/50-ecrans.txt" 2>/dev/null; then
    bon "X figé : dit tel quel, sans essayer toutes les autorisations"
else
    faute "X figé non signalé dans 50-ecrans.txt"
fi

if grep -q 'abandonnée après 10 s' "$d/50-ecrans.txt" 2>/dev/null; then
    bon "lspci coincé : abandonné après 10 s"
else
    faute "lspci coincé non abandonné (50-ecrans.txt)"
fi

if grep -q '== lien PCIe' "$d/00-resume.txt" 2>/dev/null; then
    bon "le résumé porte le lien PCIe"
else
    faute "le lien PCIe manque au résumé"
fi

# X (5 s au plus) + lspci (10 s) + le reste. Si `borne` attendait la fin de
# chaque commande déjà finie (zombie pris pour vivant), on dépasserait 90 s.
if [ "$duree" -le 45 ]; then
    bon "rapport complet en $duree s"
else
    faute "rapport trop long : $duree s"
fi

echo "rapport-cle : $fautes faute(s)"
[ "$fautes" = 0 ]
