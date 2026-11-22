#!/bin/sh
# Essai du démarrage du grenOS installé, sur une machine factice (22 novembre).
#
#   sh linux/tools/essai-demarrage.sh
#
# Les quatre plaintes de Grenofar après l'installation de la 1719 :
#   - le lobby (OOBE) paraît 3 s puis un chargement sans fin le recouvre :
#     Plymouth doit être PARTI avant que son serveur X démarre ;
#   - « lobby seulement sur grenOS installé » : jamais sur la clé ;
#   - « on voit debian » dans les options du BIOS : l'entrée s'appelle grenOS,
#     sans jamais toucher à Windows ni au Debian d'un autre disque ;
#   - « affiche plusieurs trucs choix, enlève » : aucun menu, aucune ligne.
# Rien de cette machine n'est lu ni écrit : tout passe par des outils factices.
set -eu
ICI=$(cd "$(dirname "$0")" && pwd)
I="$ICI/../config/includes.chroot"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
mkdir -p "$T/bin" "$T/efivars" "$T/sys/sda1"
JOURNAL="$T/journal"
export ETAT="$T/etat" JOURNAL
fautes=0
cas=0
ok() { cas=$((cas + 1)); echo "ok   $1"; }
faux() { cas=$((cas + 1)); fautes=$((fautes + 1)); echo "FAUX $1"; }
attendu() { if grep -qF -- "$1" "$JOURNAL"; then ok "$2"; else faux "$2"; fi; }
jamais() { if grep -qF -- "$1" "$JOURNAL"; then faux "$2"; else ok "$2"; fi; }

# ------------------------------------------------------- les outils factices
cat > "$T/bin/plymouth" <<'EOF'
#!/bin/sh
echo "plymouth $*" >> "$JOURNAL"
case "$1" in
    --ping) [ -e "$ETAT.plymouth" ] ;;
    quit) [ -e "$ETAT.plymouth-sourd" ] && exit 1; rm -f "$ETAT.plymouth" ;;
esac
EOF
cat > "$T/bin/pkill" <<'EOF'
#!/bin/sh
echo "pkill $*" >> "$JOURNAL"
rm -f "$ETAT.plymouth" "$ETAT.plymouth-sourd"
EOF
cat > "$T/bin/pgrep" <<'EOF'
#!/bin/sh
case "$*" in
    *Xorg*) [ -e "$ETAT.xorg" ] ;;
    *) [ -e "$ETAT.plymouth" ] ;;
esac
EOF
cat > "$T/bin/python3" <<'EOF'
#!/bin/sh
case "$*" in
    *0x4B3A*) if [ -e "$ETAT.plymouth" ]; then echo "texte TROP TOT" >> "$JOURNAL"; else echo "terminal en texte" >> "$JOURNAL"; fi ;;
esac
EOF
cat > "$T/bin/xinit" <<'EOF'
#!/bin/sh
if [ -e "$ETAT.plymouth" ]; then echo "xinit AVEC plymouth vivant" >> "$JOURNAL"; else echo "xinit sans plymouth" >> "$JOURNAL"; fi
EOF
printf '#!/bin/sh\nexit 0\n' > "$T/bin/Xorg"

# efibootmgr 18 : « BootNNNN* nom<TAB>chemin ». L'état tient dans un fichier.
cat > "$T/bin/efibootmgr" <<'EOF'
#!/bin/sh
echo "efibootmgr $*" >> "$JOURNAL"
n="" b="" efface="" cree="" disque="" partie="" nom="" charge=""
while [ $# -gt 0 ]; do
    case "$1" in
        -n) n=$2; shift ;;
        -b) b=$2; shift ;;
        -B) efface=1 ;;
        -c) cree=1 ;;
        -d) disque=$2; shift ;;
        -p) partie=$2; shift ;;
        -L) nom=$2; shift ;;
        -l) charge=$2; shift ;;
    esac
    shift
done
if [ -n "$n" ]; then echo "$n" > "$ETAT.suivant"; exit 0; fi
if [ -n "$b" ] && [ -n "$efface" ]; then
    grep -v "^$b|" "$ETAT" > "$ETAT.x" || true
    mv "$ETAT.x" "$ETAT"
    exit 0
fi
if [ -n "$cree" ]; then
    [ -e "$ETAT.refus" ] && exit 5
    printf '%s|%s|HD(%s,GPT,aaaaaaaa-0000-4000-8000-000000000001,0x800,0x100000)/%s\n' 0009 "$nom" "$partie" "$charge" >> "$ETAT"
    exit 0
fi
echo "BootCurrent: 0002"
[ -e "$ETAT.suivant" ] && echo "BootNext: $(cat "$ETAT.suivant")"
echo "BootOrder: $(cut -d'|' -f1 "$ETAT" | tr '\n' ',' | sed 's/,$//')"
while IFS='|' read -r num lab chemin; do
    printf 'Boot%s* %s\t%s\n' "$num" "$lab" "$chemin"
done < "$ETAT"
exit 0
EOF
cat > "$T/bin/findmnt" <<'EOF'
#!/bin/sh
echo /dev/sda1
EOF
cat > "$T/bin/lsblk" <<'EOF'
#!/bin/sh
case "$*" in
    *PARTTYPE*) echo e3c9e316-0b5c-4db8-817d-f92df00215ae ;;
    *PKNAME*) echo sda ;;
    *PARTUUID*/dev/sda1*) echo aaaaaaaa-0000-4000-8000-000000000001 ;;
    *PARTUUID*)
        [ -e "$ETAT.lsblk-muet" ] && exit 0
        echo
        echo aaaaaaaa-0000-4000-8000-000000000001
        echo bbbbbbbb-0000-4000-8000-000000000002
        echo cccccccc-0000-4000-8000-000000000003
        ;;
esac
EOF
printf '#!/bin/sh\nexec sh "%s" "$@"\n' "$I/usr/lib/grenos/grenos-ecran-libre" > "$T/bin/ecran-libre"
chmod +x "$T"/bin/*
echo 1 > "$T/sys/sda1/partition"
PATH="$T/bin:$PATH"
export PATH

# ------------------------------------------------------------------ le lobby
echo "quiet splash" > "$T/ligne-installe"
echo "BOOT_IMAGE=/live/vmlinuz boot=live components quiet splash" > "$T/ligne-cle"
echo "Standard PC" > "$T/pc"
echo "grenos-essai-charge" > "$T/pc-essai"
lobby() { # ligne, machine
    GRENOS_LIGNE="$1" GRENOS_DMI="$2" GRENOS_IDENTITE="$T/identite" GRENOS_ECRAN_LIBRE="$T/bin/ecran-libre" \
        sh "$I/usr/bin/grenos-premier-x" > /dev/null 2>&1 || true
}

: > "$JOURNAL"; : > "$ETAT.plymouth"
lobby "$T/ligne-installe" "$T/pc"
attendu "xinit sans plymouth" "installe : Plymouth est parti AVANT le lobby (sinon il le recouvre a 8 s)"
jamais "xinit AVEC" "installe : jamais deux programmes sur l ecran"
jamais "--retain-splash" "installe : pas de --retain-splash (il a fige X le 21 novembre)"
jamais "plymouth deactivate" "installe : pas de deactivate (il a fige X le 21 novembre)"

: > "$JOURNAL"; : > "$ETAT.plymouth"; : > "$ETAT.plymouth-sourd"
lobby "$T/ligne-installe" "$T/pc"
attendu "xinit sans plymouth" "installe : un Plymouth qui n obeit pas est arrete, le lobby s ouvre"

: > "$JOURNAL"; rm -f "$ETAT.plymouth" "$ETAT.plymouth-sourd"
lobby "$T/ligne-installe" "$T/pc"
attendu "xinit sans plymouth" "installe : sans Plymouth, le lobby s ouvre quand meme"

: > "$JOURNAL"; : > "$ETAT.plymouth"
lobby "$T/ligne-cle" "$T/pc"
jamais "xinit" "cle : pas de lobby"
jamais "plymouth quit" "cle : Plymouth reste a LightDM"

: > "$JOURNAL"
lobby "$T/ligne-cle" "$T/pc-essai"
attendu "xinit" "machine d integration : le lobby reste jugeable sur l image"

# Plymouth parti laisse le terminal en KD_GRAPHICS + VT_AUTO : le noyau y
# IGNORE tout changement de terminal, et X attend sans fin (45067830100).
: > "$JOURNAL"; : > "$ETAT.plymouth"
lobby "$T/ligne-installe" "$T/pc"
if [ "$(grep -n -e 'terminal en texte' -e '^xinit' "$JOURNAL" | head -n 1 | grep -c 'terminal en texte')" = 1 ]; then
    ok "installe : le terminal repasse en texte avant X"
else
    faux "installe : le terminal repasse en texte avant X"
fi
jamais "TROP TOT" "installe : jamais avant la mort de Plymouth"
: > "$JOURNAL"; rm -f "$ETAT.plymouth"
sh "$I/usr/lib/grenos/grenos-ecran-libre" > /dev/null 2>&1 || true
attendu "terminal en texte" "demarrages : meme si Plymouth etait deja parti"
: > "$JOURNAL"; : > "$ETAT.xorg"
sh "$I/usr/lib/grenos/grenos-ecran-libre" > /dev/null 2>&1 || true
jamais "terminal en texte" "un serveur X a l ecran : son terminal n est pas touche"
rm -f "$ETAT.xorg"

# Le demarrage SUIVANT (run 45053914956) : lightdm aussi lance X pendant que
# Plymouth meurt. Il passe donc par le meme geste.
if grep -qx 'ExecStartPre=-/usr/lib/grenos/grenos-ecran-libre' "$I/etc/systemd/system/lightdm.service.d/20-grenos-ecran-libre.conf" 2>/dev/null; then
    ok "demarrages suivants : lightdm attend la mort de Plymouth"
else
    faux "demarrages suivants : lightdm attend la mort de Plymouth"
fi

# ------------------------------------------------------------ le nom au BIOS
A=aaaaaaaa-0000-4000-8000-000000000001   # notre partition EFI
B=bbbbbbbb-0000-4000-8000-000000000002   # Windows
C=cccccccc-0000-4000-8000-000000000003   # un autre disque, branche
PARTI=dddddddd-0000-4000-8000-000000000004
CH='\EFI\debian\shimx64.efi'
MS='\EFI\Microsoft\Boot\bootmgfw.efi'
entree() { printf '%s|%s|HD(1,GPT,%s,0x800,0x100000)/%s\n' "$1" "$2" "$3" "$4" >> "$ETAT"; }
depart() {
    : > "$ETAT"; : > "$JOURNAL"
    rm -f "$ETAT.suivant" "$ETAT.refus" "$ETAT.lsblk-muet"
    entree 0002 debian "$A" "$CH"
    entree 0000 "Windows Boot Manager" "$B" "$MS"
}
demarrage() {
    GRENOS_EFIVARS="$T/efivars" GRENOS_ESP="$T/esp" GRENOS_SYSBLOCK="$T/sys" GRENOS_MEMOIRE="$T/memoire" \
        sh "$I/usr/lib/grenos/grenos-demarrage" "$@" > "$T/sortie" 2>&1
}
porte() { if grep -qF -- "$1" "$ETAT"; then ok "$2"; else faux "$2"; fi; }
sans() { if grep -qF -- "$1" "$ETAT"; then faux "$2"; else ok "$2"; fi; }
mkdir -p "$T/esp/EFI/debian"
echo shim > "$T/esp/EFI/debian/shimx64.efi"

depart
demarrage nommer || true
porte "|grenOS|" "BIOS : l entree s appelle grenOS"
sans "|debian|" "BIOS : plus d entree debian pour ce disque"
attendu "-d /dev/sda -p 1 -L grenOS" "BIOS : creee sur le bon disque, la bonne partition"
porte "|grenOS|HD(1,GPT,$A,0x800,0x100000)/$CH" "BIOS : le dossier reste EFI/debian (prefixe du GRUB signe)"
porte "0000|Windows Boot Manager|" "BIOS : Windows intact"
if [ "$(grep -n -e ' -c ' -e ' -B' "$JOURNAL" | head -n 1 | grep -c -e ' -c ')" = 1 ]; then
    ok "BIOS : creee AVANT d effacer l ancienne"
else
    faux "BIOS : creee AVANT d effacer l ancienne"
fi

: > "$JOURNAL"
demarrage nommer || true
jamais " -c " "BIOS : rejoue, il ne recree rien"
jamais " -B" "BIOS : rejoue, il n efface rien"

depart
entree 0003 debian "$C" "$CH"
demarrage nommer nettoyer || true
porte "0003|debian|" "BIOS : le Debian d un autre disque branche reste"

depart
entree 0004 debian "$PARTI" "$CH"
demarrage nommer || true
porte "0004|debian|" "BIOS : sans nettoyer, une entree d ailleurs ne bouge pas"
demarrage nommer nettoyer || true
sans "0004|" "BIOS : a l installation, une entree debian vers une partition effacee part"

depart
entree 0004 debian "$PARTI" "$CH"
: > "$ETAT.lsblk-muet"
demarrage nommer nettoyer || true
porte "0004|debian|" "BIOS : disques illisibles, rien n est efface a l aveugle"

depart
: > "$ETAT.refus"
demarrage nommer nettoyer || true
porte "0002|debian|" "BIOS : creation refusee, l ancienne entree reste (il faut demarrer)"

depart
echo 0002 > "$ETAT.suivant"
demarrage nommer || true
if [ "$(cat "$ETAT.suivant")" = 0009 ]; then ok "BIOS : un BootNext vers l ancienne suit la nouvelle"; else faux "BIOS : un BootNext vers l ancienne suit la nouvelle"; fi

if [ -f "$T/esp/EFI/debian/BOOTX64.CSV" ] && tr -d '\000' < "$T/esp/EFI/debian/BOOTX64.CSV" | grep -aq 'shimx64.efi,grenOS'; then
    ok "BIOS : le secours de shim recreerait « grenOS », pas « debian »"
else
    faux "BIOS : le secours de shim recreerait « grenOS », pas « debian »"
fi

secours() { # la 1719 : fbx64 copie au chemin de secours
    mkdir -p "$T/esp/EFI/BOOT"
    echo fb | tee "$T/esp/EFI/debian/fbx64.efi" > "$T/esp/EFI/BOOT/fbx64.efi"
    echo grub > "$T/esp/EFI/BOOT/grubx64.efi"
}
depart; secours
mkdir -p "$T/esp/EFI/Microsoft"
demarrage nommer || true
if [ -f "$T/esp/EFI/BOOT/fbx64.efi" ]; then ok "secours : partition partagee avec Windows, rien n en part"; else faux "secours : partition partagee avec Windows, rien n en part"; fi
rmdir "$T/esp/EFI/Microsoft"
depart; secours
demarrage nommer || true
if [ ! -f "$T/esp/EFI/BOOT/fbx64.efi" ] && [ -f "$T/esp/EFI/BOOT/grubx64.efi" ]; then ok "secours : direct, sans ecran bleu"; else faux "secours : direct, sans ecran bleu"; fi

depart
entree 0005 grenOS "$A" "$CH"
demarrage nommer || true
jamais " -c " "BIOS : une entree grenOS deja la n est pas doublee"
sans "0002|" "BIOS : son double debian part"

depart
mv "$T/esp/EFI/debian/shimx64.efi" "$T/shim"
demarrage nommer nettoyer || true
jamais " -c " "BIOS : sans shim, aucune entree vers du vide"
porte "0002|debian|" "BIOS : sans shim, l ancienne reste"
mv "$T/shim" "$T/esp/EFI/debian/shimx64.efi"

# ---------------------------------------------------- Windows, sans le menu
depart
if demarrage windows-existe; then ok "Windows : trouve"; else faux "Windows : trouve"; fi
demarrage windows || true
if [ "$(cat "$ETAT.suivant" 2>/dev/null)" = 0000 ]; then ok "Windows : le prochain demarrage, et lui seul"; else faux "Windows : le prochain demarrage, et lui seul"; fi
jamais " -o " "Windows : l ordre du BIOS n est pas change"

: > "$ETAT"
entree 0001 "Windows Boot Manager" "$PARTI" "$MS"
entree 0002 grenOS "$A" "$CH"
if demarrage windows-existe; then faux "Windows : une entree morte n offre pas le bouton"; else ok "Windows : une entree morte n offre pas le bouton"; fi

# ------------------------------------------------------------------ le menu
REGLAGES=$(GRUB_CMDLINE_LINUX_DEFAULT="quiet splash resume=UUID=garde" sh -c '. "$1"; printf "%s|%s|%s\n" "$GRUB_TIMEOUT_STYLE" "$GRUB_TIMEOUT" "$GRUB_CMDLINE_LINUX_DEFAULT"' sh "$I/etc/default/grub.d/grenos.cfg")
case "$REGLAGES" in "hidden|0|"*) ok "menu : aucun, meme avec Windows" ;; *) faux "menu : aucun, meme avec Windows ($REGLAGES)" ;; esac
case "$REGLAGES" in *"quiet splash"*) ok "ligne du noyau : le chargement reste" ;; *) faux "ligne du noyau : le chargement reste" ;; esac
case "$REGLAGES" in *"resume=UUID=garde"*) ok "ligne du noyau : ce qui y etait reste" ;; *) faux "ligne du noyau : ce qui y etait reste" ;; esac
for mot in loglevel=3 systemd.show_status=false rd.systemd.show_status=false udev.log_level=3 vt.global_cursor_default=0; do
    case "$REGLAGES" in *"$mot"*) ok "ligne du noyau : $mot, comme sur la cle" ;; *) faux "ligne du noyau : $mot, comme sur la cle" ;; esac
done

if [ "$(sh "$I/etc/grub.d/06_grenos_discret" 2>/dev/null)" = 'echo -n " "
set color_normal=black/black' ]; then
    ok "menu : GRUB n ecrit plus a l ecran (console ouverte AVANT la couleur)"
else
    faux "menu : GRUB n ecrit plus a l ecran (console ouverte AVANT la couleur)"
fi
if grep -q 'chmod -x /etc/grub.d/25_bli' "$I/usr/lib/grenos/appliquer-systeme"; then
    ok "menu : plus de « prohibited by secure boot policy » (insmod bli)"
else
    faux "menu : plus de « prohibited by secure boot policy » (insmod bli)"
fi
DEB="$ICI/make-deb.sh"
for fichier in etc/default/grub.d/grenos.cfg etc/grub.d/06_grenos_discret \
               etc/systemd/system/lightdm.service.d/20-grenos-ecran-libre.conf; do
    if grep -qF "$fichier" "$DEB"; then ok "mise a jour : porte $fichier"; else faux "mise a jour : porte $fichier"; fi
done

# -------------------------------------------------------- la pose de GRUB
cat > "$T/bin/pose" <<'EOF'
#!/bin/sh
echo "pose $*" >> "$JOURNAL"
[ ! -e "$ETAT.pose-ratee" ]
EOF
cat > "$T/bin/nomme" <<'EOF'
#!/bin/sh
echo "nomme $*" >> "$JOURNAL"
EOF
chmod +x "$T/bin/pose" "$T/bin/nomme"
poser() {
    GRENOS_GRUB_INSTALL="$T/bin/pose" GRENOS_DEMARRAGE="$T/bin/nomme" \
        sh "$I/usr/lib/grenos/grenos-grub-install" "$@" > /dev/null 2>&1
}
: > "$JOURNAL"
poser --target=x86_64-efi --efi-directory="$T/esp" --bootloader-id=debian --force || true
attendu "pose --target" "pose : grub-install lance"
attendu "nomme nommer nettoyer" "pose : l entree est nommee juste apres"
attendu " --no-nvram" "pose : grub-install n efface plus les « debian » des autres disques"
if [ "$(grep -c '^pose ' "$JOURNAL")" = 1 ]; then ok "pose : une seule"; else faux "pose : une seule"; fi
: > "$JOURNAL"
printf '#!/bin/sh\nexit 1\n' > "$T/bin/refuse"
chmod +x "$T/bin/refuse"
GRENOS_GRUB_INSTALL="$T/bin/pose" GRENOS_DEMARRAGE="$T/bin/refuse" sh "$I/usr/lib/grenos/grenos-grub-install" \
    --target=x86_64-efi --efi-directory="$T/esp" --bootloader-id=debian > /dev/null 2>&1 || true
if [ "$(grep '^pose ' "$JOURNAL" | grep -vc -e '--no-nvram')" = 1 ]; then ok "pose : entree refusee, grub-install garde un PC qui demarre"; else faux "pose : entree refusee, grub-install garde un PC qui demarre"; fi
: > "$JOURNAL"
poser --target=x86_64-efi --efi-directory="$T/esp" --bootloader-id=debian --no-nvram || true
jamais "nomme" "pose : --no-nvram ne touche jamais la NVRAM"
: > "$JOURNAL"; : > "$ETAT.pose-ratee"
if poser --target=x86_64-efi --efi-directory="$T/esp" --bootloader-id=debian; then
    faux "pose : un echec de grub-install reste un echec"
else
    ok "pose : un echec de grub-install reste un echec"
fi
jamais "nomme" "pose : rien n est nomme apres un echec"

echo "demarrage : $cas cas, $fautes faute(s)"
[ "$fautes" -eq 0 ]
