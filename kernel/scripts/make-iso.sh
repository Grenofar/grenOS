#!/usr/bin/env bash
set -euo pipefail

# Directory where we are
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
KERNEL_DIR="$SCRIPT_DIR/.."
ISO_DIR="$KERNEL_DIR/iso_root"
LIMINE_DIR="$KERNEL_DIR/limine"
LIMINE_REPO="https://github.com/limine-bootloader/limine.git"
LIMINE_BRANCH="v10.x-binary"

# Fetch Limine if not present
if [ ! -d "$LIMINE_DIR" ]; then
    git clone --depth=1 --branch=$LIMINE_BRANCH $LIMINE_REPO $LIMINE_DIR
    make -C $LIMINE_DIR
fi

# Create ISO root directory
rm -rf $ISO_DIR
mkdir -p $ISO_DIR/boot/limine

# Copy the kernel
cp $KERNEL_DIR/target/x86_64-unknown-none/release/kernel $ISO_DIR/boot/kernel.el

# Copy limine.conf
cp $KERNEL_DIR/limine.conf $ISO_DIR/boot/limine.conf

# Copy Limine binaries
cp $LIMINE_DIR/bios.img $ISO_DIR/boot/limine/
cp $LIMINE_DIR/limine-bios.sys $ISO_DIR/boot/limine/
cp $LIMINE_DIR/limine-bios-cd.bin $ISO_DIR/boot/limine/
cp $LIMINE_DIR/limine-bios-eltorito.elito $ISO_DIR/boot/limine/
cp $LIMINE_DIR/limine-uefi-cd.bin $ISO_DIR/boot/limine/
cp $LIMINE_DIR/BOOTX64.EFI $ISO_DIR/boot/limine/
cp $LIMINE_DIR/BOOTIA32.EFI $ISO_DIR/boot/limine/

# Create the ISO
xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table \
    -eltorito-alt-boot -e boot/limine/limine-uefi-cd.bin \
    -no-emul-boot -isohybrid-gpt-baset -isohybrid-apm-hfsplus \
    -o $KERNEL_DIR/kernel.iso $ISO_DIR

# Make it bootable with Limine
$LIMINE_DIR/limine bios-install $KERNEL_DIR/kernel.iso

echo "ISO created at $KERNEL_DIR/kernel.iso"
