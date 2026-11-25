#!/bin/sh
# Les trois programmes Rust de l'image — grenos-ui, grenos-coins, et la barre,
# le bureau et le panneau du son natifs — compilés dans Debian trixie et posés
# dans includes.chroot.
#
#   scripts/construire-binaires.sh            essais puis compilation
#   scripts/construire-binaires.sh sans-essais
#
# UN SEUL ENDROIT pour deux constructions (18 novembre, K3) : la construction
# de tous les jours (linux.yml) et la preuve de reproductibilité
# (reproductible.yml) doivent compiler EXACTEMENT de la même façon, sinon la
# preuve porterait sur autre chose que ce qu'on publie.
#
# La version de Rust est FIGÉE (`RUST`) : `rust:1-trixie` change à chaque
# version de Rust, et un autre compilateur donne un autre binaire. `--locked`
# refuse de compiler si Cargo.lock n'est pas à jour : les dépendances sont
# celles du dépôt, pas les dernières publiées.
set -eu

RUST=rust:1.100.0-trixie
ESSAIS=1
[ "${1:-}" = "sans-essais" ] && ESSAIS=0

docker run --rm -v "$PWD:/work" -w /work \
    -e CARGO_TERM_COLOR=never -e ESSAIS="$ESSAIS" \
    -e SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-}" \
    "$RUST" sh -ec '
      apt-get update -qq
      apt-get install -y -qq --no-install-recommends \
        libwebkit2gtk-4.1-dev libxdo-dev libssl-dev librsvg2-dev \
        build-essential pkg-config >/dev/null
      # grenos-ui (Tauri), grenos-coins (x11rb, le démon qui arrondit les
      # fenêtres d openbox), linux/natif (gtk-rs : barre, bureau, son),
      # linux/adaptateur-xbox (le pilote de la clé sans fil Xbox, GPL).
      for caisse in ui coins natif adaptateur-xbox; do
        cd /work/linux/$caisse
        [ "$ESSAIS" = 1 ] && cargo test --release --locked
        cargo build --release --locked
      done
    '

sudo install -m 0755 linux/ui/target/release/grenos-ui \
    linux/config/includes.chroot/usr/bin/grenos-ui
sudo install -m 0755 linux/coins/target/release/grenos-coins \
    linux/config/includes.chroot/usr/bin/grenos-coins
# Dans /usr/lib/grenos, pas sur le PATH : le point d'entrée reste le programme
# Python, qui passe la main (natif.py).
for prog in grenos-shell-natif grenos-bureau-natif grenos-son-natif grenos-presse-natif; do
    sudo install -m 0755 "linux/natif/target/release/$prog" \
        "linux/config/includes.chroot/usr/lib/grenos/$prog"
done
# Le pilote de l'adaptateur sans fil Xbox (25 novembre) : lancé par udev.
sudo install -m 0755 linux/adaptateur-xbox/target/release/grenos-adaptateur-xbox \
    linux/config/includes.chroot/usr/lib/grenos/grenos-adaptateur-xbox
ls -l linux/config/includes.chroot/usr/bin/grenos-ui \
    linux/config/includes.chroot/usr/bin/grenos-coins \
    linux/config/includes.chroot/usr/lib/grenos/grenos-*-natif \
    linux/config/includes.chroot/usr/lib/grenos/grenos-adaptateur-xbox
sha256sum linux/config/includes.chroot/usr/bin/grenos-ui \
    linux/config/includes.chroot/usr/bin/grenos-coins \
    linux/config/includes.chroot/usr/lib/grenos/grenos-*-natif \
    linux/config/includes.chroot/usr/lib/grenos/grenos-adaptateur-xbox
# La place compte : l'image en demande beaucoup plus loin.
sudo rm -rf linux/ui/target linux/coins/target linux/natif/target linux/adaptateur-xbox/target
docker image rm "$RUST" >/dev/null 2>&1 || true
