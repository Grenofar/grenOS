#!/bin/bash
set -e

# Build the kernel
cargo build --release

# Create ISO directory structure
mkdir -p iso/boot/limine
cp target/x86_64-grenos/release/libgrenos_kernel.a iso/boot/kernel
cp limine.cfg iso/boot/limine/limine.cfg

# Copy Limine bootloader files (assuming limine is installed)
# For CI, we'll use a pre-built limine binary
if [ -f /usr/local/share/limine/limine-bios.sys ]; then
    cp /usr/local/share/limine/limine-bios.sys iso/boot/limine/
    cp /usr/local/share/limine/limine-bios-cd.bin iso/boot/limine/
    cp /usr/local/share/limine/limine-uefi-cd.bin iso/boot/limine/
else
    echo "Limine not found. Please install limine or provide the bootloader files."
    exit 1
fi

# Create ISO
xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table \
    --efi-boot boot/limine/limine-uefi-cd.bin \
    -efi-boot-part --efi-boot-image --protective-msdos-label \
    iso -o grenos.iso

# Clean up
rm -rf iso

echo "ISO created: grenos.iso"
