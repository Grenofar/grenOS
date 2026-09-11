//! Glyphs of the 8x8 font (the `font8x8` crate): ASCII, and Latin-1 for the
//! accents French needs.

use font8x8::UnicodeFonts;

/// The eight rows of `c`, bit 0 being the leftmost dot; a blank for a
/// character the font does not have.
pub fn glyph(c: char) -> [u8; 8] {
    font8x8::BASIC_FONTS
        .get(c)
        .or_else(|| font8x8::LATIN_FONTS.get(c))
        .unwrap_or([0; 8])
}
