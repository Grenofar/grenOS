//! The loading screen: what the machine shows while the kernel checks itself.
//!
//! Until the desktop exists, the screen would stay black from Limine's handover
//! to `desktop: drawn`, while the kernel measures its memory, proves its
//! cryptography, finds its disks and starts the network. This module draws the
//! same badge, name and bar the desktop's own splash fades in from, so the
//! hand-over is seamless: one look, from the first framebuffer to the desktop.

use crate::fb::{Rgb, Screen};
use crate::font::Font;
use crate::icons::{self, Icon};

/// The colours the desktop's splash uses, so the two are indistinguishable.
const BACKGROUND: Rgb = Rgb(0x05, 0x07, 0x0D);
const ACCENT: Rgb = Rgb(0x2F, 0x7D, 0xF6);
const TEXT: Rgb = Rgb(0xE4, 0xE9, 0xF1);
const TEXT_DIM: Rgb = Rgb(0x9C, 0xA7, 0xB9);
const TRACK: Rgb = Rgb(0x1B, 0x20, 0x2B);

/// Draws the loading screen: the grenOS badge, the name, what the kernel is
/// doing, and a bar `done` out of `total` long, then presents the screen.
pub fn draw(screen: &mut Screen, step: &str, done: usize, total: usize) {
    let width = screen.width();
    let height = screen.height();
    screen.fill(screen.full(), BACKGROUND);

    // The badge: a 64-pixel rounded square in the accent colour, with the
    // home icon inside, centred a little above the middle.
    let badge = 64;
    let bx = width / 2 - badge / 2;
    let by = height / 2 - badge / 2 - 40;
    screen.round(crate::fb::Rect::new(bx, by, badge, badge), 14, ACCENT);
    icons::draw(screen, crate::fb::Rect::new(bx + 14, by + 14, badge - 28, badge - 28), Icon::Home, BACKGROUND, ACCENT);

    // The name, centred under the badge, then the step under the name.
    let name = "grenOS";
    let name_width = Screen::text_width(name, Font::Title);
    screen.text(width / 2 - name_width / 2, by + badge + 18, name, TEXT, Font::Title);
    let step_width = Screen::text_width(step, Font::Small);
    screen.text(width / 2 - step_width / 2, by + badge + 18 + crate::font::height(Font::Title) + 10, step, TEXT_DIM, Font::Small);

    // The bar: 180 x 4, the filled part in the accent colour.
    let bar_width = 180;
    let bar_x = width / 2 - bar_width / 2;
    let bar_y = by + badge + 18 + crate::font::height(Font::Title) + 10 + crate::font::height(Font::Small) + 22;
    screen.round(crate::fb::Rect::new(bar_x, bar_y, bar_width, 4), 2, TRACK);
    let filled = if total == 0 { 0 } else { bar_width * done.min(total) / total };
    if filled > 0 {
        screen.round(crate::fb::Rect::new(bar_x, bar_y, filled, 4), 2, ACCENT);
    }

    screen.present(screen.full());
}
