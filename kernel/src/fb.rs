//! The screen: Limine's linear framebuffer, and the primitives the desktop is
//! drawn with. Every drawing call ends in `Screen::fill`, which clips to the
//! screen, so none of them can write outside the framebuffer.

use crate::font;

/// A colour, turned into the framebuffer's pixel format when drawn.
#[derive(Clone, Copy)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// A rectangle, in pixels.
#[derive(Clone, Copy)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

impl Rect {
    pub const fn new(x: usize, y: usize, w: usize, h: usize) -> Self {
        Rect { x, y, w, h }
    }

    /// The rectangle `n` pixels inside this one.
    pub const fn inset(self, n: usize) -> Self {
        Rect {
            x: self.x + n,
            y: self.y + n,
            w: self.w.saturating_sub(2 * n),
            h: self.h.saturating_sub(2 * n),
        }
    }
}

/// How the framebuffer is laid out, as Limine describes it.
#[derive(Clone, Copy)]
pub struct Mode {
    pub width: usize,
    pub height: usize,
    /// Bytes from one row to the next.
    pub pitch: usize,
    pub bits_per_pixel: usize,
    pub red_shift: u8,
    pub green_shift: u8,
    pub blue_shift: u8,
}

pub struct Screen {
    base: *mut u8,
    mode: Mode,
}

impl Screen {
    /// # Safety
    ///
    /// `base` must point to `mode.height` rows of `mode.pitch` bytes, mapped
    /// and writable for as long as the `Screen` is used.
    pub unsafe fn new(base: *mut u8, mode: Mode) -> Self {
        Screen { base, mode }
    }

    pub fn width(&self) -> usize {
        self.mode.width
    }

    pub fn height(&self) -> usize {
        self.mode.height
    }

    fn encode(&self, colour: Rgb) -> u32 {
        (u32::from(colour.0) << self.mode.red_shift)
            | (u32::from(colour.1) << self.mode.green_shift)
            | (u32::from(colour.2) << self.mode.blue_shift)
    }

    /// Fills a rectangle, clipped to the screen.
    pub fn fill(&mut self, area: Rect, colour: Rgb) {
        let x_end = area.x.saturating_add(area.w).min(self.mode.width);
        let y_end = area.y.saturating_add(area.h).min(self.mode.height);
        if area.x >= x_end || area.y >= y_end {
            return;
        }
        let value = self.encode(colour);
        let bytes = self.mode.bits_per_pixel / 8;
        for y in area.y..y_end {
            // SAFETY: y < height and every x < width, so each pixel lies in
            // the rows of pitch bytes that `new` was promised.
            unsafe {
                let row = self.base.add(y * self.mode.pitch);
                for x in area.x..x_end {
                    let pixel = row.add(x * bytes);
                    if bytes == 4 {
                        pixel.cast::<u32>().write_volatile(value);
                    } else {
                        for (i, byte) in value.to_le_bytes().iter().take(bytes).enumerate() {
                            pixel.add(i).write_volatile(*byte);
                        }
                    }
                }
            }
        }
    }

    /// A one-pixel frame, `light` on the top and left and `dark` on the
    /// bottom and right: a raised edge, or a sunken one with the two swapped.
    pub fn bevel(&mut self, area: Rect, light: Rgb, dark: Rgb) {
        if area.w == 0 || area.h == 0 {
            return;
        }
        self.fill(Rect::new(area.x, area.y, area.w, 1), light);
        self.fill(Rect::new(area.x, area.y, 1, area.h), light);
        self.fill(Rect::new(area.x, area.y + area.h - 1, area.w, 1), dark);
        self.fill(Rect::new(area.x + area.w - 1, area.y, 1, area.h), dark);
    }

    /// A horizontal gradient from `left` to `right`, as on a title bar.
    pub fn gradient(&mut self, area: Rect, left: Rgb, right: Rgb) {
        let span = area.w.max(1) as i32;
        for dx in 0..area.w {
            let step = dx as i32;
            let mix = |a: u8, b: u8| (i32::from(a) + (i32::from(b) - i32::from(a)) * step / span) as u8;
            let colour = Rgb(mix(left.0, right.0), mix(left.1, right.1), mix(left.2, right.2));
            self.fill(Rect::new(area.x + dx, area.y, 1, area.h), colour);
        }
    }

    /// The raw pixel at (x, y), to be put back later; 0 off the screen.
    pub fn read_raw(&self, x: usize, y: usize) -> u32 {
        if x >= self.mode.width || y >= self.mode.height {
            return 0;
        }
        let bytes = self.mode.bits_per_pixel / 8;
        let mut value = [0u8; 4];
        // SAFETY: (x, y) is on the screen, so its bytes lie in the framebuffer.
        unsafe {
            let pixel = self.base.add(y * self.mode.pitch + x * bytes);
            for (i, byte) in value.iter_mut().take(bytes).enumerate() {
                *byte = pixel.add(i).read_volatile();
            }
        }
        u32::from_le_bytes(value)
    }

    /// Puts back a pixel `read_raw` returned; nothing off the screen.
    pub fn write_raw(&mut self, x: usize, y: usize, value: u32) {
        if x >= self.mode.width || y >= self.mode.height {
            return;
        }
        let bytes = self.mode.bits_per_pixel / 8;
        // SAFETY: (x, y) is on the screen, so its bytes lie in the framebuffer.
        unsafe {
            let pixel = self.base.add(y * self.mode.pitch + x * bytes);
            for (i, byte) in value.to_le_bytes().iter().take(bytes).enumerate() {
                pixel.add(i).write_volatile(*byte);
            }
        }
    }

    /// Text in the 8x8 font, each dot drawn as a `scale` x `scale` square.
    pub fn text(&mut self, x: usize, y: usize, text: &str, colour: Rgb, scale: usize) {
        for (i, c) in text.chars().enumerate() {
            let left = x + i * 8 * scale;
            for (row, bits) in font::glyph(c).iter().enumerate() {
                for col in 0..8 {
                    if bits & (1 << col) != 0 {
                        self.fill(Rect::new(left + col * scale, y + row * scale, scale, scale), colour);
                    }
                }
            }
        }
    }

    /// The width `text` takes at `scale`.
    pub fn text_width(text: &str, scale: usize) -> usize {
        text.chars().count() * 8 * scale
    }
}
