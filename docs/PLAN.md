# grenOS GDT, IDT and CPU Exceptions Technical Plan

## 1. Problem

The kernel currently boots into 64-bit long mode under Limine, initializes COM1, prints `grenOS`, and halts, but it relies entirely on the bootloader's initial GDT and has no IDT. Any CPU exception (such as a breakpoint, page fault, general protection fault, or stack issue) causes an unhandled fault escalating directly to a triple fault and CPU reset. This mission establishes kernel-controlled execution state by creating a permanent Global Descriptor Table (GDT) with kernel code/data segments and a Task State Segment (TSS) equipped with an Interrupt Stack Table (IST), installs an Interrupt Descriptor Table (IDT) handling critical CPU exceptions (#BP, #DF with IST, #GP, #PF), completes repository housekeeping (removing `kernel/Cargo.tompl` and configuring the `repository` field in `kernel/Cargo.toml`), and ensures the kernel continues to build and cleanly boot printing `grenOS` on COM1.

## 2. Constraints

- **Bootloader and Template**: Limine bootloader using crate `limine` 0.5, conforming to `limine-rust-template` (`kernel/linker-x86_64.ld`, `limine.conf` / boot configuration references, and `GNUmakefile` flag conventions).
- **Target & Toolchain**: Built-in target `x86_64-unknown-none` with flags specified in `kernel/.cargo/config.toml` (`[target.x86_64-unknown-none] rustflags = ["-C", "relocation-model=static"]`). Nightly Rust compiler with `#![no_std]` and `#![no_main]`.
- **Inline Assembly Requirement**: Any and all inline assembly (`core::arch::asm!`) must strictly be enclosed within an `unsafe { ... }` block with an explicit `// SAFETY:` rationale. Neither `core::arch::x86_64` nor `core` provides `lgdt`, `lidt`, `ltr`, `hlt`, `inb`, or `outb` safe functions.
- **Scope Boundary**: Strictly no device interrupts (PIC 8259, APIC, IOAPIC, timer, keyboard), no virtual memory / paging modifications, and no dynamic memory allocator (heap).
- **Housekeeping**: Delete legacy typo file `kernel/Cargo.tompl` if present and verify `repository = "https://github.com/grenOS/grenOS"` in `kernel/Cargo.toml`.
- **Verification Contract**: CI runs `cargo build --release` in `kernel/`, `cargo clippy --release -- -D warnings` in `kernel/`, builds ISO via `bash kernel/scripts/make-iso.sh`, and tests boot in QEMU requiring serial output to include `grenOS` with no `panic`, `triple fault`, or `double fault`.

## 3. Approach

1. **Housekeeping**:
   - Ensure `kernel/Cargo.tompl` is removed from the filesystem.
   - Ensure `kernel/Cargo.toml` has `repository = "https://github.com/grenOS/grenOS"` under `[package]`.
2. **GDT and TSS Setup (`kernel/src/gdt.rs`)**:
   - Construct a static GDT containing:
     1. Null descriptor (index 0, selector `0x00`).
     2. Kernel 64-bit Code Segment (index 1, selector `0x08`, base 0, limit 0, flags: Present, Ring 0, Executable, Readable, Long-mode `L=1, D=0`).
     3. Kernel 64-bit Data Segment (index 2, selector `0x10`, base 0, limit 0, flags: Present, Ring 0, Writable).
     4. TSS Descriptor (index 3 and 4, selector `0x18`, 16-byte system segment descriptor for 64-bit TSS, type `0x9`, Present, Ring 0).
   - Define a static `TaskStateSegment` with a dedicated 16-KiB double fault stack allocated in `IST1`.
   - Load the GDT using `lgdt` inside an `unsafe` block.
   - Reload data segments (`ds`, `es`, `fs`, `gs`, `ss`) with kernel data selector (`0x10`).
   - Reload the code segment selector (`0x08`) via a 64-bit far return (`push 0x08; lea rax, [rip + 1f]; push rax; retfq; 1:`).
   - Load the task register using `ltr` inside an `unsafe` block with TSS selector (`0x18`).
3. **IDT Setup (`kernel/src/idt.rs`)**:
   - Define a 256-entry `InterruptDescriptorTable` of 16-byte gates as mandated by the AMD64/x86_64 architecture.
   - Configure gate attributes (Present, DPL 0, Gate Type `0xE` for 64-bit Interrupt Gate).
   - Register handlers for:
     - Vector 3: Breakpoint (`#BP`), no error code.
     - Vector 8: Double Fault (`#DF`), error code, configured with `IST1` so execution switches to a known clean stack even on kernel stack exhaustion.
     - Vector 13: General Protection Fault (`#GP`), error code.
     - Vector 14: Page Fault (`#PF`), error code.
   - Handlers print diagnostic details (exception name, instruction pointer, error code, fault address from `CR2` for `#PF`) via `serial::write_str` and halt, or resume (for `#BP`).
   - Load the IDT using `lidt` inside an `unsafe` block.
4. **Integration in `kernel/src/main.rs`**:
   - In `kmain()`, initialize serial (`serial::init()`), initialize GDT/TSS (`gdt::init()`), initialize IDT (`idt::init()`), and output `grenOS\n`.
   - Trigger a non-fatal test breakpoint exception (`int3`) to verify that the IDT and exception handling are operational, or proceed to standard serial greeting and clean halt.

## 4. Interfaces

### Segment Selectors and Descriptors

```rust
pub const KERNEL_CODE_SELECTOR: u16 = 0x08;
pub const KERNEL_DATA_SELECTOR: u16 = 0x10;
pub const TSS_SELECTOR: u16 = 0x18;

#[repr(C, packed)]
pub struct DescriptorTablePointer {
    pub limit: u16,
    pub base: u64,
}

#[repr(C, packed)]
pub struct TaskStateSegment {
    pub reserved0: u32,
    pub rsp0: u64,
    pub rsp1: u64,
    pub rsp2: u64,
    pub reserved1: u64,
    pub ist1: u64,
    pub ist2: u64,
    pub ist3: u64,
    pub ist4: u64,
    pub ist5: u64,
    pub ist6: u64,
    pub ist7: u64,
    pub reserved2: u64,
    pub reserved3: u16,
    pub iopb_offset: u16,
}
```

### GDT Loading Implementation

All inline assembly must be in `unsafe` blocks with explicit safety comments:

```rust
pub unsafe fn load_gdt(ptr: &DescriptorTablePointer) {
    // SAFETY: ptr points to a valid static GDT descriptor table pointer.
    unsafe {
        core::arch::asm!("lgdt [{}]", in(reg) ptr, options(readonly, nostack, preserves_flags));
    }
}

pub unsafe fn reload_segments(code_sel: u16, data_sel: u16) {
    // SAFETY: code_sel and data_sel are valid selectors in the loaded GDT.
    unsafe {
        core::arch::asm!(
            "mov ds, {0:x}",
            "mov es, {0:x}",
            "mov fs, {0:x}",
            "mov gs, {0:x}",
            "mov ss, {0:x}",
            "push {1}",
            "lea {2}, [rip + 1f]",
            "push {2}",
            "retfq",
            "1:",
            in(reg) data_sel,
            in(reg) u64::from(code_sel),
            lateout(reg) _,
            options(preserves_flags)
        );
    }
}

pub unsafe fn load_tss(tss_sel: u16) {
    // SAFETY: tss_sel references a valid TSS descriptor in the active GDT.
    unsafe {
        core::arch::asm!("ltr {0:x}", in(reg) tss_sel, options(nostack, preserves_flags));
    }
}
```

### IDT Gate Structure and Exception Context

```rust
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct IdtEntry {
    pub offset_low: u16,
    pub selector: u16,
    pub ist: u8,          // bits 0..2: IST index (0 = none, 1..7 = IST1..7)
    pub type_attr: u8,    // 0x8E = present, ring 0, 64-bit interrupt gate
    pub offset_mid: u16,
    pub offset_high: u32,
    pub zero: u32,
}

#[repr(C)]
pub struct ExceptionStackFrame {
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

pub unsafe fn load_idt(ptr: &DescriptorTablePointer) {
    // SAFETY: ptr points to a valid IDT pointer with correct limit and base.
    unsafe {
        core::arch::asm!("lidt [{}]", in(reg) ptr, options(readonly, nostack, preserves_flags));
    }
}
```

### Exception Handlers

Handlers use either Rust `#![feature(abi_x86_interrupt)]` (standard on nightly) or naked assembly stubs that preserve registers, invoke logging over serial, and either halt (`hlt` in an infinite loop inside `unsafe`) or return (`iretq`).

## 5. Rejected Alternatives

- **External crates like `x86_64`**: Rejected to avoid external dependency drift, version churn, and unneeded complexity for a minimal freestanding kernel.
- **Enabling 8259 PIC / APIC or timer interrupts**: Rejected because the mission scope is strictly CPU exceptions and descriptor tables.
- **Paging / Virtual Memory remapping**: Rejected as Limine provides the identity/higher-half mapping necessary for early boot; page table manipulation belongs to a dedicated memory management mission.
- **Omitting IST for Double Fault**: Rejected because a double fault frequently stems from stack overflows; without a dedicated IST stack, servicing `#DF` immediately triple-faults the CPU.
- **Placing `asm!` outside `unsafe` blocks**: Rejected because Rust compiler and preflight checks require all `asm!` invocations to be contained within `unsafe` blocks.

## 6. Risks

- **CS Register Reload Fault**: In x86_64 long mode, `mov cs, ax` is illegal; changing CS requires `retfq`, `lretq`, or far jmp. Mitigation: Use standard `push cs; lea rax, [rip + 1f]; push rax; retfq; 1:` inside `unsafe` assembly.
- **TSS Descriptor Size Mismatch**: In 64-bit mode, TSS descriptors are 16 bytes (occupying two consecutive 8-byte GDT slots), unlike 8-byte segment descriptors. Mitigation: Accurately configure upper 8 bytes (base 32..63 and zero) and mark GDT limit to encompass 16-byte TSS descriptor.
- **Stack Alignment in Exception Handlers**: The AMD64 ABI requires a 16-byte aligned stack prior to function calls. Mitigation: Ensure IST and exception handler entry frames respect 16-byte alignment before making subroutine calls.

## 7. Task Breakdown

### Task 1: Housekeeping and GDT/TSS Initialization
**Goal**: Clean up legacy files and establish a valid 64-bit GDT with kernel code/data segments and TSS with IST.
**Assigned to**: `coder`
**Acceptance Criteria**:
- `kernel/Cargo.tompl` does not exist.
- `kernel/Cargo.toml` contains `repository = "https://github.com/grenOS/grenOS"`.
- `kernel/src/gdt.rs` is implemented with kernel code segment, kernel data segment, TSS with IST1 stack, and initialization functions.
- All `asm!` calls are strictly inside `unsafe` blocks.
- `cargo build --release` succeeds in `kernel/`.
- `cargo clippy --release -- -D warnings` succeeds in `kernel/`.
- QEMU boot succeeds and prints `grenOS` on COM1 without panic or faults.

### Task 2: IDT and CPU Exception Handlers
**Goal**: Implement the IDT with handlers for breakpoint (#BP), double fault (#DF with IST1), general protection (#GP), and page fault (#PF).
**Assigned to**: `coder`
**Acceptance Criteria**:
- `kernel/src/idt.rs` provides a 256-entry IDT loaded via `lidt` inside an `unsafe` block.
- Exception handlers for vector 3 (#BP), vector 8 (#DF using IST1), vector 13 (#GP), and vector 14 (#PF) are registered.
- All `asm!` calls are strictly inside `unsafe` blocks.
- `kmain()` invokes `gdt::init()` and `idt::init()` and serial output retains `grenOS\n`.
- No PIC/APIC or hardware device interrupts, paging changes, or heap code are introduced.
- `cargo build --release` and `cargo clippy --release -- -D warnings` succeed in `kernel/`.
- Kernel boots cleanly in QEMU printing `grenOS` with no unexpected faults.

## 8. Sources

- `https://raw.githubusercontent.com/limine-bootloader/limine-rust-template/trunk/kernel/linker-x86_64.ld` — Linker script configuration and higher-half section placements.
- `https://raw.githubusercontent.com/limine-bootloader/limine-rust-template/trunk/limine.conf` and `CONFIG.md` — Limine bootloader protocol and configuration formats.
- `https://raw.githubusercontent.com/limine-bootloader/limine-rust-template/trunk/GNUmakefile` — Relocation model (`-C relocation-model=static`) and static linkage settings.
- `https://doc.rust-lang.org/reference/inline-assembly.html` — Rust `core::arch::asm!` requirements and unsafe qualification.
- AMD64 Architecture Programmer's Manual, Volume 2: System Programming (Segments, TSS 64-bit format, IDT gate descriptors, IST mechanics).
