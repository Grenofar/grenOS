//! The little pictures: a gear for the settings, a page for the notepad, a
//! globe for the browser. Drawn from rectangles and discs rather than taken
//! from a letter of the font — a letter in a coloured square is a placeholder,
//! and it looked like one.
//!
//! Every icon draws inside the square it is given, at any size, and takes the
//! colour behind it so it can punch holes in itself.

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
    Power,
    Restart,
    Lock,
    Back,
    Forward,
    Home,
    Search,
    Plus,
    Trash,
    Save,
}

/// A ring: a disc with a smaller disc of the background punched out.
fn ring(screen: &mut Screen, area: Rect, thickness: usize, colour: Rgb, behind: Rgb) {
    let side = area.w.min(area.h);
    let disc = Rect::new(area.x, area.y, side, side);
    screen.round(disc, side / 2, colour);
    let inner = disc.inset(thickness);
    screen.round(inner, inner.w / 2, behind);
}

/// A triangle pointing left or right, filled row by row.
fn arrow(screen: &mut Screen, area: Rect, colour: Rgb, left: bool) {
    let height = area.h.max(2);
    for row in 0..height {
        let from_middle = row.abs_diff(height / 2);
        let width = (height / 2).saturating_sub(from_middle) + 1;
        let x = if left { area.x } else { area.right().saturating_sub(width) };
        screen.fill(Rect::new(x, area.y + row, width, 1), colour);
    }
}

/// A triangle pointing up, for the roof of the home icon.
fn roof(screen: &mut Screen, area: Rect, colour: Rgb) {
    for row in 0..area.h {
        let width = (row * 2 + 1).min(area.w);
        let x = area.x + (area.w - width) / 2;
        screen.fill(Rect::new(x, area.y + row, width, 1), colour);
    }
}

pub fn draw(screen: &mut Screen, area: Rect, icon: Icon, tint: Rgb, behind: Rgb) {
    let side = area.w.min(area.h);
    if side < 8 {
        screen.round(area, 2, tint);
        return;
    }
    // Everything is measured in eighths of the icon, so it holds at any size.
    let u = side / 8;
    let unit = u.max(1);
    let x = area.x;
    let y = area.y + (area.h - side) / 2;
    let dim = Rgb(tint.0 / 2 + 40, tint.1 / 2 + 40, tint.2 / 2 + 40);

    match icon {
        Icon::Terminal => {
            let box_area = Rect::new(x, y + unit, side, side - 2 * unit);
            screen.round(box_area, unit, Rgb(0x0A, 0x0D, 0x13));
            screen.border(box_area, tint);
            // A chevron and an underscore, as a prompt.
            for step in 0..3 {
                screen.fill(Rect::new(x + 2 * unit + step * unit, y + 2 * unit + step * unit, unit, unit), tint);
            }
            for step in 0..2 {
                screen.fill(Rect::new(x + 3 * unit - step * unit, y + 4 * unit + step * unit, unit, unit), tint);
            }
            screen.fill(Rect::new(x + 4 * unit, y + 5 * unit, 3 * unit, unit), tint);
        }
        Icon::Notes | Icon::File => {
            let page = Rect::new(x + unit, y, side - 2 * unit, side);
            screen.round(page, unit, tint);
            // The folded corner, and the lines of text.
            for row in 0..2 * unit {
                screen.fill(Rect::new(page.right() - 2 * unit + row, page.y, 2 * unit - row, 1), behind);
            }
            for line in 0..3 {
                let width = if line == 2 { (side - 4 * unit) / 2 } else { side - 4 * unit };
                screen.fill(Rect::new(page.x + unit, page.y + (3 + line * 2) * unit, width, unit.max(1)), behind);
            }
        }
        Icon::Files | Icon::Folder => {
            screen.round(Rect::new(x, y + 2 * unit, 4 * unit, 2 * unit), unit / 2, dim);
            screen.round(Rect::new(x, y + 3 * unit, side, 4 * unit), unit, tint);
        }
        Icon::Browser => {
            ring(screen, Rect::new(x, y, side, side), unit.max(1), tint, behind);
            let middle = y + side / 2;
            screen.fill(Rect::new(x + unit / 2, middle, side - unit, unit.max(1)), tint);
            screen.fill(Rect::new(x + side / 2 - unit / 2, y + unit / 2, unit.max(1), side - unit), tint);
            // A narrower ring inside suggests the meridians of a globe.
            let inner = Rect::new(x + 2 * unit, y + unit / 2, side - 4 * unit, side - unit);
            screen.border(inner, tint);
        }
        Icon::Settings => {
            // A gear: a disc, eight teeth, and a hole.
            let disc = Rect::new(x + unit, y + unit, side - 2 * unit, side - 2 * unit);
            screen.round(disc, disc.w / 2, tint);
            let tooth = (2 * unit).max(2);
            let centre_x = x + side / 2 - tooth / 2;
            let centre_y = y + side / 2 - tooth / 2;
            screen.fill(Rect::new(centre_x, y, tooth, 2 * unit), tint);
            screen.fill(Rect::new(centre_x, area.y + side - 2 * unit, tooth, 2 * unit), tint);
            screen.fill(Rect::new(x, centre_y, 2 * unit, tooth), tint);
            screen.fill(Rect::new(x + side - 2 * unit, centre_y, 2 * unit, tooth), tint);
            let hole = Rect::new(x + 3 * unit, y + 3 * unit, side - 6 * unit, side - 6 * unit);
            screen.round(hole, hole.w / 2, behind);
        }
        Icon::About => {
            ring(screen, Rect::new(x, y, side, side), unit.max(1), tint, behind);
            screen.fill(Rect::new(x + side / 2 - unit / 2, y + 2 * unit, unit.max(1), unit.max(1)), tint);
            screen.fill(Rect::new(x + side / 2 - unit / 2, y + 4 * unit, unit.max(1), 2 * unit), tint);
        }
        Icon::Power => {
            ring(screen, Rect::new(x, y, side, side), unit.max(1), tint, behind);
            screen.fill(Rect::new(x + side / 2 - unit, y, 2 * unit, 3 * unit), behind);
            screen.fill(Rect::new(x + side / 2 - unit / 2, y + unit / 2, unit.max(1), 3 * unit), tint);
        }
        Icon::Restart => {
            ring(screen, Rect::new(x, y, side, side), unit.max(1), tint, behind);
            screen.fill(Rect::new(x + side / 2, y, side / 2, 2 * unit), behind);
            arrow(screen, Rect::new(x + side / 2, y, 2 * unit, 3 * unit), tint, false);
        }
        Icon::Lock => {
            let body = Rect::new(x + unit, y + 3 * unit, side - 2 * unit, side - 4 * unit);
            let shackle = Rect::new(x + 2 * unit, y, side - 4 * unit, 5 * unit);
            ring(screen, shackle, unit.max(1), tint, behind);
            screen.fill(Rect::new(shackle.x, y + 3 * unit, shackle.w, 2 * unit), behind);
            screen.round(body, unit, tint);
            screen.fill(Rect::new(x + side / 2 - unit / 2, body.y + unit, unit.max(1), 2 * unit), behind);
        }
        Icon::Back => arrow(screen, Rect::new(x + 2 * unit, y + unit, side - 4 * unit, side - 2 * unit), tint, true),
        Icon::Forward => arrow(screen, Rect::new(x + 2 * unit, y + unit, side - 4 * unit, side - 2 * unit), tint, false),
        Icon::Home => {
            roof(screen, Rect::new(x, y + unit, side, 3 * unit), tint);
            screen.fill(Rect::new(x + 2 * unit, y + 4 * unit, side - 4 * unit, 3 * unit), tint);
            screen.fill(Rect::new(x + side / 2 - unit, y + 5 * unit, 2 * unit, 2 * unit), behind);
        }
        Icon::Search => {
            ring(screen, Rect::new(x, y, 6 * unit, 6 * unit), unit.max(1), tint, behind);
            for step in 0..3 {
                screen.fill(Rect::new(x + 5 * unit + step * unit / 2, y + 5 * unit + step * unit / 2, unit.max(1), unit.max(1)), tint);
            }
        }
        Icon::Plus => {
            screen.fill(Rect::new(x + side / 2 - unit / 2, y + unit, unit.max(1), side - 2 * unit), tint);
            screen.fill(Rect::new(x + unit, y + side / 2 - unit / 2, side - 2 * unit, unit.max(1)), tint);
        }
        Icon::Trash => {
            screen.fill(Rect::new(x + 2 * unit, y + unit, side - 4 * unit, unit.max(1)), tint);
            screen.fill(Rect::new(x + unit, y + 2 * unit, side - 2 * unit, unit.max(1)), tint);
            let body = Rect::new(x + 2 * unit, y + 3 * unit, side - 4 * unit, side - 4 * unit);
            screen.round(body, unit / 2, tint);
            for rib in 1..3 {
                screen.fill(Rect::new(body.x + rib * body.w / 3, body.y + unit, unit.max(1) / 2 + 1, body.h - 2 * unit), behind);
            }
        }
        Icon::Save => {
            screen.round(Rect::new(x, y, side, side), unit, tint);
            screen.fill(Rect::new(x + 2 * unit, y, side - 4 * unit, 3 * unit), behind);
            screen.fill(Rect::new(x + 2 * unit, y + 4 * unit, side - 4 * unit, 4 * unit), behind);
        }
    }
}
