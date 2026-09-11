//! The grenOS desktop, drawn once at boot: Windows 95 in spirit. A still
//! picture for now; the mouse and the keyboard come with the interrupts.

use crate::fb::{Rect, Rgb, Screen};

const WALLPAPER: Rgb = Rgb(0x00, 0x80, 0x80);
const SILVER: Rgb = Rgb(0xC0, 0xC0, 0xC0);
const LIGHT: Rgb = Rgb(0xDF, 0xDF, 0xDF);
const WHITE: Rgb = Rgb(0xFF, 0xFF, 0xFF);
const GREY: Rgb = Rgb(0x80, 0x80, 0x80);
const BLACK: Rgb = Rgb(0x00, 0x00, 0x00);
const NAVY: Rgb = Rgb(0x00, 0x00, 0x80);
const SKY: Rgb = Rgb(0x10, 0x84, 0xD0);
const MONITOR: Rgb = Rgb(0x00, 0x40, 0x80);
const RED: Rgb = Rgb(0xE8, 0x3C, 0x28);
const GREEN: Rgb = Rgb(0x3C, 0xB4, 0x3C);
const BLUE: Rgb = Rgb(0x28, 0x64, 0xDC);
const YELLOW: Rgb = Rgb(0xF0, 0xC8, 0x28);

const TITLE: &str = "Bienvenue dans grenOS";
// The 8x8 font draws é well, but ô, ê and É poorly: the text keeps to é.
const MENU: &str = "Fichier  Edition  Affichage  ?";
/// Short lines, so they keep the large font down to 1024 pixels of width.
const WELCOME: [&str; 9] = [
    "Bienvenue dans grenOS !",
    "",
    "Un noyau Rust x86_64,",
    "démarré par Limine,",
    "écrit par des agents d'IA,",
    "vérifié par la CI.",
    "",
    "Prochaines étapes :",
    "la souris et le clavier.",
];

/// The mouse pointer: `X` for its black outline, `.` for its white inside.
const POINTER: [&str; 17] = [
    "X",
    "XX",
    "X.X",
    "X..X",
    "X...X",
    "X....X",
    "X.....X",
    "X......X",
    "X.......X",
    "X........X",
    "X.....XXXXX",
    "X..X..X",
    "X.X X..X",
    "XX  X..X",
    "X    X..X",
    "     X..X",
    "      XX",
];

/// Draws the whole desktop. `clock` is the time the taskbar shows: hours, minutes.
pub fn draw(screen: &mut Screen, clock: (u8, u8)) {
    let (width, height) = (screen.width(), screen.height());
    let scale = if height >= 600 { 2 } else { 1 };
    let bar = (height / 20).clamp(28, 48);

    screen.fill(Rect::new(0, 0, width, height - bar), WALLPAPER);
    icons(screen, scale);
    let top = (height / 4).saturating_sub(bar / 2);
    window(screen, Rect::new(width / 4, top, width / 2, height / 2), scale);
    taskbar(screen, bar, clock, scale);
    pointer(screen, width * 5 / 8, height * 5 / 8);
}

/// A Windows 95 button edge: white and light grey above, grey and black below.
fn raised(screen: &mut Screen, area: Rect) {
    screen.fill(area, SILVER);
    screen.bevel(area, WHITE, BLACK);
    screen.bevel(area.inset(1), LIGHT, GREY);
}

/// The same edge, pushed in.
fn sunken(screen: &mut Screen, area: Rect, inside: Rgb) {
    screen.fill(area, inside);
    screen.bevel(area, GREY, WHITE);
    screen.bevel(area.inset(1), BLACK, LIGHT);
}

/// Text centred on `centre`, one line under the other, in white.
fn label(screen: &mut Screen, centre: usize, top: usize, lines: &[&str]) {
    for (i, line) in lines.iter().enumerate() {
        let x = centre.saturating_sub(Screen::text_width(line, 1) / 2);
        screen.text(x, top + i * 10, line, WHITE, 1);
    }
}

/// Two icons down the left edge: the computer and the recycle bin.
fn icons(screen: &mut Screen, scale: usize) {
    let size = 16 * (scale + 1);
    let x = 28;

    let top = 24;
    let monitor = Rect::new(x, top, size, size * 3 / 4);
    raised(screen, monitor);
    screen.fill(monitor.inset(4), MONITOR);
    screen.fill(Rect::new(x + size / 3, top + size * 3 / 4, size / 3, size / 8), GREY);
    screen.fill(Rect::new(x + size / 6, top + size * 7 / 8, size * 2 / 3, size / 8), SILVER);
    label(screen, x + size / 2, top + size + 6, &["Poste de", "travail"]);

    let top = top + size + 52;
    let lid = Rect::new(x + size / 8, top, size * 3 / 4, size / 8);
    let bin = Rect::new(x + size / 6, top + size / 8, size * 2 / 3, size * 7 / 8);
    screen.fill(bin, SILVER);
    screen.bevel(bin, WHITE, GREY);
    screen.fill(lid, GREY);
    for rib in 1..4 {
        screen.fill(Rect::new(bin.x + bin.w * rib / 4, bin.y + 4, 1, bin.h - 8), GREY);
    }
    label(screen, x + size / 2, top + size + 6, &["Corbeille"]);
}

#[derive(Clone, Copy)]
enum Button {
    Minimise,
    Maximise,
    Close,
}

/// One of the three buttons at the right end of a title bar.
fn title_button(screen: &mut Screen, area: Rect, button: Button) {
    raised(screen, area);
    let (cx, cy) = (area.x + area.w / 2, area.y + area.h / 2);
    let s = area.h / 4 + 1;
    match button {
        Button::Minimise => screen.fill(Rect::new(cx - s, cy + s - 2, 2 * s, 2), BLACK),
        Button::Maximise => {
            screen.bevel(Rect::new(cx - s, cy - s, 2 * s, 2 * s), BLACK, BLACK);
            screen.fill(Rect::new(cx - s, cy - s, 2 * s, 2), BLACK);
        }
        Button::Close => {
            for d in 0..2 * s {
                screen.fill(Rect::new(cx - s + d, cy - s + d, 2, 1), BLACK);
                screen.fill(Rect::new(cx + s - 2 - d, cy - s + d, 2, 1), BLACK);
            }
        }
    }
}

/// A window: its frame, its title bar in a gradient, three buttons, a menu
/// bar, and the welcome text in its white client area.
fn window(screen: &mut Screen, area: Rect, scale: usize) {
    raised(screen, area);
    let title_h = 10 * scale + 4;
    let title = Rect::new(area.x + 3, area.y + 3, area.w - 6, title_h);
    screen.gradient(title, NAVY, SKY);
    screen.text(title.x + 6, title.y + (title_h - 8 * scale) / 2, TITLE, WHITE, scale);

    let side = title_h - 4;
    let right = title.x + title.w - 2;
    title_button(screen, Rect::new(right - side, title.y + 2, side, side), Button::Close);
    title_button(screen, Rect::new(right - 2 * side - 4, title.y + 2, side, side), Button::Maximise);
    title_button(screen, Rect::new(right - 3 * side - 4, title.y + 2, side, side), Button::Minimise);

    let menu_h = 8 * scale + 8;
    screen.text(title.x + 6, title.y + title_h + 4, MENU, BLACK, scale);

    let client = Rect::new(
        area.x + 5,
        title.y + title_h + menu_h,
        area.w - 10,
        area.h.saturating_sub(title_h + menu_h + 10),
    );
    sunken(screen, client, WHITE);
    let fits = WELCOME.iter().all(|line| Screen::text_width(line, scale) + 24 <= client.w);
    let text_scale = if fits { scale } else { 1 };
    for (i, line) in WELCOME.iter().enumerate() {
        screen.text(client.x + 12, client.y + 12 + i * 12 * text_scale, line, BLACK, text_scale);
    }
}

/// The four-colour flag of the Start button.
fn flag(screen: &mut Screen, x: usize, y: usize, size: usize) {
    let half = size / 2;
    let tile = half.saturating_sub(1);
    screen.fill(Rect::new(x, y, tile, tile), RED);
    screen.fill(Rect::new(x + half, y, tile, tile), GREEN);
    screen.fill(Rect::new(x, y + half, tile, tile), BLUE);
    screen.fill(Rect::new(x + half, y + half, tile, tile), YELLOW);
}

/// `HH:MM`, for the clock.
fn clock_text((hours, minutes): (u8, u8)) -> [u8; 5] {
    [
        b'0' + hours / 10 % 10,
        b'0' + hours % 10,
        b':',
        b'0' + minutes / 10 % 10,
        b'0' + minutes % 10,
    ]
}

/// The taskbar: the Start button, the open window's button, and the clock.
fn taskbar(screen: &mut Screen, bar: usize, clock: (u8, u8), scale: usize) {
    let (width, height) = (screen.width(), screen.height());
    let top = height - bar;
    screen.fill(Rect::new(0, top, width, bar), SILVER);
    screen.fill(Rect::new(0, top, width, 1), LIGHT);
    screen.fill(Rect::new(0, top + 1, width, 1), WHITE);

    let inner = bar - 8;
    let text_y = top + 4 + (inner - 8 * scale) / 2;
    let logo = inner - 8;
    let start_label = "Démarrer";
    let start = Rect::new(4, top + 4, logo + Screen::text_width(start_label, scale) + 22, inner);
    raised(screen, start);
    flag(screen, start.x + 6, start.y + 4, logo);
    screen.text(start.x + logo + 12, text_y, start_label, BLACK, scale);

    let task = Rect::new(start.x + start.w + 6, top + 4, (width / 5).max(Screen::text_width("Bienvenue", scale) + 16), inner);
    sunken(screen, task, LIGHT);
    screen.text(task.x + 8, text_y, "Bienvenue", BLACK, scale);

    let digits = clock_text(clock);
    let time = core::str::from_utf8(&digits).unwrap_or("--:--");
    let tray_w = Screen::text_width(time, scale) + 16;
    let tray = Rect::new(width - tray_w - 4, top + 4, tray_w, inner);
    sunken(screen, tray, SILVER);
    screen.text(tray.x + 8, text_y, time, BLACK, scale);
}

/// The mouse pointer, where the mouse will be.
fn pointer(screen: &mut Screen, x: usize, y: usize) {
    for (dy, line) in POINTER.iter().enumerate() {
        for (dx, dot) in line.bytes().enumerate() {
            let colour = if dot == b'X' {
                BLACK
            } else if dot == b'.' {
                WHITE
            } else {
                continue;
            };
            screen.fill(Rect::new(x + dx, y + dy, 1, 1), colour);
        }
    }
}
