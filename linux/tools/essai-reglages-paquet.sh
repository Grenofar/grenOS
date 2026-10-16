#!/bin/sh
# Exercer, DANS LES DEUX SENS, le tri des reglages de /etc vers le paquet.
#
# POURQUOI CET ESSAI EXISTE, et il a coute une construction.
#
# Les reglages de /etc partaient dans le paquet par sept copies nommees une
# par une. Je les ai remplacees par `find etc -name '*.conf'` — et le glob a
# ramasse `etc/lightdm/lightdm-gtk-greeter.conf`, qui appartient au paquet
# `lightdm-gtk-greeter`. dpkg a repondu, mot pour mot comme le 24 septembre :
# « trying to overwrite […] which is also in package lightdm-gtk-greeter ».
# La construction est morte apres trois minutes, et la lecon etait ecrite dans
# le fichier meme que je modifiais.
#
# LE TRI EST DONC LE SUJET, et il a trois issues qu'il faut toutes voir :
#   * un fichier a nous (`*grenos*`) part dans le paquet ;
#   * un fichier partage part en MODELE, pose par appliquer-systeme ;
#   * un troisieme cas — a nous, mais mal nomme — ARRETE la construction,
#     parce que sinon l'image l'aurait et une machine mise a jour non.
#
# Le bloc est EXTRAIT de make-deb.sh, jamais recopie.
racine=$(cd "$(dirname "$0")/../.." && pwd)
fabrique="${MAKEDEB:-$racine/linux/tools/make-deb.sh}"
ici=$(cd "$(dirname "$0")" && pwd)
bac=$(mktemp -d)
echecs=0

fragment="$bac/frag.sh"
sed -n '/^( cd "\$DEDANS" && find etc -type f -name/,/^done || exit 1$/p' \
    "$fabrique" > "$fragment"
if [ ! -s "$fragment" ]; then
  echo "le bloc n a pas ete trouve dans make-deb.sh — l essai ne juge rien"
  rm -rf "$bac"
  exit 1
fi
echo "  bloc extrait de make-deb.sh : $(wc -l < "$fragment") lignes"

# $1 = le chemin relatif a poser dans le faux includes.chroot
# $2 = le code de retour attendu
# $3 = ce qu on doit lire dans la sortie
juger() {
  essai="$bac/essai"
  rm -rf "$essai"
  mkdir -p "$essai/dedans/$(dirname "$1")" \
           "$essai/dedans/usr/lib/grenos" \
           "$essai/build/usr/share/grenos/modeles"
  printf 'x\n' > "$essai/dedans/$1"
  # LE VRAI `appliquer-systeme`, jamais une copie de ses lignes : c est lui que
  # le bloc interroge pour savoir si un chemin partage est pose. Deux listes
  # des fichiers poses finiraient par diverger, et ce serait alors la liste de
  # l essai qu on croirait.
  #
  # ET SON ABSENCE A ETE LE PREMIER VRAI DEFAUT TROUVE ICI : sans ce fichier,
  # `grep -c` rendait une chaine vide, le test d entier sortait en erreur, et
  # les quatre chemins partages partaient en « modele » comme si tout allait
  # bien. Quatre cas verts par le meme bug, et c est le cinquieme — l orphelin
  # — qui l a dit.
  cp "$racine/linux/config/includes.chroot/usr/lib/grenos/appliquer-systeme" \
     "$essai/dedans/usr/lib/grenos/"
  ( cd "$essai" && DEDANS="$essai/dedans" BUILD="$essai/build" \
      /bin/sh "$fragment" ) > "$essai/sortie" 2> "$essai/erreur"
  code=$?
  sortie=$(cat "$essai/sortie" "$essai/erreur")
  if [ "$code" != "$2" ]; then
    printf '  [KO] %-52s code %s (attendu %s)\n%s\n' "$1" "$code" "$2" "$sortie"
    echecs=$((echecs + 1))
    return
  fi
  if ! printf '%s' "$sortie" | grep -q "$3"; then
    printf '  [KO] %-52s la sortie ne dit pas « %s »\n%s\n' "$1" "$3" "$sortie"
    echecs=$((echecs + 1))
    return
  fi
  printf '  [ok] %-52s code %s, « %s »\n' "$1" "$code" "$3"
}

# 1. UN FICHIER A NOUS : il part dans le paquet.
juger "etc/systemd/journald.conf.d/10-grenos-persistant.conf" 0 "reglage :"

# 2. UN AUTRE, ailleurs dans l arborescence : meme traitement.
juger "etc/pipewire/pipewire.conf.d/20-grenos-temps-reel.conf" 0 "reglage :"

# 3. LE FICHIER QUI A TUE LA CONSTRUCTION : il doit partir en MODELE, pas dans
#    le paquet. C est le cas qui compte.
juger "etc/lightdm/lightdm-gtk-greeter.conf" 0 "modele  :"

# 4. Les trois autres chemins partages, pour les memes raisons.
juger "etc/xdg/libfm/libfm.conf" 0 "modele  :"
juger "etc/xdg/pcmanfm/default/pcmanfm.conf" 0 "modele  :"
juger "etc/fonts/local.conf" 0 "modele  :"

# 5. LE TROISIEME CAS : un reglage a nous dont le nom ne nous designe pas, et
#    que personne ne livre. La construction doit s ARRETER en le nommant.
juger "etc/quelquechose/reglage-sans-nom.conf" 1 "REGLAGE ORPHELIN"

# 6. Et le fichier qui est vraiment a nous dans le paquet n est pas confondu
#    avec un modele : un nom qui contient grenos passe meme dans un dossier
#    partage.
juger "etc/xdg/autostart/10-grenos-truc.conf" 0 "reglage :"

rm -rf "$bac"
echo
if [ "$echecs" -gt 0 ]; then
  echo "$echecs cas ont echoue."
  exit 1
fi
echo "les 8 cas passent, dont 1 refus et 4 renvois vers les modeles."
