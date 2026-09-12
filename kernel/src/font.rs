//! The desktop's typeface: Noto Sans Mono, pre-rasterised by the
//! `noto-sans-mono-bitmap` crate. Every pixel comes as an intensity from 0 to
//! 255, which the screen mixes with what is under it, so the text is smooth
//! rather than jagged — and the accents French needs are all there.

use core::sync::atomic::{AtomicBool, Ordering};

use noto_sans_mono_bitmap::{FontWeight, RasterHeight, RasterizedChar, get_raster, get_raster_width};

/// What a piece of text is for. The sizes behind these follow the screen: on
/// a large one everything moves up a step, so the desktop stays readable at
/// 1920x1080 without being huge at 1024x768.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Font {
    /// The small print: counters, hints, the second line of a menu entry.
    Small,
    /// Everything else.
    Body,
    /// Body text that has to stand out: window titles, headings.
    Head,
    /// A screen's title.
    Title,
    /// The wordmark on the wallpaper.
    Logo,
}

static LARGE: AtomicBool = AtomicBool::new(false);

/// Sets the step from the screen's height. Called once, before anything is
/// drawn.
pub fn tune(height: usize) {
    LARGE.store(height >= 1000, Ordering::Relaxed);
}

fn metrics(style: Font) -> (FontWeight, RasterHeight) {
    let large = LARGE.load(Ordering::Relaxed);
    match style {
        Font::Small => (FontWeight::Regular, RasterHeight::Size16),
        Font::Body => (FontWeight::Regular, if large { RasterHeight::Size20 } else { RasterHeight::Size16 }),
        Font::Head => (FontWeight::Bold, if large { RasterHeight::Size20 } else { RasterHeight::Size16 }),
        Font::Title => (FontWeight::Bold, if large { RasterHeight::Size24 } else { RasterHeight::Size20 }),
        Font::Logo => (FontWeight::Bold, RasterHeight::Size32),
    }
}

/// The glyph for `c`, or the replacement one when the font has no such
/// character.
pub fn glyph(c: char, style: Font) -> Option<RasterizedChar> {
    let (weight, height) = metrics(style);
    get_raster(c, weight, height).or_else(|| get_raster('?', weight, height))
}

/// How far one character moves the pen: the font is monospaced, so this holds
/// for all of them.
pub fn width(style: Font) -> usize {
    let (weight, height) = metrics(style);
    get_raster_width(weight, height)
}

/// The height of a line in `style`.
pub fn height(style: Font) -> usize {
    metrics(style).1.val()
}
