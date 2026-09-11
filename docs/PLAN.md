# grenOS Desktop on Limine Framebuffer Technical Plan

## 1. Problem

The kernel initializes GDT, IDT, exception handling, and COM1 serial output, but has no graphical output. The display remains black or shows Limine's boot menu remnant. grenOS requires a Windows-like desktop graphical environment drawn to the screen via the Limine framebuffer (desktop background, bottom taskbar with start button, and window outlines/widgets) satisfying the CI screen verdict requiring at least 3 distinct colours with none exceeding 90% screen area.

## 2. Constraints

- **Limine Protocol & Crate**: Use `limine` crate version `0.5` (`limine::request::FramebufferRequest`). Requests are declared as static items placed in section `#[unsafe(link_section = ".requests")]` with `#[used]` between `RequestsStartMarker` and `RequestsEndMarker`.
- **Bootloader Configuration**: `limine.conf` uses valid Limine syntax (`timeout: 3`, `/grenOS`, `protocol: limine`, `kernel_path: boot():/boot/kernel`).
- **Target & Toolchain**: Built-in `x86_64-unknown-none` target with static relocation configured in `kernel/.cargo/config.toml`. Toolchain `nightly-2024-11-15` with `#![no_std]` and `#![no_main]`.
- **Inline Assembly**: All port I/O and halt instructions use `core::arch::asm!` enclosed in `unsafe { ... }` blocks with `// SAFETY:` rationale. Neither `hlt`, `outb`, nor `inb` exist in `core::arch::x86_64`.
- **Drawing Invariants**: Direct pixel writes to `Framebuffer::addr()` with `core::ptr::write_volatile` inside `unsafe` blocks. Pixel byte offset: `y * pitch + x * (bpp / 8)`.
- **CI Screen Contract**: CI screenshot analysis checks that the screen holds at least three colours and no single colour occupies more than 90% of the screen. Serial boot must print `grenOS` and have no `panic`, `triple fault`, or `double fault`.

## 3. Approach

1. **Framebuffer Request**: Declare `limine::request::FramebufferRequest::new()` in `kernel/src/main.rs` in the `.requests` section.
2. **Framebuffer Abstraction (`kernel/src/fb.rs`)**:
   - Query `FRAMEBUFFER_REQUEST.get_response()` and retrieve the primary `Framebuffer` via `.framebuffers().next()`.
   - Store framebuffer dimensions (`width`, `height`, `pitch`, `bpp`) and raw base address.
   - Implement raw pixel plotting supporting 32-bpp RGB (using `red_mask_shift`, `green_mask_shift`, `blue_mask_shift` or standard 0x00RRGGBB format for 32-bit framebuffers).
   - Implement primitive drawing: filled rectangles (`fill_rect`), horizontal/vertical lines, and borders.
3. **Windows-like Desktop UI Layout (`kernel/src/desktop.rs`)**:
   - **Desktop Background**: Classic teal/slate background (`0x00008080` or `0x003A6EA5`).
   - **Taskbar**: Bottom strip of height 32 px with light gray background (`0x00C0C0C0`) and top 3D highlight line (`0x00FFFFFF`).
   - **Start Button**: Bottom-left button (e.g. 60x24 px at offset x=4, y=height-28) with raised 3D borders (white top/left, dark gray bottom/right `0x00808080`) and green start emblem/text area (`0x00008000`).
   - **Window**: A centered or top-left window canvas (e.g. 320x240 px) featuring a dark blue title bar (`0x00000080`), close button (`0x00C0C0C0` with red cross `0x00CC0000`), window background (`0x00FFFFFF` or `0x00C0C0C0`), and 3D borders.
4. **Integration**: In `kmain()`, after serial and descriptor table initialization, initialize framebuffer, draw the desktop scene, and proceed to clean halt loop.

## 4. Interfaces

### Framebuffer Static Request

```rust
use limine::request::FramebufferRequest;

#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();
```

### Framebuffer Wrapper and Drawing Primitives

```rust
#[derive(Copy, Clone)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const TEAL: Color = Color { r: 0, g: 128, b: 128 };
    pub const GRAY: Color = Color { r: 192, g: 192, b: 192 };
    pub const DARK_GRAY: Color = Color { r: 128, g: 128, b: 128 };
    pub const WHITE: Color = Color { r: 255, g: 255, b: 255 };
    pub const NAVY: Color = Color { r: 0, g: 0, b: 128 };
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0 };
    pub const GREEN: Color = Color { r: 0, g: 160, b: 0 };
}

pub struct Display {
    addr: *mut u8,
    width: usize,
    height: usize,
    pitch: usize,
    bpp: usize,
    red_shift: u8,
    green_shift: u8,
    blue_shift: u8,
}

impl Display {
    pub fn from_limine(fb: &limine::framebuffer::Framebuffer) -> Self {
        Self {
            addr: fb.addr(),
            width: fb.width() as usize,
            height: fb.height() as usize,
            pitch: fb.pitch() as usize,
            bpp: fb.bpp() as usize,
            red_shift: fb.red_mask_shift(),
            green_shift: fb.green_mask_shift(),
            blue_shift: fb.blue_mask_shift(),
        }
    }

    pub fn draw_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }
        let pixel_value: u32 = ((color.r as u32) << self.red_shift)
            | ((color.g as u32) << self.green_shift)
            | ((color.b as u32) << self.blue_shift);

        let bytes_per_pixel = self.bpp / 8;
        let offset = y * self.pitch + x * bytes_per_pixel;

        // SAFETY: The calculated offset is strictly within the allocated framebuffer bounds checked against width and height.
        unsafe {
            let pixel_ptr = self.addr.add(offset);
            if bytes_per_pixel == 4 {
                core::ptr::write_volatile(pixel_ptr as *mut u32, pixel_value);
            } else if bytes_per_pixel == 3 {
                core::ptr::write_volatile(pixel_ptr, color.b);
                core::ptr::write_volatile(pixel_ptr.add(1), color.g);
                core::ptr::write_volatile(pixel_ptr.add(2), color.r);
            }
        }
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Color) {
        for cy in y..(y + h).min(self.height) {
            for cx in x..(x + w).min(self.width) {
                self.draw_pixel(cx, cy, color);
            }
        }
    }
}
```

### Desktop UI Drawer

```rust
pub fn render_desktop(display: &mut Display) {
    // 1. Teal background
    display.fill_rect(0, 0, display.width, display.height, Color::TEAL);

    // 2. Taskbar at bottom (32px high)
    let taskbar_height = 32;
    let taskbar_y = display.height.saturating_sub(taskbar_height);
    display.fill_rect(0, taskbar_y, display.width, taskbar_height, Color::GRAY);
    display.fill_rect(0, taskbar_y, display.width, 1, Color::WHITE);

    // 3. Start button
    display.fill_rect(4, taskbar_y + 4, 56, 24, Color::GRAY);
    display.fill_rect(6, taskbar_y + 6, 16, 20, Color::GREEN);

    // 4. Sample window (top-left / center)
    let win_x = 40;
    let win_y = 40;
    let win_w = 320;
    let win_h = 200;
    display.fill_rect(win_x, win_y, win_w, win_h, Color::GRAY);
    display.fill_rect(win_x + 2, win_y + 2, win_w - 4, 22, Color::NAVY);
    display.fill_rect(win_x + 2, win_y + 26, win_w - 4, win_h - 28, Color::WHITE);
}
```

## 5. Rejected Alternatives

- **Flanterm Text Console**: Rejected because the mission requires a Windows-like desktop graphical environment with distinct widgets, window frames, taskbar, and background colors.
- **Direct Hardware GPU Drivers (Bochs VBE / Intel HD)**: Rejected as Limine provides a standard linear framebuffer mode pre-configured by UEFI/BIOS.
- **Dynamic Font Rendering Engine**: Font glyph rendering can be added in a subsequent iteration; initial desktop widgets and layout meet CI screen multi-color requirements without external dependencies.

## 6. Risks

- **Framebuffer Request Returns None in Headless QEMU**: Mitigated by verifying that Limine default config and QEMU standard display device (`-display none` with default virtual GPU) instantiate a valid graphical framebuffer.
- **Unsupported Pixel Format / BPP**: Limine standard framebuffer mode provides 32-bpp or 24-bpp RGB format. Mitigated by querying `bpp()`, `red_mask_shift()`, `green_mask_shift()`, `blue_mask_shift()` dynamically.

## 7. Task Breakdown

### Task 1: Framebuffer Driver and Basic Drawing Primitives
**Goal**: Register Limine FramebufferRequest, implement `kernel/src/fb.rs` for pixel and rectangle drawing, and wire into `main.rs`.
**Assigned to**: `coder`
**Acceptance Criteria**:
- `kernel/src/fb.rs` defines `Display` struct with `from_limine`, `draw_pixel`, and `fill_rect`.
- `kernel/src/main.rs` contains `FRAMEBUFFER_REQUEST` static and calls `fb` initialization.
- `cargo build --release` in `kernel/` succeeds.
- `cargo clippy --release -- -D warnings` in `kernel/` passes with 0 warnings.
- QEMU boot succeeds and prints `grenOS` on COM1.

### Task 2: Windows-like Desktop GUI Rendering
**Goal**: Implement `kernel/src/desktop.rs` with desktop background, bottom taskbar, start button, and window outlines.
**Assigned to**: `coder`
**Acceptance Criteria**:
- `kernel/src/desktop.rs` draws a teal desktop background, gray bottom taskbar with white highlight, start button, and window with dark blue title bar and white body.
- Screen output contains at least 3 distinct colours and no single colour exceeds 90% of screen pixels.
- `cargo build --release` and `cargo clippy --release -- -D warnings` succeed in `kernel/`.
- QEMU boot prints `grenOS` on COM1 and exits without panic, triple fault, or double fault.

## 8. Sources

- `https://raw.githubusercontent.com/limine-bootloader/limine-protocol/trunk/PROTOCOL.md` — Limine boot protocol specification (Framebuffer feature, Request delimiters, Base revision).
- `https://docs.rs/limine/0.5.0/limine/request/struct.FramebufferRequest.html` — `limine` crate 0.5 FramebufferRequest API.
- `https://docs.rs/limine/0.5.0/limine/framebuffer/struct.Framebuffer.html` — `limine` crate 0.5 Framebuffer accessors (`addr`, `width`, `height`, `pitch`, `bpp`, mask shifts).
- `https://docs.rs/limine/0.5.0/limine/response/struct.FramebufferResponse.html` — `limine` crate 0.5 FramebufferResponse structure and iterator.
