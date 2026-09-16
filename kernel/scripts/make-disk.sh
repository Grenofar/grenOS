#!/bin/bash
set -euo pipefail

# Determine the kernel directory (where this script is located, then go up one)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KERNEL_DIR="$(dirname "$SCRIPT_DIR")"
cd "$KERNEL_DIR"

# Build the kernel
cargo build --release

# Disk geometry and layout
SIZE_MIB=64
HEADS=64; SPT=32                     # 64 heads x 32 sectors = 2048 sectors = 1 MiB per cylinder
START=2048                           # first cylinder is left to the partition tables
SECTORS=$(( (SIZE_MIB - 2) * 2048 ))
END=$(( START + SECTORS - 1 ))
OFFSET=$(( START * 512 ))
IMG=grenos-disk.img

rm -f "$IMG"
dd if=/dev/zero bs=1M count=0 seek=$SIZE_MIB of="$IMG"
sgdisk "$IMG" -n 1:$START:$END -t 1:ef00 -m 1   # GPT laid out, converted to an MBR partition 1

# A random non-zero MBR disk signature at bytes 440..443, written BEFORE bios-install;
# check after bios-install that it did not change
SIGNATURE=$(( (RANDOM << 16) | RANDOM ))
printf '%08x' "$SIGNATURE" | xxd -r -p | dd of="$IMG" bs=1 seek=440 conv=notrunc

./limine/limine bios-install "$IMG"

# Verify the signature survived bios-install
SIGNATURE_AFTER=$(dd if="$IMG" bs=1 skip=440 count=4 2>/dev/null | xxd -p)
if [ "$SIGNATURE_AFTER" != "$(printf '%08x' "$SIGNATURE")" ]; then
    echo "ERROR: MBR disk signature changed after bios-install" >&2
    exit 1
fi

# Prepare the kernel slots, padded to exactly 8 MiB
truncate -s 8M target/x86_64-unknown-none/release/kernel
cp target/x86_64-unknown-none/release/kernel slot.bin

# Create the FAT32 file system and copy files
mformat -i "$IMG"@@$OFFSET -F -v GRENOS -T $SECTORS -h $HEADS -s $SPT ::
mmd  -i "$IMG"@@$OFFSET ::/EFI ::/EFI/BOOT ::/boot ::/boot/limine ::/grenos
mcopy -i "$IMG"@@$OFFSET slot.bin ::/boot/kernel-a
mcopy -i "$IMG"@@$OFFSET slot.bin ::/boot/kernel-b
mcopy -i "$IMG"@@$OFFSET limine-disk.conf ::/boot/limine/limine.conf
mcopy -i "$IMG"@@$OFFSET limine/limine-bios.sys ::/boot/limine
mcopy -i "$IMG"@@$OFFSET limine/BOOTX64.EFI limine/BOOTIA32.EFI ::/EFI/BOOT

# The write test file: 65536 zero bytes
dd if=/dev/zero bs=65536 count=1 of=essai.bin
mcopy -i "$IMG"@@$OFFSET essai.bin ::/grenos/essai.bin

# Optional command line for entry 1
if [ -n "${GRENOS_CMDLINE:-}" ]; then
    sed -i "s|    kernel_path: boot():/boot/kernel-a|    kernel_path: boot():/boot/kernel-a\n    cmdline: $GRENOS_CMDLINE|" limine-disk.conf
fi

rm -f slot.bin essai.bin

echo "Disk image created: $IMG"
