#!/bin/bash
set -euo pipefail

# Determine the kernel directory (where this script is located, then go up one)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KERNEL_DIR="$(dirname "$SCRIPT_DIR")"
cd "$KERNEL_DIR"

# Build the kernel
cargo build --release

# Create ISO root directory
rm -rf iso_root
mkdir -p iso_root/boot/limine

# Copy kernel and limine.conf
cp target/x86_64-unknown-none/release/kernel iso_root/boot/kernel
cp limine.conf iso_root/boot/limine/limine.conf

# Fetch Limine binaries (if not already present)
if [ ! -d "limine" ]; then
    git clone https://github.com/limine-bootloader/limine.git --branch=v10.x-binary --depth=1
fi
make -C limine

# Copy Limine files
cp limine/limine-bios.sys limine/limine-bios-cd.bin limine/limine-uefi-cd.bin iso_root/boot/limine/

# Create the ISO
xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table \
    --efi-boot boot/limine/limine-uefi-cd.bin \
    -efi-boot-part --efi-boot-image --protective-msdos-label \
    iso_root -o grenos.iso

# Install Limine
./limine/limine bios-install grenos.iso

echo "ISO created: grenos.iso"
