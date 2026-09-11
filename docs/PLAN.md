# grenOS Minimal Boot Plan

## Problem

Create a bare-metal x86_64 kernel that boots via Limine in QEMU, prints the string `grenOS` to the serial console (COM1), and halts cleanly. No VGA, no interrupts, no allocator, no `std`.

## Constraints

- Target: the built-in `x86_64-unknown-none` (freestanding, `no_std`). No custom target JSON.
- Bootloader: Limine, via the `limine` crate version 0.5 (source: crates.io API, `max_stable_version` 0.6.5, version 0.5.0 published 2025-06-05; the mission pins 0.5).
- Build: `cargo build --release` run inside `kernel/`, with the target set in `kernel/.cargo/config.toml` (`[build] target = "x86_64-unknown-none"`). CI runs exactly this command with no `--target` flag.
- Linker script: `kernel/linker-x86_64.ld`, passed to the linker by `kernel/build.rs` via `cargo:rustc-link-arg=-Tlinker-x86_64.ld` (source: limine-rust-template `kernel/build.rs`).
- Output: serial port COM1 (I/O port `0x3F8`) only.
- Termination: `hlt` loop after printing, written as inline assembly (`core::arch::asm!`), because `core::arch::x86_64` has no `hlt`, `outb` or `inb` functions.
- Image: ISO built with `xorriso` and `mtools`, bootable in QEMU with `-serial stdio`.

## Approach

1. Create a freestanding Rust project in `kernel/` with `#![no_std]` and `#![no_main]`. Use the built-in `x86_64-unknown-none` target: set it in `kernel/.cargo/config.toml` under `[build] target`, and list it under `targets` in `kernel/rust-toolchain.toml` so its precompiled `core` is available. Move the flags the limine-rust-template passes through RUSTFLAGS in its GNUmakefile (`-C relocation-model=static`) into `[target.x86_64-unknown-none] rustflags` in `.cargo/config.toml`, because CI runs a plain `cargo build --release`.
2. Add the `limine` crate version 0.5 as the only dependency in `kernel/Cargo.toml`. Use its `BaseRevision` tag and its request structs (`limine::request::BootloaderInfoRequest`, `limine::request::StackSizeRequest`) instead of hand-written `#[repr(C)]` structs with invented IDs. The crate places requests in the `.requests` section itself; the linker script keeps that section.
3. Write `kernel/linker-x86_64.ld` based on the limine-rust-template's script: `OUTPUT_FORMAT(elf64-x86-64)`, `ENTRY(kmain)`, base address `0xffffffff80000000`, and a `.data` output section that keeps `.requests_start_marker`, `.requests`, and `.requests_end_marker` (source: limine-rust-template `kernel/linker-x86_64.ld`).
4. Write `kernel/build.rs` that passes the linker script: `println!("cargo:rustc-link-arg=-Tlinker-x86_64.ld");` and `println!("cargo:rerun-if-changed=linker-x86_64.ld");` (source: limine-rust-template `kernel/build.rs`).
5. Implement `kmain` as the kernel entry point (the linker script's `ENTRY(kmain)`), declared `#[no_mangle] pub extern "C" fn kmain() -> !`. Inside it, initialise the serial driver, write `grenOS\n`, then enter an infinite `hlt` loop using `core::arch::asm!("hlt", options(nomem, nostack, preserves_flags))`.
6. Implement a minimal serial driver for COM1 in `kernel/src/serial.rs`: initialise the UART (disable interrupts, set baud divisor for 38400, 8N1, enable FIFO), then provide `write_byte` and `write_str`. Port I/O uses inline assembly: `core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags))` for output and `core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags))` for input. Wait for the transmit buffer empty bit (line status bit 5) before each byte.
7. Build an ISO with a script `kernel/scripts/make-iso.sh` that CI calls via `bash` (files committed by agents are never executable). The script fetches the Limine binaries itself (CI has `xorriso` and `mtools` and nothing from Limine), copies the kernel ELF and `limine.conf` into the image, and runs `limine` to make it bootable. `limine.conf` (not `limine.cfg`) specifies the kernel path; Limine scans for `limine.conf` at `/boot/limine.conf` among other locations (source: Limine CONFIG.md).
8. Verify in QEMU: `qemu-system-x86_64 -cdrom <image> -serial stdio -display none -no-reboot -no-shutdown -m 256M` must print `grenOS` and then halt (no reboot, no crash). CI kills QEMU after 90 seconds and checks that the serial output contains `grenOS` and none of `panic`, `triple fault`, `double fault`.

## Interfaces

### Limine boot protocol (via the `limine` crate 0.5)

Do not hand-write request structs. Use the crate's types (source: docs.rs `limine::request` module for 0.5.0):

```rust
use limine::BaseRevision;
use limine::request::{BootloaderInfoRequest, StackSizeRequest};

// Require protocol revision 2 or higher (source: limine 0.5.0 crate docs, "Usage").
pub static BASE_REVISION: BaseRevision = BaseRevision::new();

// Request a larger stack, recommended on debug Rust builds (source: limine 0.5.0 crate docs).
pub const STACK_SIZE: u64 = 0x100000;
pub static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest::new().with_size(STACK_SIZE);

// Request the bootloader name and version (source: docs.rs limine::request::BootloaderInfoRequest).
pub static BOOTLOADER_INFO_REQUEST: BootloaderInfoRequest = BootloaderInfoRequest::new();
```

The crate places these statics in the `.requests` section. The linker script must keep `.requests_start_marker`, `.requests`, and `.requests_end_marker` in `.data` (source: limine-rust-template `kernel/linker-x86_64.ld`).

### Serial driver (COM1)

```rust
pub const COM1: u16 = 0x3F8;

pub fn init();
pub fn write_byte(byte: u8);
pub fn write_str(s: &str);
```

Register offsets: `0` data, `1` interrupt enable, `2` FIFO control, `3` line control, `4` modem control, `5` line status. Baud divisor latch: set DLAB (bit 7 of line control), write divisor low/high to ports `0`/`1`, clear DLAB. Divisor for 38400 baud = 3 (115200 / 38400).

Port I/O is inline assembly, because `core::arch::x86_64` has no `outb` or `inb` (source: Rust standard library documentation for `core::arch::x86_64`; the module provides CPU intrinsics such as `_rdtsc`, not port I/O):

```rust
unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    value
}
```

### Kernel entry

```rust
#[no_mangle]
pub extern "C" fn kmain() -> ! {
    serial::init();
    serial::write_str("grenOS\n");
    loop {
        core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
    }
}
```

`kmain` is the entry point because the linker script declares `ENTRY(kmain)` (source: limine-rust-template `kernel/linker-x86_64.ld`).

## Rejected Alternatives

- **VGA text mode output**: rejected because the mission requires serial-only; VGA adds framebuffer complexity without benefit.
- **Multiboot2**: rejected because the mission mandates Limine.
- **Hand-written Limine protocol structs**: rejected because the `limine` crate 0.5 provides them, and the original plan's hand-written IDs were invented. Using the crate is simpler and auditable.
- **Custom target JSON (`x86_64-grenos.json`)**: rejected because the built-in `x86_64-unknown-none` target exists and its `core` ships precompiled; a custom target JSON requires unstable flags and `build-std`, which CI does not configure. Mission 1 lost an attempt to exactly that error.
- **`limine.cfg`**: rejected because Limine reads `limine.conf`, not `limine.cfg` (source: Limine CONFIG.md).
- **`core::arch::x86_64::hlt`/`outb`/`inb`**: rejected because these functions do not exist in `core::arch::x86_64`; halting and port I/O are inline assembly.
- **Higher baud rate (115200)**: rejected because 38400 is more forgiving in QEMU and sufficient for a single line.

## Risks

- **Linker script mismatch**: if `.requests` is not kept in `.data`, Limine will not find the requests and the kernel will not boot. Mitigation: copy the limine-rust-template's linker script exactly, including the `KEEP(*(.requests_start_marker))`, `KEEP(*(.requests))`, `KEEP(*(.requests_end_marker))` lines.
- **Serial port not initialised correctly**: if the UART init sequence is wrong, no output appears. Mitigation: use the well-documented 8N1 38400 sequence; test in QEMU with `-serial stdio`.
- **Toolchain missing the target**: if `kernel/rust-toolchain.toml` does not list `x86_64-unknown-none` under `targets`, the build fails because `core` is not available. Mitigation: include the target in the toolchain file, as the limine-rust-template does.
- **Flags lost**: the template passes `-C relocation-model=static` through RUSTFLAGS in its GNUmakefile; CI runs a plain `cargo build --release`. Mitigation: move the flag into `[target.x86_64-unknown-none] rustflags` in `kernel/.cargo/config.toml`.

## Task Breakdown

### Phase 1: Project setup and freestanding build

**Goal**: Create a Rust project in `kernel/` that compiles to a freestanding x86_64 ELF with no `std`, using the built-in `x86_64-unknown-none` target.

**Acceptance criteria**:
- `kernel/Cargo.toml` exists with `[package]` and the `limine` crate version 0.5 as a dependency
- `kernel/.cargo/config.toml` exists with `[build] target = "x86_64-unknown-none"` and `[target.x86_64-unknown-none] rustflags = ["-C", "relocation-model=static"]`
- `kernel/rust-toolchain.toml` exists and lists `x86_64-unknown-none` under `targets`
- `cargo build --release` succeeds from `kernel/`

### Phase 2: Linker script and build script

**Goal**: Add the linker script and build script so the kernel links at the Limine-mandated higher-half address with the `.requests` section kept.

**Acceptance criteria**:
- `kernel/linker-x86_64.ld` exists with `OUTPUT_FORMAT(elf64-x86-64)`, `ENTRY(kmain)`, base address `0xffffffff80000000`, and `KEEP(*(.requests_start_marker))`, `KEEP(*(.requests))`, `KEEP(*(.requests_end_marker))` in `.data`
- `kernel/build.rs` exists and passes `-Tlinker-x86_64.ld` via `cargo:rustc-link-arg`
- `cargo build --release` succeeds from `kernel/`

### Phase 3: Limine requests via the crate

**Goal**: Use the `limine` crate 0.5 to declare the base revision and requests.

**Acceptance criteria**:
- `kernel/src/main.rs` exists with `#![no_std]` and `#![no_main]`
- A static `BASE_REVISION: BaseRevision = BaseRevision::new()` exists
- A static `STACK_SIZE_REQUEST: StackSizeRequest` exists
- `cargo build --release` succeeds from `kernel/`

### Phase 4: Kernel entry point

**Goal**: Implement `kmain` that initialises serial, prints `grenOS`, and halts.

**Acceptance criteria**:
- `kernel/src/main.rs` defines `#[no_mangle] pub extern "C" fn kmain() -> !`
- `kmain` calls `serial::init()` and `serial::write_str("grenOS\n")`
- `kmain` enters an infinite `hlt` loop using `core::arch::asm!("hlt", options(nomem, nostack, preserves_flags))`
- `cargo build --release` succeeds from `kernel/`
- `cargo clippy --release -- -D warnings` succeeds from `kernel/`

### Phase 5: Serial output driver

**Goal**: Implement a minimal COM1 serial driver with `init`, `write_byte`, `write_str`.

**Acceptance criteria**:
- `kernel/src/serial.rs` exists with the functions specified in Interfaces
- `init` configures COM1 for 38400 baud, 8N1, FIFO enabled, interrupts disabled
- `write_str` writes each byte via inline-assembly port I/O and waits for transmit buffer empty (line status bit 5)
- `cargo build --release` succeeds from `kernel/`
- `cargo clippy --release -- -D warnings` succeeds from `kernel/`

### Phase 6: Image build

**Goal**: Produce a bootable ISO containing the kernel and Limine bootloader.

**Acceptance criteria**:
- `kernel/scripts/make-iso.sh` exists and, when run via `bash`, produces a bootable ISO
- The script fetches the Limine binaries itself (CI has `xorriso` and `mtools` and nothing from Limine)
- The ISO contains the kernel ELF and `limine.conf` (not `limine.cfg`)
- `limine.conf` specifies the kernel path
- The ISO is non-empty and bootable

### Phase 7: QEMU verification

**Goal**: Boot the ISO in QEMU and verify serial output.

**Acceptance criteria**:
- `qemu-system-x86_64 -cdrom <image> -serial stdio -display none -no-reboot -no-shutdown -m 256M` prints `grenOS`
- The serial output contains `grenOS` and none of `panic`, `triple fault`, `double fault`
- QEMU process remains running (kernel halted, not crashed or rebooted)

## Sources

- crates.io API for `limine`: confirmed version 0.5.0 exists (published 2025-06-05).
- docs.rs `limine` 0.5.0 crate page: `BaseRevision` tag, request usage, `StackSizeRequest::new().with_size(...)`.
- docs.rs `limine::request` module for 0.5.0: `BootloaderInfoRequest`, `StackSizeRequest`, and other request structs.
- Limine CONFIG.md: `limine.conf` is the config file name; Limine scans `/boot/limine.conf` among other locations.
- limine-rust-template `kernel/linker-x86_64.ld`: linker script with `ENTRY(kmain)`, base `0xffffffff80000000`, and `.requests` kept in `.data`.
- limine-rust-template `kernel/build.rs`: passes the linker script via `cargo:rustc-link-arg=-Tlinker-x86_64.ld`.
- Rust standard library documentation for `core::arch::x86_64`: the module provides CPU intrinsics, not `hlt`, `outb` or `inb`; those are inline assembly.
