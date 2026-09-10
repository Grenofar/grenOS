#!/bin/sh
set -e

# Build the kernel
cargo build --release --target x86_64-unknown-none

# Create ISO directory structure
rm -rf iso_root
mkdir -p iso_root/boot iso_root/EFI/BOOT

# Copy kernel and limine files
cp target/x86_64-unknown-none/release/grenos-kernel iso_root/boot/
cp limine.conf iso_root/boot/
cp -r /usr/share/limine/limine-bios.sys /usr/share/limine/limine-bios-cd.bin /usr/share/limine/limine-uefi-cd.bin iso_root/boot/limine/ 2>/dev/null || true

# Create the ISO
xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table \
    --efi-boot boot/limine/limine-uefi-cd.bin \
    -efi-boot-part --efi-boot-image --protective-msdos-label \
    iso_root -o grenos.iso

# Deploy Limine
limine bios-install grenos.iso

echo "ISO created: grenos.iso"
