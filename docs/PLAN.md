# grenOS Minimal Boot Plan

## Problem

Create a bare-metal x86_64 kernel that boots via Limine 11.x in QEMU, prints the string `grenOS` to the serial console (COM1), and halts cleanly. No VGA, no interrupts, no allocator, no `std`.

## Constraints

- Target: `x86_64-unknown-none` (freestanding, `no_std`)
- Bootloader: Limine 11.x (limine.h protocol revision 3)
- Output: serial port COM1 (I/O port `0x3F8`) only
- Termination: `hlt` loop after printing
- Build: `cargo build --target x86_64-grenos.json`
- Image: ISO via `limine` tool, bootable in QEMU with `-serial stdio`

## Approach

1. Create a freestanding Rust project with a custom target JSON (`x86_64-grenos.json`) specifying `x86_64-unknown-none` base, `panic = "abort"`, and no default features.
2. Vendor the Limine boot protocol header (`limine.h` equivalent) as a Rust module using `#[repr(C)]` structs matching the Limine 11.x spec: base revision 3, framebuffer request (optional, unused), terminal request (optional, unused), and the stack/memory map requests (optional for this phase).
3. Implement `_start` as the kernel entry point, called by Limine with the boot info pointer. Store the pointer in a static for later use.
4. Implement a minimal serial driver for COM1: initialize the UART (disable interrupts, set baud divisor for 38400, 8N1, enable FIFO), then provide `write_byte` and `write_str` functions using port-mapped I/O (`outb`/`inb` via `core::arch::x86_64` intrinsics).
5. In `_start`, call `serial::init()`, write `grenOS\n`, then enter an infinite `hlt` loop.
6. Build an ISO: compile the kernel to an ELF, place it at `/boot/grenos` in a directory tree, copy the Limine bootloader files (`limine-bios.sys`, `limine-bios-cd.bin`, `limine.cfg`), and run `limine` to make it bootable.
7. Verify in QEMU: `qemu-system-x86_64 -cdrom grenos.iso -serial stdio -no-reboot -no-shutdown` must print `grenOS` and then halt (no reboot, no crash).

## Interfaces

### Limine boot protocol (Rust representation)

```rust
#[repr(C)]
pub struct LimineBootInfoRequest {
    pub id: [u64; 4],       // 0xf4, 0x3b, 0xda, 0x5b, 0x00, 0x00, 0x00, 0x00
    pub revision: u64,      // 0
    pub response: *mut LimineBootInfoResponse,
}

#[repr(C)]
pub struct LimineBootInfoResponse {
    pub revision: u64,
    pub name: *const u8,
    pub version: *const u8,
}
```

Request must be placed in a static marked `#[used]` and `#[link_section = ".requests"]` so Limine finds it.

### Serial driver (COM1)

```rust
pub const COM1: u16 = 0x3F8;

pub fn init();
pub fn write_byte(byte: u8);
pub fn write_str(s: &str);
```

Register offsets: `0` data, `1` interrupt enable, `2` FIFO control, `3` line control, `4` modem control, `5` line status. Baud divisor latch: set DLAB (bit 7 of line control), write divisor low/high to ports `0`/`1`, clear DLAB. Divisor for 38400 baud = 3.

### Kernel entry

```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial::init();
    serial::write_str("grenOS\n");
    loop { core::arch::x86_64::hlt(); }
}
```

## Rejected Alternatives

- **VGA text mode output**: rejected because mission requires serial-only; VGA adds framebuffer complexity without benefit.
- **Multiboot2**: rejected because mission mandates Limine 11.x.
- **Using `limine` crate**: rejected to keep dependencies minimal and protocol explicit; vendoring the header is simpler and auditable.
- **Higher baud rate (115200)**: rejected because 38400 is more forgiving in QEMU and sufficient for a single line.

## Risks

- **Limine protocol struct layout mismatch**: if the `#[repr(C)]` layout does not match the C header exactly, Limine will not find the request and the kernel will not boot. Mitigation: verify against the official Limine 11.x `limine.h` during implementation; first task includes a boot test that must print.
- **Serial port not initialized correctly**: if the UART init sequence is wrong, no output appears. Mitigation: use the well-documented 8N1 38400 sequence; test in QEMU with `-serial stdio`.
- **Custom target JSON missing required fields**: if `x86_64-grenos.json` is malformed, `cargo build` fails. Mitigation: base it on the standard `x86_64-unknown-none` target and add only `panic = "abort"`.

## Task Breakdown

### Phase 1: Project setup and freestanding build

**Goal**: Create a Rust project that compiles to a freestanding x86_64 ELF with no `std`.

**Acceptance criteria**:
- `kernel/Cargo.toml` exists with `[package]` and no dependencies
- `kernel/x86_64-grenos.json` exists and is valid JSON
- `cargo build --target x86_64-grenos.json` succeeds from `kernel/`
- Output ELF exists at `kernel/target/x86_64-grenos/debug/grenos`
- `file` on the ELF reports `x86-64` and not dynamically linked

### Phase 2: Limine boot protocol header

**Goal**: Vendor the Limine 11.x boot protocol as Rust structs and a boot info request.

**Acceptance criteria**:
- `kernel/src/limine.rs` exists with `LimineBootInfoRequest` and `LimineBootInfoResponse` structs
- A static `BOOT_INFO_REQUEST` is marked `#[used]` and placed in `.requests` section
- `cargo build --target x86_64-grenos.json` succeeds
- `readelf -S` on the ELF shows a `.requests` section containing the request

### Phase 3: Kernel entry point

**Goal**: Implement `_start` that stores the boot info pointer and halts.

**Acceptance criteria**:
- `kernel/src/main.rs` exists with `#[no_mangle] pub extern "C" fn _start() -> !`
- `_start` stores the boot info pointer in a static `BOOT_INFO`
- `_start` enters an infinite `hlt` loop
- `cargo build --target x86_64-grenos.json` succeeds
- Kernel does not reference any `std` symbols (verified with `nm -u`)

### Phase 4: Serial output driver

**Goal**: Implement a minimal COM1 serial driver with `init`, `write_byte`, `write_str`.

**Acceptance criteria**:
- `kernel/src/serial.rs` exists with the functions specified in Interfaces
- `init` configures COM1 for 38400 baud, 8N1, FIFO enabled, interrupts disabled
- `write_str` writes each byte via port I/O and waits for transmit buffer empty (line status bit 5)
- `cargo build --target x86_64-grenos.json` succeeds
- `cargo clippy --target x86_64-grenos.json` reports 0 warnings

### Phase 5: Image build

**Goal**: Produce a bootable ISO containing the kernel and Limine bootloader.

**Acceptance criteria**:
- A build script or Makefile target creates `grenos.iso`
- ISO contains `/boot/grenos` (the kernel ELF), `/boot/limine.cfg`, and Limine bootloader files
- `limine.cfg` specifies `grenos` as the kernel path
- `limine` tool runs without errors and reports the ISO is bootable
- `grenos.iso` exists and is non-empty

### Phase 6: QEMU verification

**Goal**: Boot the ISO in QEMU and verify serial output.

**Acceptance criteria**:
- `qemu-system-x86_64 -cdrom grenos.iso -serial stdio -no-reboot -no-shutdown` prints `grenOS`
- QEMU process remains running (kernel halted, not crashed or rebooted)
- No error messages or warnings from QEMU about the kernel
- The string `grenOS` appears exactly once in the serial output
