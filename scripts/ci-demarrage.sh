#!/usr/bin/env bash
# L'image démarre-t-elle vraiment, et obéit-elle ?
#
# Ce script vivait dans `.github/workflows/linux.yml`, dans un seul `run:`. Il
# a fini par dépasser **21 000 caractères**, la limite d'un `run` chez GitHub
# (« Runs command-line programs that do not exceed 21,000 characters »).
# Au-delà, GitHub refuse le fichier ENTIER : aucun job ne démarre, et le seul
# indice est le nom du workflow remplacé par son propre chemin. Deux
# constructions perdues, et un long moment à chercher une faute de syntaxe qui
# n'existait pas.
#
# Il est ici tel quel, ligne pour ligne. Ce qu'il fait : démarrer l'image dans
# QEMU avec deux disques, taper un nom au premier écran, cliquer pour de vrai
# sur le bouton grenOS, ouvrir le magasin, les Jeux, le son et les Réglages,
# puis juger ce que le port série a dit.
#
# Il lit de son appelant : ISO_CONSTRUITE, ACCEL_FORCE, RUNNER_TEMP,
# GITHUB_OUTPUT. Il n'a plus le droit de grandir sans fin : un fichier, lui,
# n'a pas de limite, et c'est bien la raison d'être de ce déplacement.

# GitHub lance un « run: » avec `bash -e {0}` : la moindre commande qui
# echoue arrete l'etape. Tout ce script a ete ecrit sous cette regle — c'est
# pour cela qu'il porte des « || true » partout ou un echec est acceptable.
# Appele comme un fichier, il perdrait ce -e, et une panne passerait inapercue
# jusqu'a la fin. On le remet, explicitement, pour que le comportement ne
# depende plus de la facon dont on l'appelle.
set -e

# L'image a construire, passee par l'environnement : une expression
# ${{ }} n'a de sens que dans le YAML, et ce script n'y vit plus.
ISO="${ISO_CONSTRUITE:?l appelant doit poser ISO_CONSTRUITE}"
ACCEL=""
if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then
  ACCEL="-enable-kvm -cpu host"
  echo "KVM disponible"
else
  echo "sans KVM : l'emulation est lente, le bureau met plusieurs minutes"
fi
# Deux disques, sur un controleur SATA comme en a un PC et comme en
# donne VirtualBox.
#
# Le premier est **celui qu'on livre** : une partition ext4 etiquetee
# « persistence », avec le fichier que live-boot y cherche. C'est ce
# qui fait tenir la promesse de la page de telechargement — « tout ce
# que tu fais est garde ». Personne ne l'avait jamais verifie : la
# machine d'essai demarrait sans disque, donc live-boot cherchait une
# persistance, n'en trouvait pas, et continuait. Le chemin que
# Grenofar emprunte vraiment n'etait pas celui qu'on testait.
#
# Le second est celui que l'installateur proposerait, et il porte desormais
# une partition ext4 etiquetee DONNEES.
#
# Il etait **vierge** jusqu'a ce matin : ni table de partition, ni systeme de
# fichiers. C'est ce qui rendait la question de Grenofar — « est-ce qu'on peut
# les voir sur l'explorateur ? » — impossible a trancher ici : un disque sans
# systeme de fichiers n'a aucun volume a montrer, et Windows ne l'afficherait
# pas davantage. L'essai repondait donc « non » quelle que soit la
# configuration, et aurait continue a le faire apres n'importe quelle
# correction.
#
# Meme faute que le detecteur de trous qui mesurait la diction d'une voix :
# ce n'etait pas la mesure qu'il fallait corriger, c'etait le signal.
#
# L'etiquette n'est surtout pas « persistence » : live-boot la cherche, et
# deux candidats feraient une panne que personne ne saurait lire.
#
# Le marqueur `/etc/grenos/essai-charge` fait de ce disque celui de la machine
# d'integration, et rien d'autre : la session y joue le son une SECONDE fois,
# les coeurs satures. C'est la seule facon de voir le hoquet que Grenofar
# entend « quand y'a la video », et un son propre au repos ne le voit pas.
# Aucune machine reelle n'a ce fichier.
sudo linux/tools/make-disk.sh "$RUNNER_TEMP/persistence.vdi" 8 etc/grenos/essai-charge
sudo chown "$(id -u):$(id -g)" "$RUNNER_TEMP/persistence.vdi"
qemu-img create -f raw "$RUNNER_TEMP/disque.raw" 20G >/dev/null
sudo sgdisk --new=1:2048:0 --typecode=1:8300 --change-name=1:DONNEES \
  "$RUNNER_TEMP/disque.raw" >/dev/null
BOUCLE=$(sudo losetup --find --show --partscan "$RUNNER_TEMP/disque.raw")
sudo mkfs.ext4 -q -L DONNEES "${BOUCLE}p1"
sudo losetup -d "$BOUCLE"
sudo chown "$(id -u):$(id -g)" "$RUNNER_TEMP/disque.raw"
echo "second disque : 20 Go, une partition ext4 etiquetee DONNEES"
qemu-system-x86_64 $ACCEL -m 3072 -smp 2 \
  -cdrom "$ISO" -boot d \
  -drive file="$RUNNER_TEMP/persistence.vdi",format=vdi,if=none,id=garde \
  -drive file="$RUNNER_TEMP/disque.raw",format=raw,if=none,id=dur \
  -device ahci,id=sata \
  -device ide-hd,drive=garde,bus=sata.0 \
  -device ide-hd,drive=dur,bus=sata.1 \
  -display none -vga std \
  -global VGA.xres=1280 -global VGA.yres=720 \
  -usb -device usb-tablet \
  -smbios type=1,version=grenos-essai-charge \
  -audiodev wav,id=son,path="$RUNNER_TEMP/son.wav" \
  -device intel-hda -device hda-duplex,audiodev=son \
  -serial file:"$RUNNER_TEMP/serial.log" \
  -monitor unix:"$RUNNER_TEMP/monitor.sock",server,nowait \
  -qmp unix:"$RUNNER_TEMP/qmp.sock",server,nowait \
  -no-reboot &
QEMU=$!
# QEMU a-t-il seulement demarre ? Une option mal ecrite le fait
# sortir aussitot, et sans ce controle on attend six minutes devant
# un ecran qui n'existe pas avant de comprendre. C'est arrive trois
# fois le 23 septembre.
sleep 3
if ! kill -0 "$QEMU" 2>/dev/null; then
  echo "::error::QEMU n'a pas demarre : sa ligne de commande est fautive."
  exit 1
fi
# Vingt secondes pour arriver au menu de démarrage, puis Entrée,
# comme le ferait la personne devant l'écran. Sans cela, un menu qui
# attend passe pour un système qui a démarré.
sleep 20
python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/ecran-menu.ppm" || true
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey ret" || true
# Le premier écran demande un nom, et la session ne s'ouvre pas tant
# qu'il n'en a pas. Il l'annonce sur le port série : on l'attend, on
# tape « ci », et on valide — exactement ce que fait une personne.
for essai in $(seq 1 60); do
  if grep -q 'premier ecran affiche' "$RUNNER_TEMP/serial.log" 2>/dev/null; then
    echo "premier ecran vu apres $((essai * 5))s"
    python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/ecran-nom.ppm" || true
    for touche in c i ret; do
      python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey $touche" || true
      sleep 1
    done
    break
  fi
  sleep 5
done
if ! grep -q 'nom choisi' "$RUNNER_TEMP/serial.log" 2>/dev/null; then
  echo "le nom n'a pas encore ete pris en compte, on continue d'observer"
fi
sleep 15
# Le premier ecran doit accepter ce qu'on tape. S'il ne l'accepte pas,
# la personne reste devant une fenetre qui ignore son clavier — et le
# bureau finit par s'ouvrir tout seul, ce qui masque la panne. C'est
# exactement ce qui s'est passe le 18 septembre a 19h54.
if grep -q 'nom choisi' "$RUNNER_TEMP/serial.log" 2>/dev/null; then
  grep -a 'nom choisi' "$RUNNER_TEMP/serial.log"
else
  echo "::error::Le premier ecran n'a pas pris le nom tape : il ignore le clavier."
  kill $QEMU 2>/dev/null || true
  exit 1
fi
# Le bureau met du temps à venir : on regarde à intervalles, et on
# garde chaque capture pour pouvoir les lire ensuite.
for minute in 1 2 3 4 5; do
  sleep 60
  python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/ecran-${minute}min.ppm" || true
done
# Le bureau s'affiche-t-il, ou obeit-il ? Une capture d'ecran ne
# repond qu'a la premiere question. On lui parle donc : on ouvre le
# menu au clavier, on le referme, on ouvre le gestionnaire de taches,
# et le bureau dit sur le port serie ce qu'il a fait. Sans cela, une
# barre qui tombe au premier clic passerait au vert — c'est ce qui a
# failli arriver le 23 septembre.
echo "--- la souris, pour de vrai ---"
# Le clic passe par QMP, pas par le moniteur. La source de QEMU est
# sans appel : `hmp_mouse_move` finit toujours par un deplacement
# RELATIF, et `qemu_input_find_handler` ne remet un evenement qu'a un
# peripherique dont le masque le reconnait. Une tablette USB est
# ABSOLUE : son masque ne contient pas REL. Nos trois tentatives
# parlaient donc a la souris PS/2, et lui disaient « avance de 15900
# pixels » — le pointeur partait dans un coin. `input-send-event`,
# lui, porte de vraies coordonnees absolues.
# Ce test ne bloque pas encore la publication : on ne rend une etape
# obligatoire qu'apres l'avoir vue verte.
# `grep -c` rend 0 et sort en erreur quand il ne trouve rien : le
# « || echo 0 » ajoutait alors un second zero, et la comparaison
# d'entiers plus bas recevait « 0 0 ». On prend la premiere ligne.
AVANT=$(grep -ac 'menu ouvert' "$RUNNER_TEMP/serial.log" 2>/dev/null | head -1)
AVANT=${AVANT:-0}
# La barre dit ou se trouve son bouton : on vise ce point plutot que
# de deviner. Elle est centree, donc sa position depend du nombre
# d'icones — un clic au juge tombe a cote, et c'est ce qui est arrive.
# Attendre que la barre ait annonce sa position : elle le fait trois
# secondes apres son demarrage, et le premier essai cliquait avant
# que la ligne ne soit arrivee — donc au mauvais endroit.
for essai in $(seq 1 30); do
  grep -aq 'bouton grenos en' "$RUNNER_TEMP/serial.log" && break
  sleep 2
done
# `tr -d` enleve le retour chariot : une console serie termine ses
# lignes par CR LF, et « 774 » arrivait ici en « 774\r ». Le calcul
# echouait, la souris recevait une abscisse sans ordonnee, et le clic
# tombait n'importe ou — sans que rien ne le dise.
POINT=$(grep -a 'bouton grenos en' "$RUNNER_TEMP/serial.log" \
        | tail -1 | tr -d '\r' | sed 's/.*en //')
# La taille de l'ecran se lit dans la capture, au lieu de la supposer :
# X ne choisit pas forcement la resolution qu'on croit, et un clic
# converti avec la mauvaise taille tombe a cote sans rien dire.
python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/avant-clic.ppm" || true
TAILLE=$(python3 scripts/ci-screen.py taille "$RUNNER_TEMP/avant-clic.ppm" || echo "1280 800")
LARGEUR=${TAILLE%% *}
HAUTEUR=${TAILLE##* }
echo "souris: l ecran fait ${LARGEUR}x${HAUTEUR}"
if [ -n "$POINT" ]; then
  PX=${POINT%,*}
  PY=${POINT#*,}
  echo "souris: le bouton grenOS est en $PX,$PY"
else
  PX=$(( LARGEUR / 2 ))
  PY=$(( HAUTEUR - 24 ))
  echo "souris: position inconnue, on vise le centre de la barre"
fi
python3 scripts/ci-screen.py clic "$RUNNER_TEMP/qmp.sock" "$PX" "$PY" "$LARGEUR" "$HAUTEUR" || true
sleep 5
python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/ecran-clic.ppm" || true
# `grep -c` rend « 0 » ET sort en erreur quand il ne trouve rien : le
# « || echo 0 » ajoutait un second zero, et la comparaison recevait
# « 0 0 ». Cela ne se voyait pas, parce que le `if` avalait l'erreur
# et prenait la bonne branche par accident. C'est le meme piege qui
# bloquait l'indicateur de mises a jour sur « je ne sais pas ».
APRES=$(grep -ac 'menu ouvert' "$RUNNER_TEMP/serial.log" 2>/dev/null | head -1)
APRES=${APRES:-0}
CLIC=1
if [ "$APRES" -gt "$AVANT" ]; then
  echo "souris: le clic sur grenOS a ouvert le menu"
else
  CLIC=0
  echo "souris: le clic n'a rien ouvert"
fi
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey esc" || true
sleep 3

echo "--- le bureau repond-il ---"
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey ctrl-esc" || true
sleep 6
python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/ecran-menu-ouvert.ppm" || true
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey esc" || true
sleep 4

echo "--- la touche Windows ouvre-t-elle le menu ---"
# Grenofar : « quand on clique sur le bouton windows de notre clavier,
# c'est comme si on ouvrait le menu deroulant grenOS ». Savoir que
# xcape a demarre ne prouve rien : il faut appuyer sur la touche.
#
# `sendkey meta_l` envoie l'appui ET le relachement sans autre touche
# entre les deux — exactement la condition que xcape attend pour
# emettre. Si le menu s'ouvre, la promesse est tenue.
AVANT_META=$(grep -ac 'menu ouvert' "$RUNNER_TEMP/serial.log" 2>/dev/null | head -1)
AVANT_META=${AVANT_META:-0}
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey meta_l" || true
sleep 6
python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/ecran-touche-windows.ppm" || true
APRES_META=$(grep -ac 'menu ouvert' "$RUNNER_TEMP/serial.log" 2>/dev/null | head -1)
APRES_META=${APRES_META:-0}
if [ "$APRES_META" -gt "$AVANT_META" ]; then
  echo "windows: la touche Windows a ouvert le menu"
else
  # EXIGE depuis le 25 septembre : vu vert trois fois. Grenofar l'a demande
  # explicitement, et c'est un geste qu'on fait cent fois par jour.
  echo "::error::La touche Windows n a pas ouvert le menu."
  exit 1
fi
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey esc" || true
sleep 3
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey ctrl-shift-esc" || true
sleep 10
python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/ecran-taches.ppm" || true

echo "--- le magasin s ouvre-t-il ---"
# GrenPlace est la vitrine, et personne ne l'avait jamais ouvert ici :
# il aurait pu ne plus demarrer du tout sans qu'une seule etape ne
# rougisse. On l'ouvre comme le ferait une personne — le menu, quatre
# lettres, Entree.
#
# « grenp » ne designe que lui, et ses cinq lettres sont a la meme
# place en AZERTY qu'en QWERTY. « a » ne l'est pas : `sendkey a`
# donnerait « q » dans la machine, et la recherche ne trouverait rien.
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey esc" || true
sleep 2
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey ctrl-esc" || true
sleep 4
for touche in g r e n p ret; do
  python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey $touche" || true
  sleep 1
done
sleep 20
python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/ecran-grenplace.ppm" || true
# Il dit aussi d'ou vient son catalogue : du reseau, du dernier connu,
# ou de celui livre avec l'image. Les trois sont des reponses valables,
# et savoir laquelle vaut mieux que de supposer.
if grep -aq 'grenplace ouvert' "$RUNNER_TEMP/serial.log"; then
  grep -a 'grenplace ouvert' "$RUNNER_TEMP/serial.log" | tail -1 | tr -d '[:cntrl:]' | sed 's/^.*grenos: //'
else
  echo "magasin: il ne s est pas annonce (test non bloquant, a affiner)"
fi

echo "--- le mode Jeux s ouvre-t-il ---"
# La page dont il attend le plus, et la derniere qui n'avait jamais
# ete ouverte ici. « jeux » ne designe qu'elle dans le menu, et ses
# quatre lettres sont a la meme place en AZERTY qu'en QWERTY.
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey ctrl-esc" || true
sleep 4
for touche in j e u x ret; do
  python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey $touche" || true
  sleep 1
done
sleep 20
python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/ecran-jeux.ppm" || true
if grep -aq 'jeux ouvert' "$RUNNER_TEMP/serial.log"; then
  grep -a 'jeux ouvert' "$RUNNER_TEMP/serial.log" | tail -1 | tr -d '[:cntrl:]' | sed 's/^.*grenos: //'
else
  echo "jeux: la page ne s est pas annoncee (test non bloquant, a affiner)"
fi
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey esc" || true
sleep 2

echo "--- le son repond-il ---"
# Le seul reproche de Grenofar qu'on n'ait pas encore pu prouver
# autrement qu'en comptant les sorties. On clique sur le petit
# haut-parleur en bas a gauche — la barre dit ou il est, comme pour
# le bouton grenOS — et le panneau annonce ce qu'il lit du melangeur :
# combien de sorties, quel volume, muet ou non. Lire l'etat, pas
# seulement lister des peripheriques.
POINT_SON=$(grep -a 'bouton son en' "$RUNNER_TEMP/serial.log" | tail -1 | tr -d '[:cntrl:]' | sed 's/.*en //')
if [ -n "$POINT_SON" ]; then
  SX=${POINT_SON%,*}
  SY=${POINT_SON#*,}
  echo "son: le bouton est en $SX,$SY"
  python3 scripts/ci-screen.py clic "$RUNNER_TEMP/qmp.sock" "$SX" "$SY" "$LARGEUR" "$HAUTEUR" || true
  sleep 12
  python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/ecran-son.ppm" || true
  if grep -aq 'son ouvert' "$RUNNER_TEMP/serial.log"; then
    grep -a 'son ouvert' "$RUNNER_TEMP/serial.log" | tail -1 | tr -d '[:cntrl:]' | sed 's/^.*grenos: //'
  else
    echo "son: le panneau ne s est pas annonce (test non bloquant, a affiner)"
  fi

  # « fais en sorte que si on clique autre part ca disparait ».
  # C'est un geste, pas un reglage : on le fait. Un clic loin du
  # panneau — en haut a droite, sur le bureau vide — doit le fermer.
  #
  # Ce controle vaut double depuis que l'image a dit « clic ailleurs
  # par le focus seul » : l'attrape du pointeur n'a PAS pris, et donc
  # ne prenait sans doute jamais. C'est le filet du focus qui travaille,
  # et il n'avait jamais ete exerce.
  # --- Les coins sont-ils vraiment arrondis, ou noirs ? ---
  #
  # « les bords sont noirs dans l'overlay » : c'est le reproche d'origine, et
  # il n'etait verifie NULLE PART. Les quatre coins avaient ete mesures a la
  # main le 25 septembre, une fois, jamais transformes en controle — et le
  # panneau a demenage a droite depuis, donc cette mesure ne valait meme plus
  # pour l'endroit ou il s'ouvre.
  #
  # X11 sans composition n'a pas de transparence : un `border-radius` ne fait
  # que montrer le noir de la fenetre. Les vrais coins sont RETIRES par un
  # masque de forme. Si ce masque cesse de s'appliquer, la seule chose qui le
  # dira est un carre noir a chaque angle — invisible sur le port serie.
  #
  # Le panneau dit lui-meme ou il est : deviner sa position sur l'image se
  # trompe de fenetre des qu'une autre est ouverte derriere. Essaye, et c'est
  # exactement ce qui est arrive.
  PLACE=$(grep -a 'grenos: son : panneau en' "$RUNNER_TEMP/serial.log" | tail -1 \
          | tr -d '[:cntrl:]' | sed 's/.*panneau en //')
  if [ -n "$PLACE" ] && [ -f "$RUNNER_TEMP/ecran-son.ppm" ]; then
    echo "son: le panneau est en $PLACE"
    python3 - "$RUNNER_TEMP/ecran-son.ppm" "$PLACE" <<'COINS'
import sys

chemin, place = sys.argv[1], sys.argv[2]
coin, taille = place.split(" de ")
px0, py0 = (int(v) for v in coin.split(","))
pl, ph = (int(v) for v in taille.split("x"))

with open(chemin, "rb") as fichier:
    brut = fichier.read()

# Entete PPM : P6, largeur, hauteur, maxval, puis les octets.
champs, i = [], 2
while len(champs) < 3:
    while brut[i:i + 1].isspace():
        i += 1
    if brut[i:i + 1] == b"#":
        while brut[i:i + 1] != b"\n":
            i += 1
        continue
    j = i
    while not brut[j:j + 1].isspace():
        j += 1
    champs.append(int(brut[i:j]))
    i = j
i += 1
L, H, _ = champs
pixels = brut[i:]


def somme(x, y):
    d = (y * L + x) * 3
    return pixels[d] + pixels[d + 1] + pixels[d + 2]


# Le seuil, et pourquoi il est si bas.
#
# Le fond d'une fenetre GTK non masquee est du NOIR PUR : sous X11 sans visuel
# RGBA, ce que le `border-radius` laisse voir est (0,0,0). On ne cherche donc
# pas « sombre ».
#
# Premier essai a 25, et il a signale le coin bas-droit comme fautif. En
# regardant l'image plutot qu'en croyant le test : le FOND D'ECRAN, juste a
# cote du panneau, vaut 22 a 31 a cet endroit. Le coin montrait donc le papier
# peint — c'est-a-dire que le masque marchait — et c'est le seuil qui etait
# faux. On voyait meme la courbe du coin juste au-dessus, a 95, 186, 187.
#
# Un fond d'ecran tres sombre et une fenetre noire ne se distinguent que par
# le zero absolu. Minimum releve sur le papier peint : 19.
NOIR = 12
COTE = 5
angles = (("haut-gauche", px0, py0),
          ("haut-droit", px0 + pl - COTE, py0),
          ("bas-gauche", px0, py0 + ph - COTE),
          ("bas-droit", px0 + pl - COTE, py0 + ph - COTE))

fautifs = []
for nom, ax, ay in angles:
    if ax < 0 or ay < 0 or ax + COTE > L or ay + COTE > H:
        print("son: coin %s hors de l ecran, non mesure" % nom)
        continue
    noirs = sum(1 for dy in range(COTE) for dx in range(COTE)
                if somme(ax + dx, ay + dy) < NOIR)
    print("son: coin %-12s %2d pixels noirs sur %d" % (nom, noirs, COTE * COTE))
    if noirs >= 3:
        fautifs.append(nom)

if fautifs:
    print("son: le masque de forme ne s'applique plus : " + ", ".join(fautifs))
else:
    print("son: les quatre coins sont arrondis, aucun carre noir")
COINS
  else
    echo "son: le panneau n a pas dit ou il est, coins non mesures"
  fi

  AVANT_SON=$(grep -ac 'son ferme' "$RUNNER_TEMP/serial.log" 2>/dev/null | head -1)
  AVANT_SON=${AVANT_SON:-0}
  python3 scripts/ci-screen.py clic "$RUNNER_TEMP/qmp.sock" \
      "$(( LARGEUR - 120 ))" 90 "$LARGEUR" "$HAUTEUR" || true
  sleep 6
  python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/ecran-son-ferme.ppm" || true
  APRES_SON=$(grep -ac 'son ferme' "$RUNNER_TEMP/serial.log" 2>/dev/null | head -1)
  APRES_SON=${APRES_SON:-0}
  if [ "$APRES_SON" -gt "$AVANT_SON" ]; then
    echo "son: un clic a cote a referme le panneau"
  else
    # EXIGE depuis le 25 septembre : vu vert quatre fois (36118645589,
  # 36122009013, 36125176677, 36127986281). C'est la demande de Grenofar mot
  # pour mot, et elle a deja echoue une fois pour de vraies raisons — l'attrape
  # du pointeur qui ne prenait jamais.
  echo "::error::Un clic a cote n a pas referme le panneau de son."
  exit 1
  fi

  python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey esc" || true
  sleep 2
else
  echo "son: la barre n a pas dit ou est son bouton"
fi

echo "--- l explorateur montre-t-il les disques ---"
# Grenofar : « est-ce qu'on peut les voir sur l'explorateur ? »
#
# Les Reglages disent « disques vus par les Reglages : 2 » a chaque
# construction, et c'est prouve. L'explorateur, lui, n'avait JAMAIS ete ouvert
# ici — on ouvre la barre, le bureau, le menu, les taches, GrenPlace, les
# Reglages, les Jeux et le panneau de son, mais pas lui.
#
# Tout est pourtant configure pour : `side_pane_mode=places`,
# `places_computer=1`, et surtout `places_unmounted=1`, qui fait apparaitre un
# disque AVANT meme qu'il soit monte. Plus udisks2, gvfs et polkitd pour que
# cliquer dessus suffise, sans mot de passe.
#
# « Configure pour » n'est pas « le fait ». C'est exactement la forme de tous
# les defauts de la semaine, et la seule facon de trancher est de regarder.
#
# Super+E, comme sous Windows : le raccourci existe deja dans openbox, on ne
# l'invente pas pour l'essai.
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey meta_l-e" || true
sleep 12
python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/ecran-explorateur.ppm" || true
echo "explorateur: capture prise — le panneau « Emplacements » est a regarder"

echo "--- les reglages s ouvrent-ils ---"
# C'est la porte des mises a jour, du son et du mot de passe, et
# personne ne l'avait jamais ouverte ici. Huit pages construites,
# c'est huit occasions qu'une exception passe inapercue.
#
# Meta+I, comme sous Windows : le raccourci existe deja dans
# openbox, on ne l'invente pas pour le test.
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey meta_l-i" || true
sleep 18
python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/ecran-reglages.ppm" || true
if grep -aq 'reglages ouvert' "$RUNNER_TEMP/serial.log"; then
  grep -a 'reglages ouvert' "$RUNNER_TEMP/serial.log" | tail -1 | tr -d '[:cntrl:]' | sed 's/^.*grenos: //'
else
  echo "reglages: ils ne se sont pas annonces (test non bloquant, a affiner)"
fi

echo "--- l installateur s ouvre-t-il vraiment ---"
# Des trois choses qui bloquent « l'installation sur disque, menee a son
# terme », celle-ci est a notre portee : le LANCEMENT.
#
# On ne prouvait jusqu'ici que la reponse de polkit (`pkcheck : autorise sans
# mot de passe`). C'est necessaire et loin d'etre suffisant : pkexec peut
# autoriser et Calamares refuser de demarrer — une configuration illisible, un
# module manquant, un theme Qt absent. Exactement la meme distance qu'entre
# « la configuration est la » et « l'installateur propose tout le disque »,
# qui nous a deja coute une image.
#
# On l'ouvre comme le ferait Grenofar : le menu, son nom, Entree. Il est hors
# de la liste par defaut — un seul chemin pour installer — mais la RECHERCHE
# montre les entrees cachees, et c'est ce que fait quelqu'un qui le cherche.
#
# `sendkey q` donne un « a » sur un clavier AZERTY : les touches portent la
# position QWERTY. « install » s'ecrit donc i n s t q l l. On le savait dans
# l'autre sens depuis GrenPlace ; c'est la premiere fois qu'on s'en sert pour
# TAPER un accent de disposition.
#
# On n'y clique RIEN. Ouvrir suffit : un clic de trop dans un installateur est
# la seule chose de ce parcours qui puisse effacer un disque.
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey esc" || true
sleep 2
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey ctrl-esc" || true
sleep 4
for touche in i n s t q l l ret; do
  python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey $touche" || true
  sleep 1
done
# Calamares charge Qt et ses trente modules : il lui faut du temps.
sleep 30
python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/ecran-installateur.ppm" || true
echo "installateur: capture prise — la fenetre est a regarder"
# Et on le referme, pour ne pas le laisser devant les captures suivantes.
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey alt-f4" || true
sleep 3
python3 scripts/ci-screen.py send "$RUNNER_TEMP/monitor.sock" "sendkey ret" || true
sleep 2

grep -a 'grenos:' "$RUNNER_TEMP/serial.log" | tail -n 24 || true

if ! cp "$RUNNER_TEMP/ecran-5min.ppm" "$RUNNER_TEMP/final.ppm" 2>/dev/null; then
  python3 scripts/ci-screen.py grab "$RUNNER_TEMP/monitor.sock" "$RUNNER_TEMP/final.ppm" || true
fi
python3 scripts/ci-screen.py judge "$RUNNER_TEMP/final.ppm" "$RUNNER_TEMP/final.png" | tee "$RUNNER_TEMP/ecran.log"
kill $QEMU 2>/dev/null || true
tail -n 40 "$RUNNER_TEMP/serial.log" || true
# La preuve qui ne trompe pas : le bureau ecrit une ligne sur le port
# serie quand il s'ouvre. Une image ou personne ne peut se connecter
# affiche un bel ecran de connexion et n'ecrit rien.
if grep -q 'session de bureau ouverte' "$RUNNER_TEMP/serial.log"; then
  echo "session: le bureau s'est ouvert"
else
  echo "::error::Aucune session de bureau : personne ne peut entrer dans cette image."
  exit 1
fi
# Le bureau a-t-il obei ? Trois lignes, et chacune prouve autre chose :
# la barre a demarre, son menu s'ouvre a la demande, et une fenetre
# s'ouvre au clavier. Une image ou l'une manque est une image ou
# quelque chose ne repond pas.
MUET=0
for attendu in "barre prete" "menu ouvert" "taches ouvert"; do
  if grep -q "grenos: $attendu" "$RUNNER_TEMP/serial.log"; then
    echo "repond: $attendu"
  else
    echo "::error::Le bureau n'a pas repondu : « $attendu » n'est jamais venu."
    MUET=1
  fi
done
# Le clic de souris devient obligatoire : vu vert trois fois
# (36018497394, 36023450708, 36047160557). La regle tenue depuis le
# debut est qu'une etape ne devient exigee qu'apres avoir ete vue
# verte — et une barre qui tomberait au premier clic passerait au
# vert sans lui, puisque tout le reste se fait au clavier.
if [ "$CLIC" != 1 ]; then
  echo "::error::Le clic sur le bouton grenOS n'ouvre pas le menu : la barre ne repond pas a la souris."
  MUET=1
fi

# Ce qui a ete vu vert plusieurs fois devient exige. La regle n'a pas
# change : une etape ne devient obligatoire qu'apres avoir ete vue
# verte — mais une fois qu'elle l'a ete, la laisser facultative
# reviendrait a publier une image ou le magasin ne s'ouvre plus.
for preuve in "grenplace ouvert" "maj: depot grenOS configure" "maj: trousseau du depot present" "maj: grenos-desktop" "disques : " "persistance : active" "son ouvert" "reglages ouvert" "jeux ouvert"; do
  if grep -aq "grenos: $preuve" "$RUNNER_TEMP/serial.log"; then
    echo "prouve: $preuve"
  else
    echo "::error::L'image n'a jamais dit « $preuve »."
    MUET=1
  fi
done
if grep -aq 'grenos: disques : aucun' "$RUNNER_TEMP/serial.log"; then
  echo "::error::Le systeme ne voit aucun disque : l'installateur n'aurait rien a proposer."
  MUET=1
fi
# Le clavier ne se contente pas d'exister : il doit etre celui qu'on a
# choisi. La session dit « X en place alors que Y a ete choisi » quand
# les deux different, et cette phrase-la est un echec.
if grep -aq 'grenos: clavier :.*alors que' "$RUNNER_TEMP/serial.log"; then
  grep -a 'grenos: clavier :' "$RUNNER_TEMP/serial.log" | tail -1 | tr -d '[:cntrl:]' | sed 's/^.*grenos: //'
  echo "::error::Le clavier choisi au premier ecran n'est pas celui que la session utilise."
  MUET=1
fi
if [ "$MUET" = 1 ]; then
  exit 1
fi

echo "--- les fenetres tiennent-elles dans l ecran ---"
# Grenofar a demarre grenOS en 1280x720 : « il est casse, on voit pas
# tout ». Il avait raison — les Reglages demandaient 660 pixels de
# haut, la page Jeux et GrenPlace 720, et sous la barre il n'en reste
# que 668. Les boutons du bas tombaient hors de l'ecran.
#
# La machine d'essai demarre desormais en 720p, et chaque fenetre dit
# sa taille REELLE une fois affichee — demander une taille n'est pas
# l'obtenir, GTK agrandit des qu'un enfant exige plus de place.
grep -a 'grenos: fenetre ' "$RUNNER_TEMP/serial.log" | tr -d '[:cntrl:]' | sed 's/^.*grenos: //' | sort -u
#
# EXIGE depuis le 25 septembre : vu vert trois fois (36106248502, 36112644255,
# 36115432806). Le laisser facultatif reviendrait a pouvoir publier une image
# ou l'interface est de nouveau cassee en 720p — le defaut meme qu'il a
# signale. Une etape ne devient obligatoire qu'apres avoir ete vue verte ; elle
# l'a ete.
DEBORDE=$(grep -ac 'grenos: fenetre .*DEBORDE' "$RUNNER_TEMP/serial.log" || true)
DEBORDE=$(echo "$DEBORDE" | head -1)
if [ "${DEBORDE:-0}" -gt 0 ]; then
  grep -a 'grenos: fenetre .*DEBORDE' "$RUNNER_TEMP/serial.log" | tr -d '[:cntrl:]' | sed 's/^.*grenos: //'
  echo "::error::$DEBORDE fenetre(s) depassent l ecran en 720p."
  exit 1
fi
# Et il faut qu'au moins une fenetre ait parle : zero ligne « fenetre »
# signifierait que le controle lui-meme est tombe, pas que tout va bien.
VUES=$(grep -ac 'grenos: fenetre ' "$RUNNER_TEMP/serial.log" || true)
VUES=$(echo "$VUES" | head -1)
if [ "${VUES:-0}" -lt 3 ]; then
  echo "::error::Seules ${VUES:-0} fenetres ont dit leur taille : le controle du 720p ne juge rien."
  exit 1
fi
echo "aucune fenetre ne deborde en 720p (${VUES} fenetres mesurees)"

echo "--- le son pourrait-il seulement sortir ---"
# « son ouvert, 1 sorties » prouve que PipeWire repond, pas qu'on entend
# quelque chose. Sur une carte SOF ou ACP — presque tous les portables depuis
# 2019 — ALSA a besoin des profils UCM pour savoir par ou sortir. Ils
# manquaient, et QEMU ne pouvait pas le montrer : il emule une carte ancienne
# qui n'en a pas besoin.
grep -a 'grenos: son : ' "$RUNNER_TEMP/serial.log" | tr -d '[:cntrl:]' | sed 's/^.*grenos: //' | sort -u || true
if grep -aq 'grenos: son : AUCUN profil ALSA' "$RUNNER_TEMP/serial.log"; then
  echo "::warning::Aucun profil ALSA : muet sur toute carte recente."
elif grep -aq 'grenos: son : [0-9]* profils ALSA' "$RUNNER_TEMP/serial.log"; then
  echo "son: les profils ALSA sont la"
fi

echo "--- combien de disques la machine voit-elle ---"
# QEMU lui en donne deux : la persistance de 8 Go et le disque vierge de 20 Go.
# `lsblk` en comptait trois — il ajoute le lecteur de disquette que QEMU
# invente —, donc il ne prouvait pas ce que la personne lit. Les Reglages
# repondent par la fonction qu'ils emploient eux-memes.
grep -a 'grenos: disques vus par les Reglages' "$RUNNER_TEMP/serial.log" | tail -1 | tr -d '[:cntrl:]' | sed 's/^.*grenos: //' || true
VUS=$(grep -a 'grenos: disques vus par les Reglages : ' "$RUNNER_TEMP/serial.log" | tail -1 | tr -d '[:cntrl:]' | sed 's/.*Reglages : //' | cut -d' ' -f1)
if [ "${VUS:-0}" -ge 2 ] 2>/dev/null; then
  echo "disques: la machine en voit ${VUS}, comme QEMU lui en donne"
else
  echo "::warning::Les Reglages ne voient que ${VUS:-0} disque(s) alors que la machine en a deux."
fi

echo "--- le magasin peut-il vraiment installer ---"
# La CI ouvrait GrenPlace et comptait ses applications. Elle n'a jamais appuye
# sur « Installer » — et c'est exactement par la que le defaut du 25 septembre
# est passe : sans certificats racines, flatpak ne pouvait pas verifier
# Flathub, et AUCUNE application ne s'installait. Grenofar l'a trouve en une
# minute d'usage ; la CI regardait une vitrine sans jamais entrer.
grep -a 'grenos: magasin :' "$RUNNER_TEMP/serial.log" | tail -1 | tr -d '[:cntrl:]' | sed 's/^.*grenos: //' || true
if grep -aq "grenos: magasin : Flathub repond" "$RUNNER_TEMP/serial.log"; then
  echo "magasin: une application peut s installer"
else
  echo "::warning::Flathub ne repond pas : le magasin ne pourra rien installer."
fi

echo "--- l installateur pourrait-il seulement demarrer ---"
# « Installer grenOS » lance `pkexec calamares`. Notre regle polkit a longtemps
# nomme une action qui n'existe pas, et polkit demandait donc un mot de passe
# qu'une machine grenOS par defaut n'a pas : le bouton ne pouvait pas marcher.
# La session pose la question a polkit avec `pkcheck`, sans rien lancer.
# Rapporte, pas encore bloquant : vu vert zero fois pour l'instant.
grep -a 'grenos: installateur :' "$RUNNER_TEMP/serial.log" | tail -1 | tr -d '[:cntrl:]' | sed 's/^.*grenos: //' || true
if grep -aq 'grenos: installateur : autorise sans mot de passe' "$RUNNER_TEMP/serial.log"; then
  echo "installateur: le bouton pourra demarrer sans mot de passe"
else
  echo "::warning::polkit n autorise pas l installateur sans mot de passe."
fi

# Cette image peut-elle se mettre a jour ? L'essai en conteneur prouve
# que le depot marche ; il ne prouve pas que l'IMAGE le connait. Le
# depot arrive par config/archives/grenos.list.binary, et personne
# n'avait jamais verifie qu'il atteignait le systeme demarre. Sans
# lui, « Mettre a jour grenOS » n'apporte que les correctifs Debian.
# Rapporte, pas encore bloquant : une etape ne devient obligatoire
# qu'apres avoir ete vue verte.
echo "--- y a-t-il un disque ou s installer ---"
# QEMU lui donne un disque SATA de 20 Go. Si le systeme ne le voit
# pas, l'installateur n'aurait rien a proposer — et on l'apprendrait
# devant l'ecran de Calamares, sur la machine de quelqu'un.
if ! grep -a 'grenos: disques' "$RUNNER_TEMP/serial.log" | tail -1 | tr -d '[:cntrl:]' | sed 's/^.*grenos: //'; then
  echo "disques : l image n a rien dit"
fi

echo "--- ce que la personne ecrit est-il garde ---"
# La promesse de la machine VirtualBox, jamais verifiee : la machine
# d'essai demarrait sans disque, donc live-boot ne trouvait aucune
# persistance et continuait. Elle demarre maintenant sur le disque
# qu'on livre.
if ! grep -a 'grenos: persistance' "$RUNNER_TEMP/serial.log" | tail -1 | tr -d '[:cntrl:]' | sed 's/^.*grenos: //'; then
  echo "persistance : l image n a rien dit"
fi

echo "--- le clavier choisi est-il celui en place ---"
# Le premier ecran ecrit /etc/default/keyboard, mais le service qui
# le lit a peut-etre deja tourne au demarrage. Si la session garde un
# clavier americain alors que la France a ete choisie, la personne
# s'en apercoit a la premiere apostrophe. Rapporte, pas encore
# bloquant.
if ! grep -a 'grenos: clavier :' "$RUNNER_TEMP/serial.log" | tail -1 | tr -d '[:cntrl:]' | sed 's/^.*grenos: //'; then
  echo "clavier : l image n a rien dit"
fi

echo "--- a-t-on ENTENDU quelque chose ---"
# Jusqu'ici on comptait des profils ALSA et des sorties PipeWire. C'est compter
# des tuyaux, pas ecouter l'eau : une pile impeccable peut rester muette, et
# c'est exactement ce qui est arrive a Grenofar.
#
# QEMU ecrit dans un WAV ce que la carte emet. La session joue un son apres
# avoir reveille PipeWire ; si les echantillons sont tous a zero, rien n'est
# sorti. Ce controle RAPPORTE, il ne bloque pas encore : on ne rend une etape
# obligatoire qu'apres l'avoir vue verte, et celle-ci n'a jamais tourne.
python3 - "$RUNNER_TEMP/son.wav" <<'ECOUTER'
import os
import struct
import sys

chemin = sys.argv[1]
if not os.path.exists(chemin):
    # Absence de fichier = absence de preuve, et non preuve d'absence de
    # probleme. Tant que ce controle etait facultatif, sortir en 0 ici etait
    # tolerable ; maintenant qu'il bloque, ce serait un garde sans dents —
    # une option `-audiodev` mal ecrite le rendrait muet, et il dirait oui.
    print("::error::QEMU n'a ecrit aucun fichier de son : la carte n'a jamais"
          " joue, ou l'option -audiodev est fautive.")
    raise SystemExit(1)

brut = open(chemin, "rb").read()
# QEMU ecrit un entete WAV de 44 octets, et n en corrige la longueur qu en se
# fermant proprement. On ne lit donc pas les champs de longueur : on prend
# tout ce qui suit l entete comme des echantillons 16 bits signes.
donnees = brut[44:]
if len(donnees) < 4:
    print("son : le fichier est vide (%d octets) — rien n est sorti" % len(brut))
    raise SystemExit(0)

pics = 0
crete = 0
paires = len(donnees) // 2
echantillons = struct.unpack("<%dh" % paires, donnees[:paires * 2])
for valeur in echantillons:
    valeur = abs(valeur)
    if valeur > crete:
        crete = valeur
    if valeur > 512:
        pics += 1


secondes = paires / 2.0 / 44100.0
print("son : %.1f s enregistrees, crete %d sur 32767, %d echantillons audibles"
      % (secondes, crete, pics))

# LES TROUS, et c'est la seule chose qui ressemble a ce que Grenofar entend.
#
# « On entend des bom bom bom quand y'a la video » : le son sort, il hache. Un
# hachement est un TROU — le tampon de PipeWire se vide, la carte joue du vide
# pendant quelques millisecondes, et on l'entend comme un a-coup.
#
# Compter la crete et les echantillons audibles prouve qu'un son est sorti. Ce
# controle-la etait vert a chaque passage, et il ne pouvait pas voir un hoquet :
# un son hache reste un son fort. Il fallait regarder AILLEURS dans le meme
# fichier — entre les echantillons, pas leur somme.
#
# On ne regarde qu'a l'interieur de la partie sonore : le silence avant et
# apres la lecture est normal, et le compter donnerait un chiffre affolant qui
# ne voudrait rien dire.
#
# DEUX lectures, pas une, et c'est tout l'objet du changement du 27 septembre.
#
# La session joue le meme son une premiere fois au repos, puis une seconde
# fois avec tous les coeurs satures — le second essai n'a lieu que sur cette
# machine, reconnue par un marqueur pose dans son disque de persistance. Il
# faut donc DECOUPER le fichier : sans cela, les trois secondes de silence
# entre les deux lectures compteraient pour un trou geant, et le controle
# dirait le contraire de la verite.
#
# Ce que la comparaison donne, et qu'aucun chiffre isole ne donnait : « propre
# au repos, hache sous charge » est un diagnostic ; « 0 trou » tout seul ne
# repond pas a la question qu'il a posee.
SEUIL = 256
FORTS = [i for i, v in enumerate(echantillons) if abs(v) > SEUIL]
# Un trou de plus de 3 ms au milieu du son : a 44,1 kHz stereo, cela fait
# environ 260 echantillons. En dessous, c'est le silence naturel entre deux
# oscillations et non un tampon vide.
MINIMUM = 260
# Un demi-silence de 0,4 s separe deux LECTURES, pas deux oscillations.
SEPARATION = int(0.4 * 44100 * 2)


def ms(entrees):
    return entrees / 2.0 / 44100.0 * 1000.0


regions = []
if FORTS:
    debut = FORTS[0]
    precedent = FORTS[0]
    for position in FORTS[1:]:
        if position - precedent > SEPARATION:
            regions.append((debut, precedent))
            debut = position
        precedent = position
    regions.append((debut, precedent))

# Une region de moins de 100 ms est un clic, pas une lecture.
regions = [r for r in regions if ms(r[1] - r[0]) > 100]

NOMS = ["au repos", "sous charge"]
resume = []
for rang, (depart, fin) in enumerate(regions):
    dedans = [i for i in FORTS if depart <= i <= fin]
    trous = 0
    plus_long = 0
    precedent = dedans[0]
    for position in dedans[1:]:
        ecart = position - precedent
        if ecart > MINIMUM:
            trous += 1
            if ecart > plus_long:
                plus_long = ecart
        precedent = position
    nom = NOMS[rang] if rang < len(NOMS) else "lecture %d" % (rang + 1)
    resume.append((nom, trous, plus_long))
    print("son : %s — %.0f ms, %d trou(s), le plus long %.0f ms"
          % (nom, ms(fin - depart), trous, ms(plus_long)))

if len(resume) < 2:
    # Rapporte, pas bloquant : le second essai vient d'etre ecrit, et une
    # etape ne devient obligatoire qu'apres avoir ete vue verte.
    print("son : une seule lecture entendue — la mesure sous charge n a pas eu"
          " lieu, le marqueur ou le second paplay a manque")
else:
    repos, charge = resume[0], resume[1]
    if charge[1] > repos[1]:
        print("::warning::Le son hache SOUS CHARGE : %d trou(s) contre %d au"
              " repos, le plus long %.0f ms. C est le defaut que Grenofar"
              " entend." % (charge[1], repos[1], ms(charge[2])))
    else:
        print("son : sous charge comme au repos, %d trou(s) — le tampon tient"
              % charge[1])
if crete == 0 or pics < 100:
    # EXIGE depuis le 26 septembre au soir : vu vert cinq fois de suite
    # (36249052722, 36252138095, 36256203829, 36258213058, 36260754848), avec
    # des cretes de 9568 a 15422 et plus de quarante mille echantillons a
    # chaque fois. La regle n'a pas change — une etape ne devient obligatoire
    # qu'apres avoir ete vue verte — mais une fois qu'elle l'a ete, la laisser
    # facultative reviendrait a publier une image muette sans le savoir.
    #
    # C'est le reproche numero un de Grenofar, et le seul qu'on n'ait jamais
    # pu prouver autrement qu'en comptant des sorties PipeWire, c'est-a-dire
    # en comptant des tuyaux.
    print("::error::Aucun son n'est sorti : crete %d, %d echantillons audibles."
          % (crete, pics))
    raise SystemExit(1)
print("son : grenOS a ete ENTENDU")
ECOUTER

echo "--- cette image peut-elle se mettre a jour ---"
if ! grep -a 'grenos: maj:' "$RUNNER_TEMP/serial.log" | sed 's/^.*grenos: //'; then
  echo "maj: l image n a rien dit du tout"
fi

# Un bureau couvre son fond de fenetres, d'icones et d'un panneau :
# sa couleur dominante ne depasse jamais 70 % de l'ecran. Un menu de
# demarrage, lui, est noir a 90 %.
PART=$(sed -n 's/.*covers \([0-9]*\)%.*/\1/p' "$RUNNER_TEMP/ecran.log" | head -1)
echo "couleur dominante : ${PART:-?} %"
if [ -z "$PART" ] || [ "$PART" -gt 70 ]; then
  echo "::error::L'ecran n'est pas un bureau : la couleur dominante couvre ${PART:-?} % apres cinq minutes."
  exit 1
fi
