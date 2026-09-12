//! Movement on screen, measured in milliseconds.
//!
//! Every animation asks the same question — how far along am I between the
//! moment it started and the moment it ends — and the answer comes from the
//! clock, never from a count of frames. Draw the desktop 24 times a second or
//! 360, a window still takes the same 160 ms to open. No floating point: the
//! progress is a number out of 1000.

/// Progress from 0 to [`FULL`], as `now` moves from `start` to
/// `start + length`.
pub const FULL: u32 = 1000;

pub fn progress(now: u64, start: u64, length: u64) -> u32 {
    if length == 0 || now <= start {
        return if length == 0 { FULL } else { 0 };
    }
    let gone = now - start;
    if gone >= length {
        return FULL;
    }
    (gone * u64::from(FULL) / length) as u32
}

/// Fast at first, slow at the end: what a window opening should feel like.
pub fn ease_out(p: u32) -> u32 {
    let left = u64::from(FULL - p.min(FULL));
    FULL - (left * left * left / (u64::from(FULL) * u64::from(FULL))) as u32
}

/// Slow at first, fast at the end: for something leaving.
pub fn ease_in(p: u32) -> u32 {
    let p = u64::from(p.min(FULL));
    (p * p / u64::from(FULL)) as u32
}

/// `from` moving to `to` at progress `p`.
pub fn mix(from: usize, to: usize, p: u32) -> usize {
    let p = usize::try_from(p.min(FULL)).unwrap_or(0);
    let full = FULL as usize;
    if to >= from {
        from + (to - from) * p / full
    } else {
        from - (from - to) * p / full
    }
}
