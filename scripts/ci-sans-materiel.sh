#!/bin/sh
# A11 : « un peripherique attendu et absent ne doit jamais faire echouer le
# demarrage ». C est la ligne qu il appelle lui-meme « la plus importante de
# la section », et c etait la seule a n avoir aucune preuve.
#
# La moitie decisive est faite depuis le 15 octobre : Calamares ecrivait
# `defaults` sur chaque ligne du fstab, donc aucun `nofail`, donc un second
# disque absent envoyait une machine installee en mode secours derriere un mot
# de passe root que grenOS ne definit jamais. Corrige, et garde a la
# construction.
#
# Mais un fichier de configuration pose ne prouve rien — c est la lecon de
# `logo.png`, pose, vu par son garde, et sans effet d un seul pixel. Il fallait
# RETIRER le materiel et regarder.
#
# On enleve donc les deux absences les plus courantes, et les seules que la
# machine d integration sache produire : AUCUN DISQUE et AUCUNE CARTE RESEAU.
# Sa liste les nomme toutes les deux — A9 « les pilotes Ethernet sont tres
# cibles, une machine peut simplement ne pas avoir de reseau », et A1/A7 pour
# le disque.
#
# CE QUI REND CETTE PREUVE NON VIDE. Un essai qui enleve une carte reseau et
# qui ne verifie pas qu elle est partie dirait « vert » meme si QEMU avait
# ignore l option. On exige donc que la machine CONSTATE l absence :
# `maj: depot INJOIGNABLE` ne peut sortir que si rien ne repond. Sans cette
# ligne, le bureau ouvert ne prouverait rien du tout — ce serait le detecteur
# de trous qui mesurait la diction d une voix.
#
# Le verdict est un SEUL morceau de code, appele par le vrai parcours comme
# par l essai (`--juger <journal>`) : deux copies d une meme mesure finissent
# par diverger, et c est alors la copie qu on croit.

set -u

juger() {
    journal="$1"
    if [ ! -s "$journal" ]; then
        echo "::error::Aucune ligne sur le port serie : la machine n a rien dit."
        return 1
    fi

    # 1. L ABSENCE A-T-ELLE EU LIEU ? A verifier AVANT le reste : un bureau
    #    qui s ouvre sur une machine qui avait son reseau ne repond pas a la
    #    question posee.
    if grep -aq 'maj: depot joignable' "$journal"; then
        echo "::error::Le depot a repondu : la carte reseau n a pas ete retiree."
        echo "::error::La preuve serait vide — elle ne mesurerait pas l absence."
        return 1
    fi
    if ! grep -aq 'maj: depot INJOIGNABLE' "$journal"; then
        echo "::error::La machine n a pas dit que le depot etait injoignable."
        echo "::error::Rien ne prouve que le reseau etait absent : essai sans valeur."
        return 1
    fi
    echo "sans materiel : le depot est injoignable, l absence est constatee"

    # 2. ET LE BUREAU S EST-IL QUAND MEME OUVERT ? C est la demande, mot pour
    #    mot : un peripherique absent ne doit jamais faire echouer le demarrage.
    if ! grep -aq 'session de bureau ouverte' "$journal"; then
        echo "::error::Sans disque et sans reseau, le bureau ne s ouvre pas."
        echo "::error::C est exactement A11 : un peripherique absent fait echouer le demarrage."
        return 1
    fi
    echo "sans materiel : le bureau s est ouvert sans disque et sans carte reseau"

    # 3. CE QU ON RAPPORTE SANS EN FAIRE UNE CONDITION. On ne sait pas encore
    #    ce que la machine compte quand il n y a aucun disque — et poser une
    #    condition sur un chiffre qu on n a jamais vu, c est ecrire une
    #    assertion qui ne peut que se tromper. On le lit d abord.
    grep -a 'emplacements :\|disques :\|hub : diagnostic Stockage\|hub : diagnostic Reseau\|hub : diagnostic Réseau' \
        "$journal" | sed 's/^.*grenos: /sans materiel : /' || true
    return 0
}

if [ "${1:-}" = "--juger" ]; then
    juger "${2:-}"
    exit $?
fi

# ---------------------------------------------------------------- le parcours

ISO="${1:-}"
if [ -z "$ISO" ] || [ ! -f "$ISO" ]; then
    echo "::error::ci-sans-materiel.sh : image introuvable."
    exit 1
fi
SERIE="$RUNNER_TEMP/serial-sans-materiel.log"
PRISE="$RUNNER_TEMP/sansmat.sock"
: > "$SERIE"

ACCEL=""
if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then ACCEL="-enable-kvm -cpu host"; fi

# `-nic none` retire la carte, et AUCUN `-drive` : la machine n a que son
# lecteur optique. C est la machine la plus nue que QEMU sache faire.
qemu-system-x86_64 $ACCEL -m 2048 -smp 2 -cdrom "$ISO" -boot d -nic none \
    -display none -vga std -global VGA.xres=1280 -global VGA.yres=720 \
    -serial file:"$SERIE" -monitor unix:"$PRISE",server,nowait -no-reboot &
QEMU=$!
sleep 3
if ! kill -0 "$QEMU" 2>/dev/null; then
    echo "::error::QEMU n a pas demarre sans materiel : sa ligne de commande est fautive."
    exit 1
fi

# Le menu de demarrage, puis Entree — comme la personne devant l ecran.
sleep 20
python3 scripts/ci-screen.py send "$PRISE" "sendkey ret" || true

# Le premier ecran demande un nom, et la session ne s ouvre pas tant qu il n
# en a pas. On tape « ci », exactement comme le parcours ordinaire.
for essai in $(seq 1 48); do
    if grep -aq 'premier ecran affiche' "$SERIE" 2>/dev/null; then
        echo "sans materiel : premier ecran vu apres $((essai * 5))s"
        for touche in c i ret; do
            python3 scripts/ci-screen.py send "$PRISE" "sendkey $touche" || true
            sleep 1
        done
        break
    fi
    sleep 5
done

# Puis on attend le bureau. Six minutes : sans reseau, la sonde met plus de
# temps a renoncer qu a reussir, et conclure trop tot dirait « pas de bureau »
# pour une machine qui en avait un.
for essai in $(seq 1 72); do
    if grep -aq 'session de bureau ouverte' "$SERIE" 2>/dev/null; then
        echo "sans materiel : bureau vu apres $((essai * 5))s"
        break
    fi
    sleep 5
done
sleep 10
python3 scripts/ci-screen.py grab "$PRISE" "$RUNNER_TEMP/ecran-sans-materiel.ppm" || true
kill $QEMU 2>/dev/null || true

grep -a 'grenos:' "$SERIE" | tail -n 12 || true
juger "$SERIE"
