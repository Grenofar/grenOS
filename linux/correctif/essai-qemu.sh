#!/bin/bash
# Démarrer une clé (cle-fat.sh) comme le PC de Grenofar : UEFI, Secure Boot
# ACTIF avec les clés de Microsoft, clé USB sur un contrôleur USB 3. Captures
# d'écran à intervalles, puis arrêt PROPRE (bouton d'alimentation ACPI) pour
# que le rapport de démarrage écrive sa dernière page, puis on relit la clé.
#
#   bash linux/correctif/essai-qemu.sh <cle.img> <dossier de sortie> [secondes]
set -uo pipefail

IMG=$1
OUT=$2
DUREE=${3:-270}
ICI=$(cd "$(dirname "$0")" && pwd)
RACINE="$ICI/../.."
mkdir -p "$OUT"

CODE=/usr/share/OVMF/OVMF_CODE_4M.secboot.fd
VARS=/usr/share/OVMF/OVMF_VARS_4M.ms.fd
[ -f "$CODE" ] && [ -f "$VARS" ] || { echo "OVMF secboot introuvable"; ls /usr/share/OVMF; exit 1; }
cp "$VARS" "$OUT/vars.fd"
ACCEL=""
if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then ACCEL="-enable-kvm -cpu host"; fi

qemu-system-x86_64 $ACCEL -machine q35,smm=on -global driver=cfi.pflash01,property=secure,value=on \
    -m 4096 -smp 4 \
    -drive if=pflash,format=raw,readonly=on,file="$CODE" -drive if=pflash,format=raw,file="$OUT/vars.fd" \
    -device qemu-xhci,id=xhci \
    -drive if=none,id=cle,format=raw,file="$IMG" \
    -device usb-storage,bus=xhci.0,drive=cle,removable=on,bootindex=0 \
    -device usb-tablet,bus=xhci.0 \
    -vga std -display none \
    -serial file:"$OUT/serial.log" \
    -monitor unix:"$OUT/mon.sock",server,nowait &
QEMU=$!
sleep 3
if ! kill -0 "$QEMU" 2>/dev/null; then
    echo "QEMU n'a pas démarré : sa ligne de commande est fautive"
    exit 1
fi

t=0
for cap in 45 75 110 150 200 260; do
    [ "$cap" -le "$DUREE" ] || break
    # La souris BOUGE avant la capture de 75 s (21 novembre) : sur aucune
    # capture de l'assistant le curseur n'apparaissait — caché tant qu'on ne
    # bouge pas, ou absent ? La capture de 75 s le dit.
    if [ "$cap" = 75 ]; then
        sleep $((cap - t - 5))
        python3 "$RACINE/scripts/ci-screen.py" send "$OUT/mon.sock" "mouse_move 40 30" || true
        sleep 5
    else
        sleep $((cap - t))
    fi
    t=$cap
    kill -0 "$QEMU" 2>/dev/null || { echo "QEMU s'est arrêté seul à ~${t}s"; break; }
    python3 "$RACINE/scripts/ci-screen.py" grab "$OUT/mon.sock" "$OUT/ecran-${cap}s.ppm" || true
    python3 "$RACINE/scripts/ci-screen.py" judge "$OUT/ecran-${cap}s.ppm" "$OUT/ecran-${cap}s.png" > "$OUT/ecran-${cap}s.txt" 2>&1 || true
    echo "${cap} s : $(grep -m1 'screen:' "$OUT/ecran-${cap}s.txt" 2>/dev/null || echo 'pas de capture')"
    rm -f "$OUT/ecran-${cap}s.ppm"
done

# Le bouton d'alimentation, comme Grenofar l'appuierait : systemd éteint,
# et le rapport écrit sa page « arret ».
python3 "$RACINE/scripts/ci-screen.py" send "$OUT/mon.sock" "system_powerdown" || true
for _ in $(seq 1 40); do
    kill -0 "$QEMU" 2>/dev/null || break
    sleep 3
done
if kill -0 "$QEMU" 2>/dev/null; then
    echo "la machine ne s'est pas éteinte en 120 s : coupée"
    kill "$QEMU" 2>/dev/null || true
fi
sleep 2

export MTOOLS_SKIP_CHECK=1
if mcopy -s -n -i "$IMG@@1048576" ::/grenos-rapports "$OUT/" >/dev/null 2>&1; then
    echo "rapports relus sur la clé :"
    ls -1 "$OUT/grenos-rapports" | sed 's/^/  /'
else
    echo "AUCUN rapport sur la clé"
fi
rm -f "$OUT/vars.fd" "$OUT/mon.sock"
exit 0
