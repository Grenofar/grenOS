#!/bin/sh
# Le pilote NVIDIA officiel se compile-t-il pour le noyau de grenOS ?
#
# 20 novembre. Grenofar : « essaie de le faire en autonomie, lance dans un
# VPS ou comme tu veux ». Il n'y a pas de carte NVIDIA en intégration, mais le
# vrai risque n'est pas là : NVIDIA suit les noyaux neufs avec retard, et
# grenOS prend le noyau de trixie-backports (7.x). Si le module ne se
# compile pas pour lui, le bouton « Installer le pilote NVIDIA » échouerait
# chez tout le monde. Cet essai le compile, dans un Debian 13 propre, avec
# EXACTEMENT ce que grenOS branche : le même épinglage du noyau, la même clé,
# la même ligne de dépôt.
#
#   docker run --rm -v "$PWD:/work" -w /work debian:trixie sh linux/tools/essai-pilote-nvidia.sh
set -eu
export DEBIAN_FRONTEND=noninteractive
D=linux/config/includes.chroot/usr/share/grenos/depots

echo "--- le noyau de grenOS (trixie-backports, meme epinglage que l'image)"
echo 'deb http://deb.debian.org/debian trixie-backports main contrib non-free non-free-firmware' \
    > /etc/apt/sources.list.d/backports.list
cp linux/config/archives/grenos-noyau.pref.chroot /etc/apt/preferences.d/grenos-noyau.pref
apt-get update -q
apt-get install -y -q --no-install-recommends ca-certificates gnupg dkms kmod xz-utils zstd linux-headers-amd64 >/dev/null

echo "--- la cle de NVIDIA livree dans grenOS, et son empreinte"
lue=$(gpg --show-keys --with-colons "$D/nvidia-driver.gpg" 2>/dev/null | awk -F: '/^fpr/ {print $10; exit}')
attendue=$(tr -d ' \n' < "$D/nvidia-driver.empreinte")
[ "$lue" = "$attendue" ] || { echo "::error::cle NVIDIA : $lue, attendue $attendue"; exit 1; }
echo "cle verifiee : $lue"

echo "--- le depot de NVIDIA, branche comme grenos-depot-editeur le fait"
install -D -m 0644 "$D/nvidia-driver.gpg" /usr/share/keyrings/nvidia-driver-archive-keyring.gpg
install -D -m 0644 "$D/nvidia-driver.list" /etc/apt/sources.list.d/nvidia-driver.list
install -D -m 0644 "$D/nvidia-driver.pref" /etc/apt/preferences.d/nvidia-driver.pref
apt-get update -q
apt-cache policy nvidia-driver firmware-nvidia-gsp | grep -E '^[a-z]|Candidate'

noyau=$(ls /usr/src | sed -n 's/^linux-headers-\(.*-amd64\)$/\1/p' | sort -V | tail -n 1)
[ -n "$noyau" ] || { echo "::error::pas d'en-tetes de noyau"; exit 1; }
echo "--- la cle de signature des modules, creee comme grenos-depot-editeur le fait (Secure Boot)"
apt-get install -y -q --no-install-recommends openssl mokutil >/dev/null
install -d -m 0700 /var/lib/dkms
openssl req -new -x509 -nodes -days 36500 -subj "/CN=grenOS - signature des modules/"     -newkey rsa:2048 -keyout /var/lib/dkms/mok.key -outform DER -out /var/lib/dkms/mok.pub 2>/dev/null
echo "--- compilation du module pour le noyau $noyau"
apt-get install -y -q --no-install-recommends nvidia-kernel-open-dkms >/dev/null
version=$(dpkg-query -W -f='${Version}' nvidia-kernel-open-dkms | cut -d- -f1)
if ! dkms install "nvidia/$version" -k "$noyau"; then
    echo "::error::le module NVIDIA $version ne se compile pas pour le noyau $noyau"
    tail -n 60 /var/lib/dkms/nvidia/"$version"/build/make.log 2>/dev/null || true
    exit 1
fi
dkms status -m nvidia -k "$noyau"
dkms status -m nvidia -k "$noyau" | grep -q installed || { echo "::error::module non installe"; exit 1; }
find "/lib/modules/$noyau" -name 'nvidia*.ko*' | sed 's|.*/||' | sort
# AVEC LE SECURE BOOT, le module doit etre signe par NOTRE cle : sinon le
# firmware le refuse et l'ecran retombe sur l'image du BIOS.
ko=$(find "/lib/modules/$noyau" -name 'nvidia.ko*' | head -n 1)
case "$ko" in *.xz) xz -dkc "$ko" > /tmp/nvidia.ko; ko=/tmp/nvidia.ko ;; *.zst) zstd -dqc "$ko" > /tmp/nvidia.ko; ko=/tmp/nvidia.ko ;; esac
signataire=$(modinfo -F signer "$ko" 2>/dev/null || true)
echo "signataire du module : ${signataire:-AUCUN}"
case "$signataire" in
    *"grenOS - signature des modules"*) echo "module signe par la cle de grenOS : il se chargera une fois la cle inscrite (MOK)" ;;
    *) echo "::error::le module NVIDIA n'est pas signe par la cle de grenOS : avec le Secure Boot, il ne se chargerait pas"; exit 1 ;;
esac
echo "pilote NVIDIA $version : compile et installe pour le noyau $noyau"
