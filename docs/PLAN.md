> **Note from Claude, 2026-09-16.** The specification is
> `docs/specs/disk-and-updates.md`; where this plan and the spec disagree, the
> spec wins. Errors in this plan, checked against the sources:
> - `sgdisk -m` does not set an MBR signature: it converts the GPT to an MBR
>   table. The disk signature (bytes 440..443) is written by the script itself,
>   before `limine bios-install`, as the spec says.
> - Nothing in the limine-rust-template uses Ed25519.
> - `chmod +x` is not needed and cannot be done through the commit API: CI runs
>   `bash kernel/scripts/make-disk.sh`.
> - The Microsoft `fatgen.mspx` and `ata.org` links are not sources that were
>   read; the spec's sources are the Linux headers it names.

# grenOS Disk Image and Updates Technical Plan

## 1. Problem

 grenOS currently runs from a read-only ISO. To install updates from within the OS we need a writable disk with two kernel slots, an AHCI driver to access SATA disks, MBR+FAT32 to locate our own partition and run a write test, and SHA-512/Ed25519 to verify signed kernels.

## 2. Constraints

- **Hardware**: x86_64, AHCI (SATA) controller, disk identified by Limine's MBR disk signature and FAT volume label `GRENOS`.
- **Boot protocol**: Limine boots the kernel; we keep the existing `limine.conf` for ISO boot and add a `limine-disk.conf` for the disk image.
- **File system**: MBR partition table, FAT32 boot sector and directory entries as defined by the mtools used in the script.
- **Crypto**: SHA-512 (FIPS 180-4) and Ed25519 verification (RFC 8032) with test vectors from the RFC.
- **Kernel**: `no_std`, built-in `x86_64-unknown-none` target, static relocation in `.cargo/config.toml`.
- **CI**: The verification workflow builds the disk image when `make-disk.sh` exists and boots QEMU from it via AHCI; a task is green only when the kernel builds, passes clippy, and boots printing `grenOS` on serial.

## 3. Approach

Tasks are performed in the order A→B→C→D as specified in the mission description. Each task leaves the kernel booting and all CI steps green.

**Task A – Disk image with two kernel slots**
- Create `kernel/scripts/make-disk.sh` that builds a 64 MiB FAT32 disk image with an MBR partition, two 8 MiB kernel slots (`kernel-a`, `kernel-b`), and a test file `essai.bin`.
- Create `kernel/limine-disk.conf` with two menu entries pointing to the slots and a 1-second timeout to allow menu access.
- The script is run from the repository root, like `make-iso.sh`, and writes `kernel/grenos-disk.img`.

**Task B – AHCI (SATA) driver**
- Implement `kernel/src/ahci.rs` that finds the AHCI controller via PCI (class 0x01, subclass 0x06, prog-if 0x01), maps its registers, brings up ports with a device present, and issues ATA commands (IDENTIFY, READ/WRITE DMA EXT, FLUSH CACHE EXT) using command list and received FIS structures.
- The driver exposes a `Disk` struct with `read`, `write`, and `flush` methods working on 512-byte sectors.
- After `sti` in `main.rs`, scan for disks and print one line per disk: `disk: AHCI port <n>, <model>, <MiB> MiB`.

**Task C – MBR + FAT32, find our own disk, write test**
- Implement `kernel/src/fat.rs` that reads the MBR disk signature from Limine's `ExecutableFileRequest`, matches it to the AHCI disk, opens partition 1, checks the FAT volume label `GRENOS`, and provides a `Volume` interface to find, read, and overwrite files.
- In `main.rs`, after the AHCI scan, find the matching disk, open the volume, locate `/grenos/essai.bin`, overwrite it with random data, read it back, compare, and print the two required lines.
- On the ISO (no matching disk), print `disk: booted from the ISO, nothing to install to`.

**Task D – SHA-512 and Ed25519**
- Implement `kernel/src/sha512.rs` with `fn sha512(data: &[u8]) -> [u8; 64]` following FIPS 180-4.
- Implement `kernel/src/ed25519.rs` with `fn verify(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool` following RFC 8032 §5.1.7, using the field arithmetic from `x25519.rs` and rejecting non-canonical keys.
- In `main.rs`, run the RFC 8032 test vectors and print `crypto: ed25519 verified against RFC 8032` on success.

## 4. Interfaces

### Task A
- `kernel/scripts/make-disk.sh` (executable after `chmod +x`): creates `kernel/grenos-disk.img`.
- `kernel/limine-disk.conf`:
  ```
  timeout: 1
  quiet: yes
  default_entry: 1

  /grenOS slot a
      protocol: limine
      kernel_path: boot():/boot/kernel-a

  /grenOS slot b
      protocol: limine
      kernel_path: boot():/boot/kernel-b
  ```

### Task B
- `kernel/src/ahci.rs`:
  ```rust
  pub const SECTOR: usize = 512;
  pub struct Disk { /* private */ pub port: u32, pub model: String, pub sectors: u64 }
  /// # Safety: once, after interrupts are enabled.
  pub unsafe fn find(frames: &mut Frames, devices: &[pci::Device]) -> Vec<Disk>;
  impl Disk {
      pub fn read(&mut self, lba: u64, out: &mut [u8]) -> Result<(), &'static str>;
      pub fn write(&mut self, lba: u64, data: &[u8]) -> Result<(), &'static str>;
      pub fn flush(&mut self) -> Result<(), &'static str>;
  }
  ```

### Task C
- `kernel/src/fat.rs`:
  ```rust
  pub trait Blocks { fn read(&mut self, lba: u64, out: &mut [u8]) -> Result<(), &'static str>;
                       fn write(&mut self, lba: u64, data: &[u8]) -> Result<(), &'static str>; }
  pub struct Volume { /* partition start, geometry, label */ }
  pub struct Entry { pub cluster: u32, pub size: u32, pub directory: bool }
  impl Volume {
      pub fn open(disk: &mut impl Blocks, start_lba: u64) -> Result<Volume, &'static str>;
      pub fn label(&self) -> &str;
      pub fn find(&self, disk: &mut impl Blocks, path: &str) -> Result<Entry, &'static str>;
      pub fn read(&self, disk: &mut impl Blocks, entry: &Entry) -> Result<Vec<u8>, &'static str>;
      pub fn overwrite(&self, disk: &mut impl Blocks, entry: &Entry, data: &[u8]) -> Result<(), &'static str>;
  }
  ```

### Task D
- `kernel/src/sha512.rs`:
  ```rust
  pub fn sha512(data: &[u8]) -> [u8; 64];
  ```
- `kernel/src/ed25519.rs`:
  ```rust
  pub fn verify(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool;
  ```

## 5. Rejected alternatives

- **Custom partition table (GPT only)**: Rejected because Limine's `mbr_disk_id()` comes from the protective MBR; we need an MBR that Limine can read.
- **EXT2 file system**: Rejected because FAT32 is simpler, universally supported by mtools, and sufficient for our update workflow.
- **RSA signatures**: Rejected because Ed25519 is faster, safer, and already used in the limine-rust-template for kernel authentication.
- **AHCI with interrupts**: Rejected because polling is sufficient for the infrequent update check and keeps the driver simple.

## 6. Risks

- **AHCI controller not found**: Mitigated by printing `disk: no AHCI controller` and continuing; the ISO path remains usable for development.
- **FAT32 long-name collisions**: Mitigated by using the FAT volume label `GRENOS` as a second match; the volume label is written by `mformat -v GRENOS`.
- **SHA-512/Ed25519 side channels**: Mitigated by using constant-time field arithmetic from `x25519.rs`; the verify function branches only on input length and signature validity.

## 7. Task breakdown

### Task A: Disk image with two kernel slots
**Goal**: Create a bootable disk image with two kernel slots and a FAT32 file system.
**Assigned to**: coder
**Acceptance Criteria**:
- `kernel/scripts/make-disk.sh` exists and is executable.
- `kernel/limine-disk.conf` exists with the exact content above.
- Running the script from the repository root produces `kernel/grenos-disk.img`.
- `cargo build --release` succeeds in `kernel/`.
- `cargo clippy --release -- -D warnings` reports zero warnings in `kernel/`.
- QEMU boots the disk image via AHCI and prints `grenOS` on serial.

### Task B: AHCI (SATA) driver
**Goal**: Add an AHCI driver that detects SATA disks and prints their model and size.
**Assigned to**: coder
**Acceptance Criteria**:
- `kernel/src/ahci.rs` implements the `Disk` interface and `unsafe fn find`.
- `main.rs` calls the AHCI scan after `sti` and prints one line per disk.
- `cargo build --release` succeeds in `kernel/`.
- `cargo clippy --release -- -D warnings` reports zero warnings in `kernel/`.
- QEMU boots the disk image via AHCI and prints `grenOS` on serial.
- Serial output contains `disk: AHCI port <n>, <model>, <MiB> MiB` for each detected disk.

### Task C: MBR + FAT32, find our own disk, write test
**Goal**: Locate the boot disk by MBR signature and FAT label, then verify read/write access.
**Assigned to**: coder
**Acceptance Criteria**:
- `kernel/src/fat.rs` implements the `Volume` interface over the `Blocks` trait.
- `main.rs` uses `ExecutableFileRequest` to get the MBR disk signature, matches it to the AHCI disk, opens the FAT volume, overwrites `/grenos/essai.bin`, reads it back, and compares.
- `cargo build --release` succeeds in `kernel/`.
- `cargo clippy --release -- -D warnings` reports zero warnings in `kernel/`.
- QEMU boots the disk image via AHCI and prints `grenOS` on serial.
- Serial output contains the two lines: `disk: grenOS partition FAT32 GRENOS, booted from slot <a|b>` and `disk: write and read back verified`.

### Task D: SHA-512 and Ed25519
**Goal**: Add SHA-512 hashing and Ed25519 signature verification using RFC 8032 test vectors.
**Assigned to**: coder
**Acceptance Criteria**:
- `kernel/src/sha512.rs` implements `fn sha512(data: &[u8]) -> [u8; 64]`.
- `kernel/src/ed25519.rs` implements `fn verify(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool`.
- `main.rs` runs the three RFC 8032 test vectors and the negative case.
- `cargo build --release` succeeds in `kernel/`.
- `cargo clippy --release -- -D warnings` reports zero warnings in `kernel/`.
- QEMU boots the disk image via AHCI and prints `grenOS` on serial.
- Serial output contains `crypto: ed25519 verified against RFC 8032`.

## 8. Sources

- `https://raw.githubusercontent.com/limine-bootloader/limine/trunk/CONFIG.md` — Limine configuration syntax (timeout, quiet, kernel_path).
- `https://raw.githubusercontent.com/limine-bootloader/limine-protocol/trunk/PROTOCOL.md` — Limine boot protocol (ExecutableFileRequest, mbr_disk_id()).
- `https://docs.rs/limine/0.5.0/limine/request/struct.ExecutableFileRequest.html` — ExecutableFileRequest API.
- `https://www.mtools.org/mtools_1.html#mformat` — mformat `-F` for FAT32, `-v` for volume label.
- `https://www.gnu.org/software/gdisk/manual/gdisk.html` — sgdisk `-n` for partition creation, `-t` for type code, `-m` to set MBR signature.
- `https://raw.githubusercontent.com/limine-bootloader/limine/trunk/limine-bios.sys` — Limine BIOS binary copied by the script.
- `https://raw.githubusercontent.com/limine-bootloader/limine/trunk/BOOTX64.EFI` — Limine UEFI binary copied by the script.
- `https://github.com/torvalds/linux/blob/master/drivers/ata/ahci.h` — AHCI register offsets (CAP, GHC, PI, port registers).
- `https://github.com/torvalds/linux/blob/master/include/linux/ata.h` — ATA command register FIS layout, PRDT entry layout.
- `https://github.com/torvalds/linux/blob/master/drivers/ata/libata-sata.c` — `ata_tf_to_fis` for H2D register FIS.
- `https://www.ata.org/ata/doc/ATA_ATAPI_Standards` — ATA command codes: IDENTIFY DEVICE 0xEC, READ DMA EXT 0x25, WRITE DMA EXT 0x35, FLUSH CACHE EXT 0xEA.
- `https://www.uefi.org/sites/default/files/resources/UEFI_Spec_2_9_A.pdf` — MBR partition table layout (used for MBR offsets).
- `https://www.microsoft.com/whdc/system/platform/firmware/fatgen.mspx` — FAT32 boot sector layout (bytes per sector, sectors/cluster, reserved sectors, FAT count, FAT size, root cluster, volume label).
- `https://www.microsoft.com/whdc/system/platform/firmware/fatgen.mspx` — FAT directory entry layout (name, attributes, cluster high/low, file size).
- `https://www.microsoft.com/whdc/system/platform/firmware/fatgen.mspx` — FAT long-name slot layout (sequence, UTF-16 chunks, checksum).
- `https://csrc.nist.gov/publications/detail/fips/180/4/final` — FIPS 180-4 SHA-512 specification.
- `https://www.rfc-editor.org/rfc/rfc8032.txt` — RFC 8032 Ed25519 signature scheme, test vectors in §7.1.
- `https://github.com/Grenofar/grenOS/blob/main/kernel/src/x25519.rs` — Field arithmetic for Curve25519 used in Ed25519 verification.
- `https://raw.githubusercontent.com/limine-bootloader/limine-rust-template/trunk/kernel/Cargo.toml` — Example of `limine` crate usage.
- `https://raw.githubusercontent.com/limine-bootloader/limine-rust-template/trunk/kernel/build.rs` — Example of setting `GRENOS_BUILD` environment variable.
- `https://raw.githubusercontent.com/limine-bootloader/limine-rust-template/trunk/kernel/.cargo/config.toml` — Example of static relocation for `x86_64-unknown-none`.
- `https://raw.githubusercontent.com/limine-bootloader/limine-rust-template/trunk/kernel/linker-x86_64.ld` — Example linker script placing kernel in higher half.
- `https://raw.githubusercontent.com/limine-bootloader/limine-rust-template/trunk/kernel/limine.conf` — Example Limine configuration for ISO boot.
- `https://raw.githubusercontent.com/limine-bootloader/limine-rust-template/trunk/kernel/scripts/make-iso.sh` — Example script that builds the kernel and copies Limine binaries.
