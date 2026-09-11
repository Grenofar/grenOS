//! The grenOS desktop: Windows 95 in spirit, and it answers. The mouse moves
//! the pointer and clicks; the keyboard types in the notepad; the clock
//! follows the CMOS clock. Everything is drawn straight into the
//! framebuffer: a change redraws what it touches, and the pointer puts back
//! the pixels it covered.

use crate::fb::{Rect, Rgb, Screen};
use crate::keyboard::Key;
use crate::mouse::Packet;

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
const ORANGE: Rgb = Rgb(0xFF, 0x8C, 0x00);

// The 8x8 font draws é well, but ô, ê and É poorly: the text keeps to é.
const MENU: &str = "Fichier  Edition  Affichage  ?";
const START: &str = "Démarrer";
const START_ITEMS: [&str; 3] = ["Bloc-notes", "Bienvenue", "Eteindre"];
/// Short lines, so they keep the large font down to 1024 pixels of width.
const WELCOME: [&str; 9] = [
    "Bienvenue dans grenOS !",
    "",
    "Un noyau Rust x86_64,",
    "démarré par Limine,",
    "écrit par des agents d'IA,",
    "vérifié par la CI.",
    "",
    "Cliquez sur Démarrer,",
    "puis sur Bloc-notes.",
];
const OFF: [&str; 2] = ["Vous pouvez maintenant éteindre", "votre ordinateur en toute sécurité."];

/// The mouse pointer: `X` for its black outline, `.` for its white inside.
const POINTER: [&str; POINTER_H] = [
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
const POINTER_W: usize = 11;
const POINTER_H: usize = 17;

/// What the notepad can hold.
const NOTES_CAP: usize = 512;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Window {
    Welcome,
    Notes,
}

impl Window {
    fn title(self) -> &'static str {
        match self {
            Window::Welcome => "Bienvenue dans grenOS",
            Window::Notes => "Bloc-notes",
        }
    }

    fn button(self) -> &'static str {
        match self {
            Window::Welcome => "Bienvenue",
            Window::Notes => "Bloc-notes",
        }
    }
}

#[derive(Clone, Copy)]
enum Button {
    Minimise,
    Maximise,
    Close,
}

pub struct Desktop {
    width: usize,
    height: usize,
    scale: usize,
    bar: usize,
    clock: (u8, u8),
    welcome_open: bool,
    notes_open: bool,
    front: Window,
    menu_open: bool,
    off: bool,
    notes: [char; NOTES_CAP],
    notes_len: usize,
    pointer: (usize, usize),
    under: [u32; POINTER_W * POINTER_H],
    left_down: bool,
}

impl Desktop {
    pub fn new(screen: &Screen, clock: (u8, u8)) -> Self {
        let (width, height) = (screen.width(), screen.height());
        Desktop {
            width,
            height,
            scale: if height >= 600 { 2 } else { 1 },
            bar: (height / 20).clamp(28, 48),
            clock,
            welcome_open: true,
            notes_open: false,
            front: Window::Welcome,
            menu_open: false,
            off: false,
            notes: [' '; NOTES_CAP],
            notes_len: 0,
            pointer: (width * 5 / 8, height * 5 / 8),
            under: [0; POINTER_W * POINTER_H],
            left_down: false,
        }
    }

    // ---- Where things are -------------------------------------------------

    fn window_rect(&self, which: Window) -> Rect {
        match which {
            Window::Welcome => Rect::new(
                self.width / 4,
                (self.height / 4).saturating_sub(self.bar / 2),
                self.width / 2,
                self.height / 2,
            ),
            Window::Notes => Rect::new(self.width / 12, self.height / 8, self.width * 5 / 12, self.height * 5 / 12),
        }
    }

    fn is_open(&self, which: Window) -> bool {
        match which {
            Window::Welcome => self.welcome_open,
            Window::Notes => self.notes_open,
        }
    }

    fn set_open(&mut self, which: Window, open: bool) {
        match which {
            Window::Welcome => self.welcome_open = open,
            Window::Notes => self.notes_open = open,
        }
    }

    /// The front window first: the order clicks are tested in.
    fn front_first(&self) -> [Window; 2] {
        if self.front == Window::Welcome {
            [Window::Welcome, Window::Notes]
        } else {
            [Window::Notes, Window::Welcome]
        }
    }

    fn title_h(&self) -> usize {
        10 * self.scale + 4
    }

    /// A window's title bar and client area.
    fn chrome(&self, area: Rect) -> (Rect, Rect) {
        let title_h = self.title_h();
        let menu_h = 8 * self.scale + 8;
        let title = Rect::new(area.x + 3, area.y + 3, area.w.saturating_sub(6), title_h);
        let client = Rect::new(
            area.x + 5,
            title.y + title_h + menu_h,
            area.w.saturating_sub(10),
            area.h.saturating_sub(title_h + menu_h + 10),
        );
        (title, client)
    }

    fn close_rect(&self, area: Rect) -> Rect {
        let side = self.title_h() - 4;
        Rect::new((area.x + area.w).saturating_sub(5 + side), area.y + 5, side, side)
    }

    fn start_rect(&self) -> Rect {
        let inner = self.bar - 8;
        Rect::new(4, self.height - self.bar + 4, inner - 8 + Screen::text_width(START, self.scale) + 22, inner)
    }

    fn menu_row(&self) -> usize {
        12 * self.scale + 8
    }

    fn menu_rect(&self) -> Rect {
        let row = self.menu_row();
        let h = row * START_ITEMS.len() + 6;
        Rect::new(4, self.height - self.bar - h, Screen::text_width("Bloc-notes", self.scale) + 48, h)
    }

    fn menu_item_rect(&self, index: usize) -> Rect {
        let menu = self.menu_rect();
        let row = self.menu_row();
        Rect::new(menu.x + 3, menu.y + 3 + index * row, menu.w - 6, row)
    }

    // ---- Drawing ----------------------------------------------------------

    /// Draws everything again, the pointer last.
    pub fn draw_all(&mut self, screen: &mut Screen) {
        if self.off {
            self.draw_off(screen);
            return;
        }
        screen.fill(Rect::new(0, 0, self.width, self.height - self.bar), WALLPAPER);
        icons(screen, self.scale);
        let [front, back] = self.front_first();
        for which in [back, front] {
            if self.is_open(which) {
                self.draw_window(screen, which, which == front);
            }
        }
        self.draw_taskbar(screen);
        if self.menu_open {
            self.draw_menu(screen);
        }
        self.show_pointer(screen);
    }

    fn draw_window(&self, screen: &mut Screen, which: Window, active: bool) {
        let area = self.window_rect(which);
        raised(screen, area);
        let (title, client) = self.chrome(area);
        if active {
            screen.gradient(title, NAVY, SKY);
        } else {
            screen.gradient(title, GREY, SILVER);
        }
        let title_colour = if active { WHITE } else { LIGHT };
        screen.text(title.x + 6, title.y + (title.h - 8 * self.scale) / 2, which.title(), title_colour, self.scale);

        let side = title.h - 4;
        let right = title.x + title.w - 2;
        title_button(screen, Rect::new(right - side, title.y + 2, side, side), Button::Close);
        title_button(screen, Rect::new(right - 2 * side - 4, title.y + 2, side, side), Button::Maximise);
        title_button(screen, Rect::new(right - 3 * side - 4, title.y + 2, side, side), Button::Minimise);
        screen.text(title.x + 6, title.y + title.h + 4, MENU, BLACK, self.scale);

        match which {
            Window::Welcome => {
                sunken(screen, client, WHITE);
                let fits = WELCOME.iter().all(|line| Screen::text_width(line, self.scale) + 24 <= client.w);
                let scale = if fits { self.scale } else { 1 };
                for (i, line) in WELCOME.iter().enumerate() {
                    screen.text(client.x + 12, client.y + 12 + i * 12 * scale, line, BLACK, scale);
                }
            }
            Window::Notes => self.draw_notes(screen, client),
        }
    }

    /// The notepad's text, wrapped to its width, with the caret at its end.
    fn draw_notes(&self, screen: &mut Screen, client: Rect) {
        sunken(screen, client, WHITE);
        let glyph = 8 * self.scale;
        let line_h = 10 * self.scale;
        let columns = (client.w.saturating_sub(24) / glyph).max(1);
        let bottom = client.y + client.h;
        let (mut column, mut row) = (0, 0);
        let mut buffer = [0u8; 4];
        for &c in self.notes.iter().take(self.notes_len) {
            if c == '\n' || column == columns {
                column = 0;
                row += 1;
            }
            if c == '\n' {
                continue;
            }
            let y = client.y + 10 + row * line_h;
            if y + glyph > bottom {
                return;
            }
            screen.text(client.x + 12 + column * glyph, y, c.encode_utf8(&mut buffer), BLACK, self.scale);
            column += 1;
        }
        let y = client.y + 10 + row * line_h;
        if y + glyph <= bottom {
            screen.fill(Rect::new(client.x + 12 + column * glyph, y + glyph - 2, glyph, 2), BLACK);
        }
    }

    fn draw_taskbar(&self, screen: &mut Screen) {
        let top = self.height - self.bar;
        screen.fill(Rect::new(0, top, self.width, self.bar), SILVER);
        screen.fill(Rect::new(0, top, self.width, 1), LIGHT);
        screen.fill(Rect::new(0, top + 1, self.width, 1), WHITE);

        let inner = self.bar - 8;
        let text_y = top + 4 + (inner - 8 * self.scale) / 2;
        let start = self.start_rect();
        if self.menu_open {
            sunken(screen, start, LIGHT);
        } else {
            raised(screen, start);
        }
        flag(screen, start.x + 6, start.y + 4, inner - 8);
        screen.text(start.x + inner + 4, text_y, START, BLACK, self.scale);

        // One button per open window, the front one pushed in.
        let width = (self.width / 6).max(Screen::text_width("Bloc-notes", self.scale) + 16);
        let mut x = start.x + start.w + 6;
        for which in [Window::Welcome, Window::Notes] {
            if !self.is_open(which) {
                continue;
            }
            let button = Rect::new(x, top + 4, width, inner);
            if which == self.front {
                sunken(screen, button, LIGHT);
            } else {
                raised(screen, button);
            }
            screen.text(button.x + 8, text_y, which.button(), BLACK, self.scale);
            x += width + 4;
        }
        self.draw_clock(screen);
    }

    fn draw_clock(&self, screen: &mut Screen) {
        let digits = clock_text(self.clock);
        let time = core::str::from_utf8(&digits).unwrap_or("--:--");
        let top = self.height - self.bar;
        let inner = self.bar - 8;
        let tray_w = Screen::text_width(time, self.scale) + 16;
        let tray = Rect::new(self.width - tray_w - 4, top + 4, tray_w, inner);
        sunken(screen, tray, SILVER);
        screen.text(tray.x + 8, top + 4 + (inner - 8 * self.scale) / 2, time, BLACK, self.scale);
    }

    fn draw_menu(&self, screen: &mut Screen) {
        let menu = self.menu_rect();
        raised(screen, menu);
        screen.fill(Rect::new(menu.x + 3, menu.y + 3, 20, menu.h - 6), NAVY);
        for (i, item) in START_ITEMS.iter().enumerate() {
            let row = self.menu_item_rect(i);
            screen.text(row.x + 28, row.y + (row.h - 8 * self.scale) / 2, item, BLACK, self.scale);
        }
    }

    /// Windows 95's last screen, after Eteindre.
    fn draw_off(&self, screen: &mut Screen) {
        screen.fill(Rect::new(0, 0, self.width, self.height), BLACK);
        for (i, line) in OFF.iter().enumerate() {
            let x = self.width.saturating_sub(Screen::text_width(line, self.scale)) / 2;
            let y = self.height / 2 + i * 14 * self.scale;
            screen.text(x, y.saturating_sub(14 * self.scale), line, ORANGE, self.scale);
        }
    }

    // ---- The pointer ------------------------------------------------------

    fn show_pointer(&mut self, screen: &mut Screen) {
        let (x, y) = self.pointer;
        for (i, pixel) in self.under.iter_mut().enumerate() {
            *pixel = screen.read_raw(x + i % POINTER_W, y + i / POINTER_W);
        }
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

    fn hide_pointer(&self, screen: &mut Screen) {
        let (x, y) = self.pointer;
        for (i, pixel) in self.under.iter().enumerate() {
            screen.write_raw(x + i % POINTER_W, y + i / POINTER_W, *pixel);
        }
    }

    // ---- What the devices change ------------------------------------------

    /// A new time for the clock; redrawn only when the minute changes.
    pub fn set_clock(&mut self, screen: &mut Screen, clock: (u8, u8)) {
        if self.off || clock == self.clock {
            return;
        }
        self.clock = clock;
        self.hide_pointer(screen);
        self.draw_clock(screen);
        self.show_pointer(screen);
    }

    pub fn mouse(&mut self, screen: &mut Screen, packet: Packet) {
        if self.off {
            return;
        }
        if packet.dx != 0 || packet.dy != 0 {
            self.hide_pointer(screen);
            let x = (self.pointer.0 as i64 + i64::from(packet.dx)).clamp(0, self.width as i64 - 1);
            let y = (self.pointer.1 as i64 + i64::from(packet.dy)).clamp(0, self.height as i64 - 1);
            self.pointer = (x as usize, y as usize);
            self.show_pointer(screen);
        }
        let pressed = packet.left && !self.left_down;
        self.left_down = packet.left;
        if pressed {
            self.click(screen);
        }
    }

    fn click(&mut self, screen: &mut Screen) {
        let (x, y) = self.pointer;
        let inside = |area: Rect| x >= area.x && x < area.x + area.w && y >= area.y && y < area.y + area.h;

        if self.menu_open {
            self.menu_open = false;
            if inside(self.menu_item_rect(0)) {
                self.notes_open = true;
                self.front = Window::Notes;
            } else if inside(self.menu_item_rect(1)) {
                self.welcome_open = true;
                self.front = Window::Welcome;
            } else if inside(self.menu_item_rect(2)) {
                self.off = true;
            }
            self.draw_all(screen);
            return;
        }
        if inside(self.start_rect()) {
            self.menu_open = true;
            self.draw_all(screen);
            return;
        }
        for which in self.front_first() {
            if !self.is_open(which) {
                continue;
            }
            let area = self.window_rect(which);
            if inside(self.close_rect(area)) {
                self.set_open(which, false);
                if let Some(other) = self.front_first().into_iter().find(|&w| w != which && self.is_open(w)) {
                    self.front = other;
                }
                self.draw_all(screen);
                return;
            }
            if inside(area) {
                if self.front != which {
                    self.front = which;
                    self.draw_all(screen);
                }
                return;
            }
        }
    }

    pub fn key(&mut self, screen: &mut Screen, key: Key) {
        if self.off || !self.notes_open {
            return;
        }
        let typed = match key {
            Key::Char(c) => Some(c),
            Key::Enter => Some('\n'),
            Key::Backspace => None,
        };
        match typed {
            Some(c) if self.notes_len < NOTES_CAP => {
                self.notes[self.notes_len] = c;
                self.notes_len += 1;
            }
            Some(_) => {}
            None => self.notes_len = self.notes_len.saturating_sub(1),
        }
        if self.front != Window::Notes {
            self.front = Window::Notes;
            self.draw_all(screen);
            return;
        }
        self.hide_pointer(screen);
        let (_, client) = self.chrome(self.window_rect(Window::Notes));
        self.draw_notes(screen, client);
        self.show_pointer(screen);
    }
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
