#!/bin/sh
# TOUTES LES CARTES GRAPHIQUES ONT-ELLES LEUR PILOTE ? (26 novembre)
#
# Grenofar : « rends toutes les cartes compatibles pour la prochaine mise à
# jour », après une GT 1030 (Pascal) à qui le bouton posait le pilote NVIDIA
# « open », qui ne la connaît pas.
#
#   sh linux/tools/essai-cartes-graphiques.sh
#
# 1. grenos-nvidia-variante sur de fausses cartes (GT 1030, RTX 5060, Kepler,
#    mélanges, la sortie son d'une carte) ;
# 2. LA RÈGLE CONTRE LES LISTES DE NVIDIA : chaque identifiant que le paquet
#    d'Ubuntu nvidia-driver-580-open déclare (champ Modaliases) doit donner
#    « 580-open », chaque identifiant que seul nvidia-driver-580 déclare doit
#    donner « 580 », et aucun identifiant connu d'un des deux ne doit tomber
#    sur « aucun ». Sans réseau, cette partie est sautée ; en CI, elle est
#    exigée (CI=true) ;
# 3. les dépôts des deux variantes : même clé, même règle apt, le bon module ;
# 4. AMD et Intel : leur firmware, Mesa (OpenGL et Vulkan) et les pilotes X
#    sont dans l'image — rien à installer, aucun bouton.
set -u
ICI=$(cd "$(dirname "$0")" && pwd)
RACINE="$ICI/../config/includes.chroot"
VARIANTE="$RACINE/usr/lib/grenos/grenos-nvidia-variante"
DEPOTS="$RACINE/usr/share/grenos/depots"
LISTE="$ICI/../config/package-lists/grenos.list.chroot"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
FAUTES=0
ok() { echo "  [ok] $*"; }
faute() { echo "  [FAUTE] $*"; FAUTES=$((FAUTES + 1)); }

carte() { # nom vendeur classe id
    mkdir -p "$T/sys/bus/pci/devices/$1"
    echo "$2" > "$T/sys/bus/pci/devices/$1/vendor"
    echo "$3" > "$T/sys/bus/pci/devices/$1/class"
    echo "$4" > "$T/sys/bus/pci/devices/$1/device"
}
vide() { rm -rf "$T/sys"; mkdir -p "$T/sys/bus/pci/devices"; }
attendu() { # cas attendu
    lu=$(GRENOS_SYSFS="$T/sys" sh "$VARIANTE")
    if [ "$lu" = "$2" ]; then ok "$1 : $lu"; else faute "$1 : « $lu », attendu « $2 »"; fi
}

echo "1. la variante, carte par carte"
[ -x "$VARIANTE" ] || faute "grenos-nvidia-variante n est pas executable (droits de Git ?)"
vide; attendu "aucune carte" aucun
carte 0000:00:02.0 0x8086 0x030000 0x3e92; attendu "Intel seule" aucun
carte 0000:03:00.0 0x1002 0x030000 0x73df; attendu "Intel + AMD" aucun
vide; carte 0000:01:00.0 0x10de 0x030000 0x1d01; carte 0000:01:00.1 0x10de 0x040300 0x0fb8
attendu "GT 1030 GDDR5 (10de:1d01, Pascal)" 580
vide; carte 0000:01:00.0 0x10de 0x030000 0x1d02; attendu "GT 1030 DDR4 (10de:1d02)" 580
vide; carte 0000:01:00.0 0x10de 0x030000 0x1380; attendu "GTX 750 Ti (Maxwell)" 580
vide; carte 0000:01:00.0 0x10de 0x030000 0x1d81; attendu "Titan V (Volta)" 580
vide; carte 0000:01:00.0 0x10de 0x030000 0x2d05; attendu "RTX 5060 (Blackwell)" 580-open
vide; carte 0000:01:00.0 0x10de 0x030000 0x1f82; attendu "GTX 1650 (Turing)" 580-open
carte 0000:02:00.0 0x10de 0x030000 0x1b80; attendu "GTX 1650 + GTX 1080" 580
vide; carte 0000:01:00.0 0x10de 0x030000 0x1287; attendu "GT 730 (Kepler)" aucun
carte 0000:02:00.0 0x10de 0x030000 0x2484; attendu "GT 730 + RTX 3070" 580-open
vide; carte 0000:01:00.1 0x10de 0x040300 0x10fa; attendu "la sortie son seule" aucun
vide; carte 0000:01:00.0 0x10de 0x030000 0x1d01
lu=$(GRENOS_SYSFS="$T/sys" sh "$VARIANTE" paquet)
[ "$lu" = nvidia-driver-580 ] && ok "paquet : $lu" || faute "paquet : « $lu »"

echo "2. la regle contre les listes de NVIDIA (paquets d Ubuntu 26.04)"
: > "$T/Packages"
lu_reseau=0
for poche in resolute resolute-updates; do
    if curl -fsSL --max-time 60 "http://archive.ubuntu.com/ubuntu/dists/$poche/restricted/binary-amd64/Packages.xz" \
        -o "$T/p.xz" 2>/dev/null && xz -dc "$T/p.xz" >> "$T/Packages" 2>/dev/null; then
        lu_reseau=1
        echo >> "$T/Packages"
    fi
done
if [ "$lu_reseau" = 0 ]; then
    if [ "${CI:-}" = true ]; then faute "les paquets d Ubuntu sont illisibles : la regle n est pas prouvee"
    else echo "  (pas de reseau : partie sautee)"; fi
else
    python3 - "$T/Packages" "$VARIANTE" "$T" <<'PY' || FAUTES=$((FAUTES + 1))
import os, re, subprocess, sys
paquets, variante, t = sys.argv[1], sys.argv[2], sys.argv[3]
meilleur = {}
for bloc in open(paquets, encoding="utf-8", errors="replace").read().split("\n\n"):
    m = re.search(r"^Package: (nvidia-driver-580(?:-open)?)$", bloc, re.M)
    if not m:
        continue
    v = re.search(r"^Version: (.*)$", bloc, re.M).group(1)
    ids = set(re.findall(r"v000010DEd0000([0-9A-F]{4})", (re.search(r"^Modaliases: (.*)$", bloc, re.M) or [None, ""])[1]))
    if ids and (m.group(1) not in meilleur or subprocess.run(["dpkg", "--compare-versions", v, "gt", meilleur[m.group(1)][0]]).returncode == 0):
        meilleur[m.group(1)] = (v, ids)
if set(meilleur) != {"nvidia-driver-580", "nvidia-driver-580-open"}:
    print(f"  [FAUTE] listes introuvables : {sorted(meilleur)}"); sys.exit(1)
ferme_v, ferme = meilleur["nvidia-driver-580"]
ouvert_v, ouvert = meilleur["nvidia-driver-580-open"]
print(f"  [ok] {len(ferme)} cartes pour le 580 ferme ({ferme_v}), {len(ouvert)} pour le open ({ouvert_v})")
fautes = 0
if not ouvert <= ferme:
    print(f"  [FAUTE] le open connait des cartes que le ferme ignore : {sorted(ouvert - ferme)[:8]}"); fautes += 1

def juger(ident):
    d = os.path.join(t, "un", "bus/pci/devices/0000:01:00.0")
    os.makedirs(d, exist_ok=True)
    for nom, val in (("vendor", "0x10de"), ("class", "0x030000"), ("device", "0x" + ident.lower())):
        open(os.path.join(d, nom), "w").write(val + "\n")
    return subprocess.run(["sh", variante], env={**os.environ, "GRENOS_SYSFS": os.path.join(t, "un")},
                          capture_output=True, text=True).stdout.strip()

for ident in sorted(ferme | ouvert):
    voulu = "580-open" if ident in ouvert else "580"
    lu = juger(ident)
    if lu != voulu:
        print(f"  [FAUTE] 10de:{ident.lower()} : « {lu} », attendu « {voulu} »"); fautes += 1
if fautes == 0:
    print(f"  [ok] les {len(ferme | ouvert)} identifiants donnent la variante qui les connait, aucun n est laisse sans pilote")
sys.exit(1 if fautes else 0)
PY
fi

echo "3. les depots des deux variantes"
for v in 580 580-open; do
    for ext in list gpg empreinte pref avec signe; do
        [ -s "$DEPOTS/nvidia-driver-$v.$ext" ] || faute "nvidia-driver-$v.$ext manque"
    done
    grep -q "signed-by=/usr/share/keyrings/nvidia-driver-$v-archive-keyring.gpg" "$DEPOTS/nvidia-driver-$v.list" \
        || faute "nvidia-driver-$v.list ne nomme pas sa propre cle"
    [ "$(tr -d ' \n' < "$DEPOTS/nvidia-driver-$v.avec")" = "linux-modules-nvidia-$v-generic" ] \
        || faute "nvidia-driver-$v.avec n est pas linux-modules-nvidia-$v-generic"
done
cmp -s "$DEPOTS/nvidia-driver-580.gpg" "$DEPOTS/nvidia-driver-580-open.gpg" && ok "meme cle d Ubuntu" || faute "deux cles differentes"
cmp -s "$DEPOTS/nvidia-driver-580.pref" "$DEPOTS/nvidia-driver-580-open.pref" && ok "meme regle apt" || faute "deux regles apt differentes"
python3 - "$DEPOTS/nvidia-driver-580.pref" <<'PY' || FAUTES=$((FAUTES + 1))
import re, sys
motif = re.search(r"^Package: /(.*)/$", open(sys.argv[1], encoding="utf-8").read(), re.M).group(1)
fautes = 0
for nom, voulu in (("linux-modules-nvidia-580-generic", True), ("linux-modules-nvidia-580-open-generic", True),
                   ("linux-modules-nvidia-580-7.0.0-44-generic", True), ("linux-objects-nvidia-580-7.0.0-44-generic", True),
                   ("linux-signatures-nvidia-7.0.0-44-generic", True), ("nvidia-driver-580", True),
                   ("linux-image-7.0.0-44-generic", True), ("libnvidia-gl-580", True),
                   ("linux-headers-7.0.0-44-generic", False), ("firefox", False), ("systemd", False),
                   ("linux-modules-nvidia-595-generic", False), ("mesa-vulkan-drivers", False)):
    if bool(re.match(motif, nom)) != voulu:
        print(f"  [FAUTE] regle apt : {nom} {'refuse' if voulu else 'permis'}"); fautes += 1
print("  [ok] la regle apt laisse venir d Ubuntu le noyau et les deux variantes, rien d autre" if not fautes else "", end="\n" if not fautes else "")
sys.exit(1 if fautes else 0)
PY

echo "4. AMD et Intel : tout est deja dans l image"
for p in firmware-amd-graphics xserver-xorg-video-amdgpu xserver-xorg-video-ati mesa-vulkan-drivers \
         va-driver-all mesa-vdpau-drivers firmware-intel-graphics xserver-xorg-video-nouveau \
         firmware-nvidia-graphics vulkan-tools mesa-utils; do
    grep -qx "$p" "$LISTE" && ok "$p" || faute "$p manque a l image"
done
grep -q '^Package: .*firmware' "$ICI/../config/archives/grenos-noyau.pref.chroot" 2>/dev/null \
    && ok "firmware des backports (RDNA 4, RTX 50)" || faute "le firmware n est plus pris dans les backports"

if [ "$FAUTES" -gt 0 ]; then
    echo "cartes graphiques : $FAUTES faute(s)"
    exit 1
fi
echo "cartes graphiques : NVIDIA de Maxwell a Blackwell sur leur pilote signe, Kepler sur le libre, AMD et Intel prets"
