# Spec — installing updates from grenOS itself

Written by Claude for the agents, 2026-09-16, at the human's request: "updates
must be possible from the OS". Every fact below was checked against the source
named next to it. Where this document and your memory disagree, this document
wins; where it is silent, `consult` before you guess.

Language: English, because it is read by the models (docs/** is otherwise in
French).

## 0. Why this is several tasks, and in this order

grenOS runs today from an ISO in a read-only CD drive: there is nowhere to
write a new version. So:

| # | Task | Who | Proof the CI reads on the serial line |
|---|------|-----|---------------------------------------|
| A | Disk image with two kernel slots | Coder | the machine boots from the disk (existing `boot` step) |
| B | AHCI (SATA) driver | Coder | `disk: AHCI port <n>, <model>, <MiB> MiB` |
| C | MBR + FAT32, find our own disk, write test | Coder | `disk: grenOS partition FAT32 GRENOS, booted from slot <a|b>` and `disk: write and read back verified` |
| D | SHA-512 and Ed25519 signature verification | Coder | `crypto: ed25519 verified against RFC 8032` |
| E | Signed releases (manifest + signature) | Claude (CI) | files published next to the ISO |
| F | Download, verify, install, reboot | Coder | `update: installed <build> into slot <a|b>` |
| G | End-to-end test in CI | Claude (CI) | a second boot prints `booted from slot b` |

One task, one commit, one green CI. Each task must leave the kernel booting and
all six CI steps green (build, clippy, boot, screen, input, net).

## 1. Hard rules (security)

1. grenOS writes **only** to the disk it booted from. The disk is identified by
   the MBR disk signature that Limine reports for the kernel file (§3.1) — it
   must equal bytes 440..444 of LBA 0 of the AHCI disk — **and** by the FAT
   volume label `GRENOS`. Any mismatch: no write, and say why on serial.
   A PC can have Windows on another disk; a wrong write destroys it.
2. Never grow, create, rename or delete a file on the disk. Only overwrite the
   bytes of existing files, inside their existing size and cluster chain.
3. Nothing is installed unless its Ed25519 signature verifies against the
   public key compiled into the kernel (§5). The TLS connection is encrypted but
   the server is not authenticated: the signature is the only trust.
4. Never install a build older than the running one (§6, anti-downgrade).
5. No `unwrap()`/`expect()` on anything read from the disk or the network: a
   corrupt sector or a hostile server must produce an error line, never a panic.

## 2. Task A — the disk image (`kernel/scripts/make-disk.sh`, `kernel/limine-disk.conf`)

Source: limine-c-template `GNUmakefile` (trunk, read 2026-09-16), Limine
`CONFIG.md` (branch v10.x, read 2026-09-16).

The template's hard-disk recipe, which we follow exactly, plus FAT32, a label,
and two kernel slots:

```bash
# run from kernel/, after make-iso.sh has built the kernel and cloned limine/
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
# a random non-zero MBR disk signature at bytes 440..443, written BEFORE bios-install;
# check after bios-install that it did not change
./limine/limine bios-install "$IMG"
mformat -i "$IMG"@@$OFFSET -F -v GRENOS -T $SECTORS -h $HEADS -s $SPT ::
mmd  -i "$IMG"@@$OFFSET ::/EFI ::/EFI/BOOT ::/boot ::/boot/limine ::/grenos
mcopy -i "$IMG"@@$OFFSET slot.bin ::/boot/kernel-a     # the kernel, padded with zeros to 8 MiB
mcopy -i "$IMG"@@$OFFSET slot.bin ::/boot/kernel-b     # the same, initially
mcopy -i "$IMG"@@$OFFSET disk.conf ::/boot/limine/limine.conf
mcopy -i "$IMG"@@$OFFSET limine/limine-bios.sys ::/boot/limine
mcopy -i "$IMG"@@$OFFSET limine/BOOTX64.EFI limine/BOOTIA32.EFI ::/EFI/BOOT
mcopy -i "$IMG"@@$OFFSET essai.bin ::/grenos/essai.bin   # 65536 zero bytes, for the write test
```

- `sgdisk` comes from the Ubuntu package `gdisk`; `mformat`/`mmd`/`mcopy` from
  `mtools`. `-F` forces FAT32, `-v` sets the volume label (mtools manual).
- Slots are padded to exactly 8 MiB (`truncate -s 8M`) so that an update never
  has to grow a file (rule 2). Fail the script if the kernel is larger.
- CI runs the script from the repository root, like `make-iso.sh`: start with
  `cd "$(dirname "$0")/.."` (as make-iso.sh does) and write
  `kernel/grenos-disk.img`. Fail loudly (`set -euo pipefail`).
- Optional environment variable `GRENOS_CMDLINE`: when set, add
  `    cmdline: $GRENOS_CMDLINE` to entry 1 only (CI uses it in task G).

`kernel/limine-disk.conf` — exactly this shape:

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

Why `timeout: 1` and not 0 (Limine CONFIG.md v10.x): with `timeout: 0` the
default entry boots instantly and the menu can never be reached. With a
timeout and `quiet: yes`, nothing is shown, and a key pressed during that
second reveals the menu — the only way back to the previous slot if an update
does not boot. `default_entry` is 1-based. Switching slots rewrites
`default_entry: 1` to `default_entry: 2` or back: same length, so the file
never grows.

The ISO (`make-iso.sh`, `kernel/limine.conf`) stays as it is.

CI (Claude): verify.yml builds the disk image when `kernel/scripts/make-disk.sh`
exists and boots QEMU from it through AHCI:

```
-drive file=kernel/grenos-disk.img,format=raw,if=none,id=disk0
-device ahci,id=ahci0 -device ide-hd,drive=disk0,bus=ahci0.0,bootindex=1
```

## 3. Task B — AHCI driver (`kernel/src/ahci.rs`)

Sources: Linux `drivers/ata/ahci.h`, `include/linux/ata.h`,
`drivers/ata/libata-sata.c` (`ata_tf_to_fis`), master, read 2026-09-16.

### 3.1 Finding the controller
PCI class 0x01, subclass 0x06, prog-if 0x01. Its registers (ABAR) are BAR 5
(`pci::bar(device, 5)`, config offset 0x24), memory-mapped: mask off the low 4
bits. Call `pci::enable_bus_master(device)`. Map the first two pages of ABAR
with `paging::map_device` at `0xFFFF_B100_0000_0000 + physical` (the e1000
driver already uses `0xFFFF_B000_0000_0000`; follow `e1000.rs` exactly for the
pattern).

### 3.2 Registers (offsets from ABAR)
| Register | Offset | Bits used |
|---|---|---|
| CAP | 0x00 | bit 31: 64-bit addresses supported |
| GHC | 0x04 | bit 31 AE: set it before anything else |
| PI  | 0x0C | bitmap of implemented ports |

Port `n` registers start at `0x100 + n * 0x80`:

| Register | Offset | Bits used |
|---|---|---|
| PxCLB / PxCLBU | 0x00 / 0x04 | command list physical address |
| PxFB / PxFBU | 0x08 / 0x0C | received FIS physical address |
| PxIS | 0x10 | bit 30 TFES (task file error); write all ones to clear |
| PxIE | 0x14 | write 0: this driver polls |
| PxCMD | 0x18 | bit 0 ST, bit 1 SUD, bit 2 POD, bit 4 FRE, bit 14 FR, bit 15 CR |
| PxTFD | 0x20 | status byte: 0x80 BSY, 0x08 DRQ, 0x01 ERR |
| PxSIG | 0x24 | 0x00000101 = ATA disk |
| PxSSTS | 0x28 | bits 3:0 DET = 3: device present, link up |
| PxSERR | 0x30 | write all ones to clear |
| PxCI | 0x38 | bit 0: command slot 0 issued / still running |

### 3.3 Bringing a port up
For each implemented port with DET = 3 and SIG = 0x00000101:
1. Stop: clear ST, wait until CR is clear; clear FRE, wait until FR is clear
   (at most 500 ms each; use `events::millis()`, interrupts are on by then).
2. One zeroed frame (`frames.allocate()`) for the command list, one for the
   received FIS, one for the command table; write their physical addresses to
   PxCLB/PxCLBU and PxFB/PxFBU. Reach them through the HHDM (`phys + hhdm`).
3. Clear PxSERR and PxIS (all ones), PxIE = 0.
4. Set SUD and POD, then FRE; wait until BSY and DRQ are clear; set ST.

### 3.4 One command (slot 0 only)
- Command header = first 32 bytes of the command list:
  `opts` u32 = 5 (FIS length in DWORDs) | 0x40 if writing | (PRDT entries << 16);
  `status` u32 = 0; `tbl_addr` u32 / `tbl_addr_hi` u32 = command table physical address.
- Command table: FIS at 0x00, PRDT entries at 0x80, 16 bytes each:
  `addr` u32, `addr_hi` u32, reserved u32, `flags_size` u32 = byte count − 1.
- H2D register FIS, 20 bytes: [0]=0x27, [1]=0x80 (command), [2]=command,
  [3]=features, [4..=6]=LBA bits 0-23, [7]=device (0x40 = LBA mode; 0 for
  IDENTIFY), [8..=10]=LBA bits 24-47, [11]=0, [12]=count low, [13]=count high.
- Data buffers: frames from `frames.allocate()`, one PRDT entry per 4 KiB
  frame. Keep a bounce buffer of 16 frames (64 KiB, 128 sectors); longer reads
  and writes loop over it. Copy in before a write, out after a read.
- Issue: clear PxIS, set PxCI = 1, poll until bit 0 of PxCI is clear. Fail if
  PxIS bit 30 is set or PxTFD & 0x01, or after 5 s.
- ATA commands (linux/ata.h): IDENTIFY DEVICE 0xEC (512 bytes, count 1),
  READ DMA EXT 0x25, WRITE DMA EXT 0x35, FLUSH CACHE EXT 0xEA (no data; send it
  after writes).

### 3.5 IDENTIFY data (512 bytes = 256 little-endian words)
- Model: words 27..=46, 40 characters, each word holds two characters with the
  **first character in the high byte**. Trim trailing spaces.
- LBA48 is usable when `(w[83] & 0xC000) == 0x4000 && w[83] & (1 << 10) != 0`;
  then sectors = u64 from words 100..=103 (low word first). Otherwise words
  60..=61. Sectors are 512 bytes.

### 3.6 Interface
```rust
pub const SECTOR: usize = 512;
pub struct Disk { /* private */ pub port: u32, pub model: String, pub sectors: u64 }
/// # Safety: once, after interrupts are enabled.
pub unsafe fn find(frames: &mut Frames, devices: &[pci::Device]) -> Vec<Disk>;
impl Disk {
    pub fn read(&mut self, lba: u64, out: &mut [u8]) -> Result<(), &'static str>;   // out.len() % 512 == 0
    pub fn write(&mut self, lba: u64, data: &[u8]) -> Result<(), &'static str>;     // data.len() % 512 == 0
    pub fn flush(&mut self) -> Result<(), &'static str>;
}
```
`main.rs`: after `sti`, scan, print one line per disk. No AHCI controller is not
an error: print `disk: no AHCI controller` (the ISO machine in QEMU has none).

## 4. Task C — our partition (`kernel/src/fat.rs`, and a few lines in `main.rs`)

Sources: Linux `include/uapi/linux/msdos_fs.h` (boot sector, directory entry,
long-name slot), limine crate 0.5.0 source (`request.rs`, `file.rs`).

### 4.1 Which disk and which slot booted
```rust
use limine::request::ExecutableFileRequest;
#[used] #[unsafe(link_section = ".requests")]
static EXECUTABLE_FILE_REQUEST: ExecutableFileRequest = ExecutableFileRequest::new();
// EXECUTABLE_FILE_REQUEST.get_response() -> Option<&ExecutableFileResponse>
// response.file() -> &limine::file::File
// file.path() -> &CStr            e.g. "/boot/kernel-a"; "/boot/kernel" on the ISO
// file.mbr_disk_id() -> Option<NonZeroU32>
```
Slot = `a` if the path ends in `kernel-a`, `b` for `kernel-b`, none otherwise
(ISO: no installation possible, say so).

### 4.2 MBR (LBA 0)
Disk signature: u32 LE at 440. Partition entry 1 at 446: type at +4, first LBA
u32 LE at +8, sector count u32 LE at +12. Bytes 510..=511 = 0x55 0xAA. Do not
rely on the partition type byte (it depends on how sgdisk converted the table):
identify the partition by the disk signature and the FAT label.

### 4.3 FAT32 boot sector (first sector of the partition), offsets
0x0B bytes/sector (must be 512) · 0x0D sectors/cluster · 0x0E reserved sectors
(u16) · 0x10 number of FATs · 0x16 FAT16 size (must be 0) · 0x20 total sectors
(u32) · 0x24 sectors per FAT (u32) · 0x2C root directory cluster (u32) · 0x47
volume label (11 bytes, space padded: `GRENOS     `). The mtools manual says `-v`
sets the volume label without saying where it is stored, so accept the label
from either place: the boot sector at 0x47, or the root directory entry whose
attr is 0x08.

- First data sector = partition start + reserved + FATs × sectors per FAT.
- Cluster N (N ≥ 2) starts at first data sector + (N − 2) × sectors/cluster.
- FAT entry of cluster N: u32 LE at FAT sector `N * 4 / 512`, offset
  `N * 4 % 512`, masked with 0x0FFFFFFF. ≥ 0x0FFFFFF8: end of chain.
  Refuse chains that loop (count steps against the total cluster count).

### 4.4 Directory entries (32 bytes)
name 0..=10 · attr 11 (0x10 directory, 0x08 volume label, 0x0F long-name slot)
· cluster high u16 LE at 20 · cluster low u16 LE at 26 · size u32 LE at 28.
name[0] = 0x00 ends the directory; 0xE5 is a deleted entry.

Long-name slot (attr 0x0F): id at 0 (sequence number, 0x40 set on the last one,
which is stored first), UTF-16LE characters at 1..=10 (5), 14..=25 (6), 28..=31
(2); the name ends at 0x0000, the rest is 0xFFFF. Slots precede their short
entry. `mcopy` stores `kernel-a` with a long name, so matching must use long
names (case-insensitive), falling back to the 8.3 name.

### 4.5 Interface
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
    /// Overwrites the file from its start; `data.len()` must not exceed `entry.size`,
    /// the rest of the file is zeroed. Never allocates clusters.
    pub fn overwrite(&self, disk: &mut impl Blocks, entry: &Entry, data: &[u8]) -> Result<(), &'static str>;
}
```
Write contiguous clusters in runs: read the cluster size from the boot sector
rather than assuming one (mformat chooses it), and group consecutive clusters
into one command of up to 128 sectors — one command per cluster of an 8 MiB
slot would be far too slow. Keep `fat.rs` free of hardware: it only uses `Blocks`, so it
can be tested on the host.

### 4.6 At boot
Find the AHCI disk whose signature matches `mbr_disk_id`, open partition 1,
check the label, find `/boot/kernel-a`, `/boot/kernel-b`,
`/boot/limine/limine.conf` and `/grenos/essai.bin`; overwrite `essai.bin` with a
pattern made from `rand::bytes()`, read it back, compare. Print the two lines of
§0. On the ISO: `disk: booted from the ISO, nothing to install to`.

## 5. Task D — SHA-512 and Ed25519 (`kernel/src/sha512.rs`, `kernel/src/ed25519.rs`)

Sources: RFC 8032 §5.1.7 (verify) and §7.1 (test vectors), FIPS 180-4.
`x25519.rs` already has the field arithmetic modulo 2^255 − 19 in TweetNaCl's
style; TweetNaCl's `crypto_sign_open` is the model to follow (unpack the public
key with the square root of −1, scalar multiplications on the Edwards curve,
reduction modulo L).

```rust
pub fn sha512(data: &[u8]) -> [u8; 64];
pub fn verify(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool;
```
Reject non-canonical input: signature `S` must be below L
(2^252 + 27742317777372353535851937790883648493); a public key that does not
decode must return false, never panic.

Test values, to check at boot in `main.rs` and print
`crypto: ed25519 verified against RFC 8032` (or which one failed):

- SHA-512("abc") = `ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f`
- RFC 8032 TEST 1: public key `d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a`,
  message empty, signature
  `e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b`
- TEST 2: public key `3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c`,
  message `72`, signature
  `92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00`
- TEST 3: public key `fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025`,
  message `af82`, signature
  `6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a`
- And the negative case: TEST 2's signature with its first byte changed must fail.

## 6. Task E — what a release publishes (Claude, release.yml)

Next to each ISO, in the public `releases` bucket:

- `builds/grenos-<build>.kernel` — the kernel ELF exactly as built.
- `builds/grenos-<build>.manifest` — text, `\n` line endings, exactly:
  ```
  grenos-update 1
  build 20260916-1102-e07aed4
  commit e07aed45a698de6dac83a9d8ed5459549d9adbbb
  kernel builds/grenos-20260916-1102-e07aed4.kernel
  size 942816
  sha256 <64 lowercase hex digits of the kernel file>
  ```
- `builds/grenos-<build>.sig` — the 64 raw bytes of the Ed25519 signature of the
  manifest file's exact bytes.
- `index.json` entries gain `"manifest"` and `"signature"` (paths as above).

The signing key never enters the repository: it lives in a private storage
bucket that only the release workflow reads. Its public half, the 32 bytes to
put in `update.rs` as the only trusted key:

```
4d2a2d3491057d2a64ab6959435fb61e288f5c2aa7e7e99afdc909d34873e914
```

The build name starts with `YYYYMMDD-HHMM`. release.yml exports that stamp as
the environment variable `GRENOS_STAMP` before building, so the kernel can
carry it: task F makes `kernel/build.rs` emit
`cargo:rustc-env=GRENOS_STAMP=<value>` (the variable when set, otherwise the
current UTC time in the same format) with `cargo:rerun-if-env-changed=GRENOS_STAMP`,
and refuses any manifest whose stamp is not strictly greater (the format sorts
as text).

## 7. Task F — install from Paramètres → Mise à jour

Download base: `https://tpqzhzuoyqpfairatdrw.supabase.co/storage/v1/object/public/releases/`
(the same host `update::INDEX` already uses).

1. The check already reads `index.json` (`update.rs`). Take `manifest` and
   `signature` from the first entry.
2. Fetch the manifest (text) and the signature (64 bytes). Verify (§5) against
   the compiled-in public key. Parse the manifest strictly: every line present,
   in order; `size` ≤ 8 MiB; build stamp newer than ours (§6).
3. Fetch the kernel. `http.rs` must keep binary bodies intact and bounded
   (§7.1). Check the size and the SHA-256 against the manifest.
4. Write it into the slot we did **not** boot from (`overwrite`, rule 2), flush,
   read it back, compare SHA-256 again.
5. Rewrite `limine.conf` with `default_entry` pointing at that slot (same
   length), flush, read back.
6. Show "Prêt : redémarrer pour utiliser <build>" and a restart button
   (`desktop::Action::Reboot`). Print
   `update: installed <build> into slot <a|b>`.

Each step shows progress in the window (bytes downloaded, step name) and any
failure in plain French, then stops without writing anything further.

### 7.1 `http.rs` for binaries
- Handle `Transfer-Encoding: chunked` (RFC 9112 §7.1): hex size, optional
  `;extensions`, CRLF, data, CRLF; a zero-size chunk, optional trailer lines,
  then a final CRLF. Beware the classic bug: the CRLF right after `0` belongs to
  the last-chunk line, and the trailer section ends with its own CRLF.
- Do not turn a binary body into text (`to_text` is for pages only).
- The heap (`heap::SIZE`) grows to 32 MiB; bound bodies at 12 MiB.

## 8. Task G — end to end in CI (Claude)

CI builds the disk image with `GRENOS_CMDLINE=grenos.selftest=update`. With
that command line, and only then, the kernel installs the newest published
signed kernel into slot b right after the network is up, then reboots
(`power::reboot`). CI starts QEMU a second time on the same image and expects
`booted from slot b`. The command line is read with
`limine::request::ExecutableCmdlineRequest` (`response.cmdline() -> &CStr`).
