#!/bin/sh
# Essai de grenos-installer sur une machine factice (21 novembre).
#   sh linux/tools/essai-installer.sh
#
# La machine de Grenofar, telle que la session de la clé la voit : la clé
# grenOS (sdc, montée par live-boot), le SSD Samsung (sda) qu'il a ouvert dans
# « Ce PC », le disque dur (sdb) fermé, et Windows (nvme0n1p3) monté en
# lecture seule par grenos-windows. Le lanceur doit refermer le SSD et
# Windows, ne JAMAIS toucher à la clé, puis ouvrir Calamares.
set -eu
ICI=$(cd "$(dirname "$0")" && pwd)
S="$ICI/../config/includes.chroot/usr/bin/grenos-installer"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
mkdir -p "$T/bin"
JOURNAL="$T/journal"
: > "$JOURNAL"

cat > "$T/bin/findmnt" <<'EOF'
#!/bin/sh
[ "$*" = "-rno SOURCE /run/live/medium" ] && echo /dev/sdc1
exit 0
EOF
cat > "$T/bin/lsblk" <<'EOF'
#!/bin/sh
case "$*" in
    "-rno PKNAME /dev/sdc1") echo sdc ;;
    "-rno PATH,PKNAME,TYPE,MOUNTPOINT")
        echo "/dev/sda  disk "
        echo "/dev/sda1 sda part /media/grenos/Disque\\x20ssd\\x20256gb"
        echo "/dev/sdb  disk "
        echo "/dev/sdb1 sdb part "
        echo "/dev/sdc  disk "
        echo "/dev/sdc1 sdc part /run/live/medium"
        echo "/dev/nvme0n1  disk "
        echo "/dev/nvme0n1p1 nvme0n1 part "
        echo "/dev/nvme0n1p3 nvme0n1 part /media/windows"
        ;;
esac
EOF
cat > "$T/bin/systemctl" <<EOF
#!/bin/sh
echo "systemctl \$*" >> "$JOURNAL"
exit 0
EOF
cat > "$T/bin/udisksctl" <<EOF
#!/bin/sh
echo "udisksctl \$*" >> "$JOURNAL"
exit 0
EOF
cat > "$T/bin/pkexec" <<EOF
#!/bin/sh
echo "pkexec \$*" >> "$JOURNAL"
exit 0
EOF
printf '#!/bin/sh\nexit 0\n' > "$T/bin/grenos-dire"
chmod +x "$T"/bin/*

PATH="$T/bin:$PATH" sh "$S" 2>/dev/null

fautes=0
attendu() { # motif, titre
    if grep -q -- "$1" "$JOURNAL"; then echo "ok   $2"; else echo "FAUX $2"; fautes=$((fautes + 1)); fi
}
jamais() { # motif, titre
    if grep -q -- "$1" "$JOURNAL"; then echo "FAUX $2"; fautes=$((fautes + 1)); else echo "ok   $2"; fi
}
attendu "systemctl stop grenos-windows.service" "Windows (monte par grenos-windows) : service arrete, il referme"
attendu "udisksctl unmount -b /dev/sda1" "le SSD ouvert dans Ce PC est referme"
attendu "udisksctl unmount -b /dev/nvme0n1p3" "Windows est referme meme s il reste monte"
jamais  "/dev/sdc1" "la cle grenOS n est jamais touchee"
jamais  "/dev/sdb1" "un disque deja ferme n est pas touche"
attendu "pkexec /usr/bin/calamares" "puis l installateur s ouvre (pkexec calamares)"
# La clé cachée à l'installateur AVANT qu'il s'ouvre, rendue APRÈS (21
# novembre : Calamares l'avait prise pour partition EFI, grub-install code 1).
ordre=$(grep -n -E 'grenos-cle-masquee|pkexec' "$JOURNAL" | cut -d: -f2- | tr '\n' '|')
if [ "$ordre" = "systemctl start grenos-cle-masquee.service|pkexec /usr/bin/calamares|systemctl stop grenos-cle-masquee.service|" ]; then
    echo "ok   la cle est cachee avant l installateur et rendue apres"
else
    echo "FAUX la cle doit etre cachee avant l installateur et rendue apres (vu : $ordre)"; fautes=$((fautes + 1))
fi
if grep -n 'udisksctl\|grenos-windows' "$JOURNAL" | tail -n 1 | cut -d: -f1 | \
        { read -r dernier; premier=$(grep -n pkexec "$JOURNAL" | cut -d: -f1); [ "$dernier" -lt "$premier" ]; }; then
    echo "ok   l installateur s ouvre apres les disques liberes"
else
    echo "FAUX l installateur doit s ouvrir APRES que les disques sont liberes"; fautes=$((fautes + 1))
fi
[ "$fautes" -eq 0 ] && echo "installer : 8 cas justes" || { echo "installer : $fautes faute(s)"; cat "$JOURNAL"; exit 1; }
