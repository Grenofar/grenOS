#!/usr/bin/env bash
set -e

KERNEL_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )/.." && pwd )"
ISO_ROOT="$KERNEL_DIR/iso_root"
ISO_OUTPUT="$KERNEL_DIR/grenos.iso"
LIMINE_VERSION="v11.0"
LIMINE_TARBALL="limine-$LIMINE_VERSION.tar.gz"
LIMINE_URL="https://github.com/limine-bootloader/limine/releases/download/$LIMINE_VERSION/$LIMINE_TARBALL"
LIMINE_DIR="$KERNEL_DIR/limine"

# Build the kernel
cd "$KERNEL_DIR"
cargo build --release

# Prepare ISO root
rm -rf "$ISO_ROOT"
mkdir -p "$ISO_ROOT/boot"

# Copy kernel
cp "$KERNEL_DIR/target/x86_64-unknown-none/release/kernel" "$ISO_ROOT/boot/grenos"

# Create limine.cfg
cat > "$ISO_ROOT/boot/limine.cfg" <<'EOF'
TIMEOUT 20
PROTOCOL 1
DEFAULT linux

LABEL linux
    KERNEL /boot/grenos
EOF

# Download and extract Limine if not present
if [ ! -f "$LIMINE_DIR/limine" ]; then
    echo "Downloading Limine..."
    curl -L -o "$LIMINE_TARBALL" "$LIMINE_URL"
    tar -xzf "$LIMINE_TARBALL"
    mv "limine-$LIMINE_VERSION" "$LIMINE_DIR"
fi

# Make sure the limine binary is executable
chmod +x "$LIMINE_DIR/limine"

# Create the ISO with xorriso
xorriso -as mkisofs -b "$LIMINE_DIR/limine-bios-cd.bin" -no-emul-boot -boot-load-size 4 -boot-info-table -o "$ISO_OUTPUT" "$ISO_ROOT"

# Install Limine to make it bootable
"$LIMINE_DIR/limine" bios-install "$ISO_OUTPUT"

echo "ISO created at $ISO_OUTPUT"
