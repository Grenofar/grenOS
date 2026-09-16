//! The little pictures: a gear for the settings, a page for the notepad, a
//! globe for the browser. Drawn from rectangles and discs rather than taken
//! from a letter of the font — a letter in a coloured square is a placeholder,
//! and it looked like one.
//!
//! Everything is placed on a sixteenth grid of the square it is given, so the
//! same drawing holds at 14 pixels in a list and at 52 in a window, and every
//! icon takes the colour behind it so it can punch holes in itself.

use crate::fb::{Rect, Rgb, Screen};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Terminal,
    Notes,
    Files,
    Browser,
    Settings,
    About,
    Folder,
    File,
    Drive,
    Power,
    Restart,
    Reload,
    Lock,
    Shield,
    Back,
    Forward,
    Home,
    Search,
    Plus,
    Trash,
    Save,
    Star,
    Dots,
}

/// A lighter shade of the same colour, for the face of a shape.
fn lighter(colour: Rgb, amount: u8) -> Rgb {
    let mix = |c: u8| c.saturating_add(amount);
    Rgb(mix(colour.0), mix(colour.1), mix(colour.2))
}

/// A darker one, for what is behind or below.
fn darker(colour: Rgb, amount: u8) -> Rgb {
    let mix = |c: u8| c.saturating_sub(amount);
    Rgb(mix(colour.0), mix(colour.1), mix(colour.2))
}

/// A ring: a disc with a smaller disc of the background punched out.
fn ring(screen: &mut Screen, area: Rect, thickness: usize, colour: Rgb, behind: Rgb) {
    let side = area.w.min(area.h);
    let disc = Rect::new(area.x, area.y, side, side);
    screen.round(disc, side / 2, colour);
    let inner = disc.inset(thickness.max(1));
    screen.round(inner, inner.w / 2, behind);
}

/// A triangle pointing left or right.
fn arrow(screen: &mut Screen, area: Rect, colour: Rgb, left: bool) {
    let height = area.h.max(2);
    for row in 0..height {
        let from_middle = row.abs_diff(height / 2);
        let width = (height / 2).saturating_sub(from_middle) + 1;
        let x = if left { area.x } else { area.right().saturating_sub(width) };
        screen.fill(Rect::new(x, area.y + row, width, 1), colour);
    }
}

/// A triangle pointing up.
fn roof(screen: &mut Screen, area: Rect, colour: Rgb) {
    for row in 0..area.h {
        let width = ((row * 2 + 1) * area.w / area.h.max(1)).min(area.w).max(1);
        let x = area.x + (area.w - width) / 2;
        screen.fill(Rect::new(x, area.y + row, width, 1), colour);
    }
}

/// A shield: straight shoulders, and sides that close to a point.
fn shield(screen: &mut Screen, area: Rect, colour: Rgb) {
    let shoulders = area.h / 2;
    screen.fill(Rect::new(area.x, area.y, area.w, shoulders), colour);
    let point = area.h - shoulders;
    for row in 0..point {
        let width = area.w.saturating_sub(area.w * row / point.max(1));
        let x = area.x + (area.w - width) / 2;
        screen.fill(Rect::new(x, area.y + shoulders + row, width.max(1), 1), colour);
    }
}

/// A tick, drawn as two strokes.
fn tick(screen: &mut Screen, area: Rect, colour: Rgb) {
    let thickness = (area.w / 6).max(1);
    let steps = area.w / 3;
    for step in 0..steps {
        screen.fill(Rect::new(area.x + step, area.y + area.h / 2 + step, thickness, thickness), colour);
    }
    for step in 0..area.w * 2 / 3 {
        let x = area.x + steps + step;
        let y = (area.y + area.h / 2 + steps).saturating_sub(step);
        screen.fill(Rect::new(x, y, thickness, thickness), colour);
    }
}

pub fn draw(screen: &mut Screen, area: Rect, icon: Icon, tint: Rgb, behind: Rgb) {
    let side = area.w.min(area.h);
    if side < 8 {
        screen.round(area, 2, tint);
        return;
    }
    let x = area.x;
    let y = area.y + (area.h - side) / 2;
    // Sixteenths of the icon: the same drawing at any size.
    let at = |n: usize| n * side / 16;
    let px = |n: usize| x + at(n);
    let py = |n: usize| y + at(n);
    let s = |n: usize| at(n).max(1);
    let thin = (side / 16).max(1);

    match icon {
        Icon::Terminal => {
            let window = Rect::new(px(0), py(2), s(16), s(12));
            screen.round(window, s(2), Rgb(0x0A, 0x0D, 0x13));
            screen.border(window, darker(tint, 60));
            // A title strip with three dots, then a prompt.
            screen.fill(Rect::new(px(0), py(2), s(16), s(3)), Rgb(0x18, 0x1D, 0x27));
            for dot in 0..3 {
                screen.round(Rect::new(px(2) + at(2) * dot, py(3), s(1), s(1)), s(1) / 2, darker(tint, 40));
            }
            for step in 0..3 {
                screen.fill(Rect::new(px(3) + at(step), py(7) + at(step), thin, thin), tint);
            }
            for step in 0..2 {
                screen.fill(Rect::new(px(5).saturating_sub(at(step)), py(10) + at(step), thin, thin), tint);
            }
            screen.fill(Rect::new(px(8), py(11), s(5), thin), tint);
        }
        Icon::Notes | Icon::File => {
            let page = Rect::new(px(2), py(1), s(12), s(14));
            screen.round(page, s(1), tint);
            // The folded corner.
            for row in 0..at(4) {
                screen.fill(Rect::new(page.right().saturating_sub(at(4)) + row, page.y, at(4) - row, 1), behind);
            }
            screen.fill(Rect::new(page.right().saturating_sub(at(4)), page.y, thin, s(4)), darker(tint, 40));
            for line in 0..4 {
                let width = if line == 3 { s(4) } else { s(8) };
                screen.fill(Rect::new(px(4), py(6) + at(2) * line, width, thin), behind);
            }
        }
        Icon::Files | Icon::Folder => {
            // A back tab, then a lighter front face: it reads as a folder even
            // at fourteen pixels.
            screen.round(Rect::new(px(0), py(2), s(7), s(3)), s(1), darker(tint, 40));
            screen.round(Rect::new(px(0), py(4), s(16), s(10)), s(1), darker(tint, 25));
            screen.round(Rect::new(px(0), py(6), s(16), s(8)), s(1), lighter(tint, 20));
        }
        Icon::Drive => {
            screen.round(Rect::new(px(1), py(4), s(14), s(8)), s(1), tint);
            screen.fill(Rect::new(px(3), py(9), s(10), thin), behind);
            screen.round(Rect::new(px(11), py(6), s(2), s(2)), s(1), behind);
        }
        Icon::Browser => {
            ring(screen, Rect::new(px(0), py(0), s(16), s(16)), s(1), tint, behind);
            let middle = py(8);
            screen.fill(Rect::new(px(1), middle, s(14), thin), tint);
            screen.fill(Rect::new(px(1), middle.saturating_sub(at(4)), s(14), thin), lighter(tint, 20));
            screen.fill(Rect::new(px(1), middle + at(4), s(14), thin), lighter(tint, 20));
            // Two meridians, as ellipses drawn by their outline.
            screen.border(Rect::new(px(5), py(0), s(6), s(16)), tint);
            screen.fill(Rect::new(px(8), py(0), thin, s(16)), tint);
        }
        Icon::Settings => {
            let hub = Rect::new(px(3), py(3), s(10), s(10));
            screen.round(hub, hub.w / 2, tint);
            let tooth = s(4);
            let middle_x = px(8).saturating_sub(tooth / 2);
            let middle_y = py(8).saturating_sub(tooth / 2);
            screen.round(Rect::new(middle_x, py(0), tooth, s(4)), s(1), tint);
            screen.round(Rect::new(middle_x, py(12), tooth, s(4)), s(1), tint);
            screen.round(Rect::new(px(0), middle_y, s(4), tooth), s(1), tint);
            screen.round(Rect::new(px(12), middle_y, s(4), tooth), s(1), tint);
            screen.round(Rect::new(px(2), py(2), tooth, tooth), s(1), tint);
            screen.round(Rect::new(px(10), py(2), tooth, tooth), s(1), tint);
            screen.round(Rect::new(px(2), py(10), tooth, tooth), s(1), tint);
            screen.round(Rect::new(px(10), py(10), tooth, tooth), s(1), tint);
            let hole = Rect::new(px(6), py(6), s(4), s(4));
            screen.round(hole, hole.w / 2, behind);
        }
        Icon::About => {
            ring(screen, Rect::new(px(0), py(0), s(16), s(16)), s(1), tint, behind);
            screen.round(Rect::new(px(7), py(3), s(2), s(2)), s(1), tint);
            screen.round(Rect::new(px(7), py(7), s(2), s(6)), s(1) / 2, tint);
        }
        Icon::Shield => {
            shield(screen, Rect::new(px(2), py(1), s(12), s(14)), tint);
            tick(screen, Rect::new(px(5), py(5), s(6), s(5)), behind);
        }
        Icon::Power => {
            ring(screen, Rect::new(px(1), py(1), s(14), s(14)), s(1), tint, behind);
            screen.fill(Rect::new(px(6), py(0), s(4), s(5)), behind);
            screen.round(Rect::new(px(7), py(1), s(2), s(7)), s(1) / 2, tint);
        }
        Icon::Restart | Icon::Reload => {
            ring(screen, Rect::new(px(1), py(1), s(14), s(14)), s(1), tint, behind);
            screen.fill(Rect::new(px(8), py(0), s(8), s(4)), behind);
            arrow(screen, Rect::new(px(9), py(0), s(5), s(5)), tint, false);
        }
        Icon::Lock => {
            let shackle = Rect::new(px(4), py(0), s(8), s(9));
            ring(screen, shackle, s(1), tint, behind);
            screen.fill(Rect::new(shackle.x, py(5), shackle.w, s(4)), behind);
            screen.round(Rect::new(px(2), py(6), s(12), s(9)), s(1), tint);
            screen.round(Rect::new(px(7), py(9), s(2), s(3)), s(1) / 2, behind);
        }
        Icon::Back => arrow(screen, Rect::new(px(4), py(3), s(8), s(10)), tint, true),
        Icon::Forward => arrow(screen, Rect::new(px(4), py(3), s(8), s(10)), tint, false),
        Icon::Home => {
            roof(screen, Rect::new(px(0), py(2), s(16), s(7)), tint);
            screen.fill(Rect::new(px(3), py(8), s(10), s(6)), tint);
            screen.round(Rect::new(px(6), py(10), s(4), s(4)), s(1), behind);
        }
        Icon::Search => {
            ring(screen, Rect::new(px(1), py(1), s(10), s(10)), s(1), tint, behind);
            for step in 0..at(5) {
                screen.fill(Rect::new(px(10) + step, py(10) + step, thin.max(2), thin.max(2)), tint);
            }
        }
        Icon::Plus => {
            screen.round(Rect::new(px(7), py(2), s(2), s(12)), s(1) / 2, tint);
            screen.round(Rect::new(px(2), py(7), s(12), s(2)), s(1) / 2, tint);
        }
        Icon::Trash => {
            screen.round(Rect::new(px(5), py(1), s(6), s(2)), s(1) / 2, tint);
            screen.round(Rect::new(px(2), py(3), s(12), s(2)), s(1) / 2, tint);
            let body = Rect::new(px(3), py(5), s(10), s(10));
            screen.round(body, s(1), tint);
            for rib in 1..3 {
                screen.fill(Rect::new(body.x + at(3) * rib, py(7), thin, s(6)), behind);
            }
        }
        Icon::Save => {
            screen.round(Rect::new(px(1), py(1), s(14), s(14)), s(1), tint);
            screen.fill(Rect::new(px(5), py(1), s(6), s(5)), behind);
            screen.fill(Rect::new(px(4), py(9), s(8), s(6)), behind);
            screen.fill(Rect::new(px(8), py(2), s(2), s(3)), tint);
        }
        Icon::Star => {
            // Five points, drawn as a fan of rows: a bookmark, not a rating.
            let middle = px(8);
            for row in 0..at(6) {
                let width = (at(2) + row).min(at(7));
                screen.fill(Rect::new(middle.saturating_sub(width / 2), py(2) + row, width.max(1), 1), tint);
            }
            for row in 0..at(5) {
                let width = at(9).saturating_sub(row);
                screen.fill(Rect::new(middle.saturating_sub(width / 2), py(8) + row, width.max(1), 1), tint);
            }
            screen.fill(Rect::new(px(1), py(6), s(14), s(2)), tint);
        }
        Icon::Dots => {
            for dot in 0..3 {
                screen.round(Rect::new(px(7), py(2) + at(5) * dot, s(2), s(2)), s(1), tint);
            }
        }
    }
}
