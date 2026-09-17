#!/bin/sh
# Fabrique le dépôt APT de grenOS à partir des paquets donnés : un dépôt n'est
# qu'un ensemble de fichiers statiques, donc il s'héberge n'importe où — chez
# nous, dans un bucket public Supabase.
#
#   linux/tools/make-apt-repo.sh <dossier de sortie> <paquet.deb>...
#
# Signé quand $APT_SECRET_KEY nomme un fichier de clé privée OpenPGP : apt
# refuse par défaut un dépôt qui n'est pas signé, et il a raison.

set -eu

OUT="$1"
shift

mkdir -p "$OUT/pool/main/g/grenos-desktop" "$OUT/dists/stable/main/binary-amd64"
for deb in "$@"; do
    cp "$deb" "$OUT/pool/main/g/grenos-desktop/"
done

cd "$OUT"
dpkg-scanpackages --multiversion pool > dists/stable/main/binary-amd64/Packages
gzip -9kf dists/stable/main/binary-amd64/Packages
: > dists/stable/Release

# Le fichier Release décrit le dépôt et porte les empreintes des index : c'est
# lui qui est signé, et c'est de lui que tout le reste tient.
{
    echo "Origin: grenOS"
    echo "Label: grenOS"
    echo "Suite: stable"
    echo "Codename: stable"
    echo "Architectures: amd64 all"
    echo "Components: main"
    echo "Description: Les paquets de grenOS, par-dessus Debian"
    echo "Date: $(date -Ru)"
} > dists/stable/Release

for hash in MD5Sum:md5sum SHA256:sha256sum; do
    field=${hash%%:*}
    tool=${hash##*:}
    echo "$field:" >> dists/stable/Release
    for file in dists/stable/main/binary-amd64/Packages dists/stable/main/binary-amd64/Packages.gz; do
        sum=$($tool "$file" | cut -d' ' -f1)
        size=$(stat -c %s "$file")
        printf ' %s %16s %s\n' "$sum" "$size" "${file#dists/stable/}" >> dists/stable/Release
    done
done

if [ -n "${APT_SECRET_KEY:-}" ] && [ -f "${APT_SECRET_KEY}" ]; then
    export GNUPGHOME=$(mktemp -d)
    chmod 700 "$GNUPGHOME"
    gpg --batch --quiet --import "$APT_SECRET_KEY"
    gpg --batch --yes --clearsign -o dists/stable/InRelease dists/stable/Release
    gpg --batch --yes --detach-sign --armor -o dists/stable/Release.gpg dists/stable/Release
    gpg --batch --armor --export > grenos-apt.asc
    gpg --batch --export > grenos-apt.gpg
    rm -rf "$GNUPGHOME"
    echo "depot signe"
else
    echo "ATTENTION : depot non signe (APT_SECRET_KEY absent)"
fi

find . -type f | sort
