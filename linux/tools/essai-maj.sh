#!/bin/sh
# La mise à jour depuis grenOS fonctionne-t-elle vraiment ?
#
# On part d'une Debian nue, on ajoute notre dépôt exactement comme l'image le
# fait, et on installe `grenos-desktop`. Si cela échoue ici, aucune machine ne
# pourra se mettre à jour — et il vaut mieux le savoir en deux minutes que de
# le découvrir sur la machine de quelqu'un.
#
# Ce que cet essai prouve, et c'est précisément ce qui manquait :
#   - le dépôt est joignable, et sa signature est acceptée ;
#   - le paquet s'installe avec toutes ses dépendances, y compris les nouvelles ;
#   - les fichiers du bureau arrivent bien à leur place.
#
# Lancé dans un conteneur par la CI, avec DEPOT et le dépôt local monté.

set -eu

echo "--- une Debian nue, avec les memes sections que grenOS ---"
# Le conteneur n'a que « main ». Une machine grenOS a aussi contrib, non-free
# et les micrologiciels — c'est la que vivent le microcode du processeur et les
# pilotes des cartes Wi-Fi. Sans ces sections, l'essai echouerait sur des
# paquets qui, eux, s'installent tres bien sur une vraie machine : il serait
# alors moins fidele que ce qu'il pretend verifier.
cat > /etc/apt/sources.list <<'FIN'
deb http://deb.debian.org/debian trixie main contrib non-free non-free-firmware
deb http://deb.debian.org/debian trixie-updates main contrib non-free non-free-firmware
deb http://security.debian.org/debian-security trixie-security main contrib non-free non-free-firmware
FIN
rm -f /etc/apt/sources.list.d/debian.sources
apt-get update -qq
apt-get install -y -qq ca-certificates curl gnupg >/dev/null

echo "--- notre depot, comme dans l'image ---"
install -D -m 0644 linux/grenos-apt.gpg /usr/share/keyrings/grenos-apt.gpg
printf 'deb [signed-by=/usr/share/keyrings/grenos-apt.gpg] %s stable main\n' "$DEPOT" \
    > /etc/apt/sources.list.d/grenos.list
cat /etc/apt/sources.list.d/grenos.list

echo "--- lecture du depot ---"
# Sans `-o Acquire::Retries`, une propagation de cache un peu lente ferait
# echouer l'essai pour une mauvaise raison.
apt-get -o Acquire::Retries=5 update

echo "--- ce que le depot propose ---"
apt-cache policy grenos-desktop grenos-systeme

echo "--- installation ---"
# Les memes options que la mise a jour automatique : une question posee par
# dpkg bloquerait une installation sans personne devant l'ecran.
apt-get install -y --no-install-recommends \
    -o Dpkg::Options::=--force-confdef \
    -o Dpkg::Options::=--force-confold \
    grenos-desktop

echo "--- ce qui est arrive ---"
# Ce que l'on vérifie n'est pas seulement « des fichiers sont arrivés » : c'est
# qu'une mise à jour peut apporter une VRAIE nouveauté — un service qui
# n'existait pas, une entrée de menu, une icône. Sans eux, il faudrait regraver
# une image pour changer autre chose qu'un programme.
for fichier in /usr/bin/grenos-shell /usr/bin/grenos-maj /usr/bin/grenos-jeux \
               /usr/lib/grenos/grenosui.py /usr/share/grenos/catalogue.json \
               /usr/lib/grenos/appliquer-systeme \
               /etc/systemd/system/grenos-maj-verif.timer \
               /etc/systemd/system/grenos-maj-auto.service \
               /usr/share/applications/grenos-parametres.desktop \
               /usr/share/applications/grenos-jeux.desktop \
               /usr/share/icons/hicolor/256x256/apps/grenplace.png \
               /etc/xdg/openbox/rc.xml \
               /etc/polkit-1/rules.d/49-grenos-administration.rules; do
    if [ ! -e "$fichier" ]; then
        echo "manquant : $fichier" >&2
        exit 1
    fi
done
dpkg-query -W -f='${Package} ${Version} ${Status}\n' grenos-desktop grenos-systeme

echo "la mise a jour depuis grenOS fonctionne"
