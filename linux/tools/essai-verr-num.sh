#!/bin/sh
# grenos-verr-num, sans écran ni clavier (28 novembre).
#
#   sh linux/tools/essai-verr-num.sh
#
# Un faux `xset` dit l'état de Verr Num, un faux `xdotool` note chaque touche,
# un faux chassis_type dit fixe ou portable. On vérifie : allumé s'il était
# éteint, jamais éteint s'il était allumé, rien sur un portable, et une sortie
# en 0 quoi qu'il arrive — LightDM n'ouvrirait pas l'écran de connexion sinon.

set -u
ICI=$(cd "$(dirname "$0")" && pwd)
SCRIPT="$ICI/../config/includes.chroot/usr/lib/grenos/grenos-verr-num"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
mkdir -p "$T/bin"
cat > "$T/bin/xset" <<'EOF'
#!/bin/sh
printf '  00: Caps Lock:   off    01: Num Lock:    %s    02: Scroll Lock: off\n' "$ETAT"
EOF
cat > "$T/bin/xdotool" <<'EOF'
#!/bin/sh
echo "$*" >> "$JOURNAL"
exit "${CODE:-0}"
EOF
chmod +x "$T/bin/xset" "$T/bin/xdotool"

FAUTES=0
CAS=0
cas() {   # cas <chassis> <etat> <code xdotool> <touches attendues> <texte>
    CAS=$((CAS + 1))
    printf '%s\n' "$1" > "$T/chassis"
    : > "$T/journal"
    ETAT=$2 CODE=$3 JOURNAL="$T/journal" GRENOS_CHASSIS="$T/chassis" PATH="$T/bin:/usr/bin:/bin" \
        sh "$SCRIPT"
    sortie=$?
    touches=$(cat "$T/journal")
    if [ "$sortie" = 0 ] && [ "$touches" = "$4" ]; then
        echo "  [ok] $5"
    else
        echo "  [FAUTE] $5 (sortie $sortie, touches « $touches »)"
        FAUTES=$((FAUTES + 1))
    fi
}

echo "verr num :"
cas 3 off 0 "key Num_Lock" "PC fixe, Verr Num éteint : allumé"
cas 3 on 0 "" "PC fixe, déjà allumé : on n'y touche pas (une touche de trop l'éteindrait)"
cas 10 off 0 "" "portable : rien (pavé numérique dans les lettres)"
cas 31 off 0 "" "convertible : rien"
cas 3 off 1 "key Num_Lock" "xdotool en échec : sortie 0 quand même (LightDM)"
cas "" off 0 "key Num_Lock" "type de châssis illisible : traité comme un PC fixe"

CAS=$((CAS + 1))
if GRENOS_CHASSIS="$T/chassis" PATH="/nulle-part" /bin/sh "$SCRIPT"; then
    echo "  [ok] ni xset ni xdotool : sortie 0, rien"
else
    echo "  [FAUTE] ni xset ni xdotool : la sortie n'est pas 0"
    FAUTES=$((FAUTES + 1))
fi

if [ "$FAUTES" -gt 0 ]; then
    echo "verr num : $FAUTES faute(s) sur $CAS cas"
    exit 1
fi
echo "verr num : $CAS cas, 0 faute"
