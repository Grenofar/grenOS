//! The screen: Limine's framebuffer, a back buffer in RAM, and everything the
//! desktop is drawn with.
//!
//! Drawing goes into the back buffer, one `u32` per pixel as `0x00RRGGBB`, and
//! `present` copies a rectangle of it to the framebuffer in the format the
//! card wants. Two reasons: the desktop can read what is already there — the
//! font is anti-aliased, so every glyph pixel is mixed with its background —
//! and a redraw reaches the eye in one copy instead of being watched as it is
//! painted. A clip rectangle keeps a redraw to the part of the screen that
//! actually changed.

use crate::font::{self, Font};

/// A colour, turned into the framebuffer's pixel format when presented.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const fn pack(self) -> u32 {
        (self.0 as u32) << 16 | (self.1 as u32) << 8 | self.2 as u32
    }

    pub const fn unpack(value: u32) -> Self {
        Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
    }

    /// This colour with `alpha`/255 of `other` over it.
    pub fn mix(self, other: Rgb, alpha: u8) -> Rgb {
        let blend = |a: u8, b: u8| {
            let a = u32::from(a);
            let b = u32::from(b);
            ((a * (255 - u32::from(alpha)) + b * u32::from(alpha)) / 255) as u8
        };
        Rgb(blend(self.0, other.0), blend(self.1, other.1), blend(self.2, other.2))
    }
}

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

    pub const fn empty() -> Self {
        Rect::new(0, 0, 0, 0)
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

    /// The same rectangle, `n` pixels wider on every side.
    pub const fn grow(self, n: usize) -> Self {
        Rect {
            x: self.x.saturating_sub(n),
            y: self.y.saturating_sub(n),
            w: self.w + 2 * n,
            h: self.h + 2 * n,
        }
    }

    pub const fn right(self) -> usize {
        self.x + self.w
    }

    pub const fn bottom(self) -> usize {
        self.y + self.h
    }

    pub const fn is_empty(self) -> bool {
        self.w == 0 || self.h == 0
    }

    pub fn contains(self, x: usize, y: usize) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// What the two rectangles have in common; empty when they miss.
    pub fn intersect(self, other: Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right <= x || bottom <= y {
            return Rect::empty();
        }
        Rect::new(x, y, right - x, bottom - y)
    }

    /// The smallest rectangle holding both.
    pub fn union(self, other: Rect) -> Rect {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect::new(x, y, self.right().max(other.right()) - x, self.bottom().max(other.bottom()) - y)
    }
}

/// How the framebuffer is laid out, as Limine describes it.
#[derive(Clone, Copy)]
pub struct Mode {
    pub width: usize,
    pub height: usize,
    /// Bytes from one row of the framebuffer to the next.
    pub pitch: usize,
    pub bits_per_pixel: usize,
    pub red_shift: u8,
    pub green_shift: u8,
    pub blue_shift: u8,
}

pub struct Screen {
    front: *mut u8,
    back: *mut u32,
    mode: Mode,
    clip: Rect,
    /// The framebuffer holds exactly what the back buffer holds: presenting is
    /// then a row copy, with nothing to convert.
    plain: bool,
}

impl Screen {
    /// # Safety
    ///
    /// `front` must point to `mode.height` rows of `mode.pitch` bytes, and
    /// `back` to `mode.width * mode.height` `u32`s, both mapped and writable
    /// for as long as the `Screen` is used, and the back buffer used by
    /// nothing else.
    pub unsafe fn new(front: *mut u8, back: *mut u32, mode: Mode) -> Self {
        let plain = mode.bits_per_pixel == 32 && mode.red_shift == 16 && mode.green_shift == 8 && mode.blue_shift == 0;
        Screen { front, back, mode, clip: Rect::new(0, 0, mode.width, mode.height), plain }
    }

    pub fn width(&self) -> usize {
        self.mode.width
    }

    pub fn height(&self) -> usize {
        self.mode.height
    }

    pub fn full(&self) -> Rect {
        Rect::new(0, 0, self.mode.width, self.mode.height)
    }

    /// Keeps the drawing that follows inside `area`; returns what was in force,
    /// to put back afterwards.
    pub fn set_clip(&mut self, area: Rect) -> Rect {
        let next = area.intersect(self.full());
        core::mem::replace(&mut self.clip, next)
    }

    /// What the drawing is being kept inside.
    pub fn clip(&self) -> Rect {
        self.clip
    }

    /// Fills a rectangle, clipped.
    pub fn fill(&mut self, area: Rect, colour: Rgb) {
        let area = area.intersect(self.clip);
        if area.is_empty() {
            return;
        }
        let value = colour.pack();
        for y in area.y..area.bottom() {
            // SAFETY: the rectangle lies inside the screen, and the back
            // buffer holds width * height pixels.
            let row = unsafe { core::slice::from_raw_parts_mut(self.back.add(y * self.mode.width + area.x), area.w) };
            row.fill(value);
        }
    }

    /// Darkens or tints a rectangle: `alpha` parts of `colour` per 255.
    pub fn wash(&mut self, area: Rect, colour: Rgb, alpha: u8) {
        let area = area.intersect(self.clip);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                self.mix_pixel(x, y, colour, alpha);
            }
        }
    }

    /// A vertical gradient from `top` to `bottom`.
    pub fn gradient(&mut self, area: Rect, top: Rgb, bottom: Rgb) {
        let span = area.h.max(1) as i32;
        for dy in 0..area.h {
            let step = dy as i32;
            let mix = |a: u8, b: u8| (i32::from(a) + (i32::from(b) - i32::from(a)) * step / span) as u8;
            let colour = Rgb(mix(top.0, bottom.0), mix(top.1, bottom.1), mix(top.2, bottom.2));
            self.fill(Rect::new(area.x, area.y + dy, area.w, 1), colour);
        }
    }

    /// A one-pixel frame inside `area`.
    pub fn border(&mut self, area: Rect, colour: Rgb) {
        if area.is_empty() {
            return;
        }
        self.fill(Rect::new(area.x, area.y, area.w, 1), colour);
        self.fill(Rect::new(area.x, area.bottom() - 1, area.w, 1), colour);
        self.fill(Rect::new(area.x, area.y, 1, area.h), colour);
        self.fill(Rect::new(area.right() - 1, area.y, 1, area.h), colour);
    }

    /// A filled rectangle with its corners rounded to `radius`.
    pub fn round(&mut self, area: Rect, radius: usize, colour: Rgb) {
        let radius = radius.min(area.w / 2).min(area.h / 2);
        if radius == 0 {
            self.fill(area, colour);
            return;
        }
        self.fill(Rect::new(area.x, area.y + radius, area.w, area.h - 2 * radius), colour);
        for dy in 0..radius {
            // How far the corner cuts into this row, and how much of the pixel
            // at the edge it covers: the coverage smooths the curve.
            let t = radius - dy;
            let inset = radius - isqrt(radius * radius - (t - 1) * (t - 1)).min(radius);
            let width = area.w.saturating_sub(2 * inset);
            self.fill(Rect::new(area.x + inset, area.y + dy, width, 1), colour);
            self.fill(Rect::new(area.x + inset, area.bottom() - 1 - dy, width, 1), colour);
            if inset > 0 {
                let alpha = 140;
                self.mix_pixel(area.x + inset - 1, area.y + dy, colour, alpha);
                self.mix_pixel(area.right() - inset, area.y + dy, colour, alpha);
                self.mix_pixel(area.x + inset - 1, area.bottom() - 1 - dy, colour, alpha);
                self.mix_pixel(area.right() - inset, area.bottom() - 1 - dy, colour, alpha);
            }
        }
    }

    /// A soft shadow under a window: `depth` rings of black, fading outwards.
    pub fn shadow(&mut self, area: Rect, depth: usize) {
        for ring in 1..=depth {
            let alpha = (60 / ring).max(6) as u8;
            let outer = area.grow(ring);
            self.wash(Rect::new(outer.x, outer.y, outer.w, 1), Rgb(0, 0, 0), alpha / 2);
            self.wash(Rect::new(outer.x, outer.bottom() - 1, outer.w, 1), Rgb(0, 0, 0), alpha);
            self.wash(Rect::new(outer.x, outer.y, 1, outer.h), Rgb(0, 0, 0), alpha);
            self.wash(Rect::new(outer.right() - 1, outer.y, 1, outer.h), Rgb(0, 0, 0), alpha);
        }
    }

    /// Mixes one pixel of the back buffer, clipped.
    fn mix_pixel(&mut self, x: usize, y: usize, colour: Rgb, alpha: u8) {
        if alpha == 0 || !self.clip.contains(x, y) {
            return;
        }
        // SAFETY: the clip rectangle is inside the screen, so the pixel is one
        // of the width * height the back buffer holds.
        unsafe {
            let slot = self.back.add(y * self.mode.width + x);
            let mixed = if alpha == 255 { colour } else { Rgb::unpack(slot.read()).mix(colour, alpha) };
            slot.write(mixed.pack());
        }
    }

    /// Text in the given font, its top-left corner at (x, y); returns the x
    /// just past it.
    pub fn text(&mut self, x: usize, y: usize, text: &str, colour: Rgb, style: Font) -> usize {
        self.text_scaled(x, y, text, colour, style, 1)
    }

    /// The same, every dot drawn as a `scale` by `scale` square: for the one
    /// place that wants letters far larger than the font is rasterised at.
    pub fn text_scaled(&mut self, x: usize, y: usize, text: &str, colour: Rgb, style: Font, scale: usize) -> usize {
        let advance = font::width(style) * scale;
        let height = font::height(style) * scale;
        if y >= self.clip.bottom() || y + height <= self.clip.y {
            return x + text.chars().count() * advance;
        }
        for (index, c) in text.chars().enumerate() {
            let left = x + index * advance;
            if left >= self.clip.right() {
                break;
            }
            if left + advance <= self.clip.x || c == ' ' {
                continue;
            }
            let Some(glyph) = font::glyph(c, style) else {
                continue;
            };
            for (row, pixels) in glyph.raster().iter().enumerate() {
                for (column, &intensity) in pixels.iter().enumerate() {
                    if intensity == 0 {
                        continue;
                    }
                    if scale == 1 {
                        self.mix_pixel(left + column, y + row, colour, intensity);
                        continue;
                    }
                    for dy in 0..scale {
                        for dx in 0..scale {
                            self.mix_pixel(left + column * scale + dx, y + row * scale + dy, colour, intensity);
                        }
                    }
                }
            }
        }
        x + text.chars().count() * advance
    }

    /// The width `text` takes in `style`.
    pub fn text_width(text: &str, style: Font) -> usize {
        text.chars().count() * font::width(style)
    }

    /// Copies a rectangle of the back buffer to the framebuffer: this is when
    /// the drawing becomes visible.
    pub fn present(&mut self, area: Rect) {
        let area = area.intersect(self.full());
        if area.is_empty() {
            return;
        }
        let bytes = self.mode.bits_per_pixel / 8;
        for y in area.y..area.bottom() {
            // SAFETY: the row lies inside both buffers, which the caller of
            // `new` promised are mapped and writable.
            unsafe {
                let source = self.back.add(y * self.mode.width + area.x);
                let target = self.front.add(y * self.mode.pitch + area.x * bytes);
                if self.plain {
                    core::ptr::copy_nonoverlapping(source, target.cast::<u32>(), area.w);
                    continue;
                }
                for x in 0..area.w {
                    let value = self.encode(Rgb::unpack(source.add(x).read()));
                    let pixel = target.add(x * bytes);
                    for (i, byte) in value.to_le_bytes().iter().take(bytes).enumerate() {
                        pixel.add(i).write_volatile(*byte);
                    }
                }
            }
        }
    }

    /// One pixel written straight to the framebuffer, leaving the back buffer
    /// alone: the mouse pointer, which `present` then wipes away.
    pub fn front_pixel(&mut self, x: usize, y: usize, colour: Rgb) {
        if x >= self.mode.width || y >= self.mode.height {
            return;
        }
        let bytes = self.mode.bits_per_pixel / 8;
        let value = self.encode(colour);
        // SAFETY: (x, y) is on the screen, so its bytes lie in the framebuffer.
        unsafe {
            let pixel = self.front.add(y * self.mode.pitch + x * bytes);
            if self.plain {
                pixel.cast::<u32>().write_volatile(value);
                return;
            }
            for (i, byte) in value.to_le_bytes().iter().take(bytes).enumerate() {
                pixel.add(i).write_volatile(*byte);
            }
        }
    }

    fn encode(&self, colour: Rgb) -> u32 {
        u32::from(colour.0) << self.mode.red_shift
            | u32::from(colour.1) << self.mode.green_shift
            | u32::from(colour.2) << self.mode.blue_shift
    }
}

/// The whole part of a square root, for the rounded corners.
fn isqrt(value: usize) -> usize {
    let mut root = 0;
    while (root + 1) * (root + 1) <= value {
        root += 1;
    }
    root
}
