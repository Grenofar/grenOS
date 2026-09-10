#!/bin/bash
set -e

# Run QEMU with the ISO
qemu-system-x86_64 -cdrom grenos.iso -serial stdio -display none -no-reboot
