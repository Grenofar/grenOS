//! The grenOS desktop: a dark panel at the top, a menu of applications, and
//! windows with a shadow — a Linux desktop in the spirit of Kali, not a copy
//! of one. It answers the mouse and the keyboard, and everything it knows
//! about the machine comes from the kernel in a [`Machine`].
//!
//! Nothing here touches the hardware: the desktop changes its own state, says
//! which part of the screen that spoiled, and `frame` paints it. That is what
//! lets the same code run on the host, in the preview harness, and be looked
//! at before it ever reaches an image.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::fb::{Rect, Rgb, Screen};
use crate::font::{self, Font};
use crate::keyboard::Key;
use crate::mouse::Packet;
use crate::shell::{Context, Line, Shell};
use crate::time::DateTime;

// ---- Colours --------------------------------------------------------------

const WALL_TOP: Rgb = Rgb(0x10, 0x18, 0x2A);
const WALL_BOTTOM: Rgb = Rgb(0x05, 0x07, 0x0D);
const WALL_MARK: Rgb = Rgb(0x1B, 0x27, 0x40);
const PANEL: Rgb = Rgb(0x13, 0x17, 0x21);
const SURFACE: Rgb = Rgb(0x1B, 0x20, 0x2B);
const SURFACE_ALT: Rgb = Rgb(0x15, 0x19, 0x23);
const SUNKEN: Rgb = Rgb(0x11, 0x15, 0x1E);
const LINE: Rgb = Rgb(0x2B, 0x32, 0x42);
const TEXT: Rgb = Rgb(0xE4, 0xE9, 0xF1);
const TEXT_DIM: Rgb = Rgb(0x9C, 0xA7, 0xB9);
const TEXT_FAINT: Rgb = Rgb(0x64, 0x70, 0x84);
const ACCENT: Rgb = Rgb(0x2F, 0x7D, 0xF6);
const ACCENT_SOFT: Rgb = Rgb(0x1B, 0x33, 0x5C);
const GREEN: Rgb = Rgb(0x4C, 0xC3, 0x8A);
const RED: Rgb = Rgb(0xE5, 0x54, 0x54);
const AMBER: Rgb = Rgb(0xE8, 0xB3, 0x39);
const TERMINAL_BG: Rgb = Rgb(0x0A, 0x0D, 0x13);
const NOTES_BG: Rgb = Rgb(0x12, 0x16, 0x1F);
const WHITE: Rgb = Rgb(0xFF, 0xFF, 0xFF);
const BLACK: Rgb = Rgb(0x00, 0x00, 0x00);

// ---- What the kernel tells the desktop ------------------------------------

/// Everything the desktop shows about the machine, gathered at boot.
pub struct Machine {
    pub version: &'static str,
    pub build: &'static str,
    pub built_at: &'static str,
    pub cpu: String,
    /// Megabytes of usable memory, and how many are still free.
    pub memory: (u64, u64),
    /// Bytes of the kernel heap in use, and its size.
    pub heap: (usize, usize),
    pub screen: String,
    pub paging: String,
    pub acpi: String,
    /// One line per PCI function found.
    pub devices: Vec<String>,
    /// The boot log, which the terminal opens on.
    pub log: Vec<String>,
    /// The mouse answered when the kernel set it up.
    pub mouse: bool,
    /// ACPI gave the registers that turn the machine off.
    pub can_power_off: bool,
}

impl Machine {
    /// The summary Paramètres and `grenfetch` show.
    pub fn lines(&self) -> Vec<String> {
        alloc::vec![
            format!("Processeur : {}", self.cpu),
            format!("Mémoire : {} Mo, dont {} Mo libres", self.memory.0, self.memory.1),
            format!("Tas du noyau : {} Ko sur {} Ko", self.heap.0 / 1024, self.heap.1 / 1024),
            format!("Écran : {}", self.screen),
            format!("Pagination : {}", self.paging),
            format!("Micrologiciel : {}", self.acpi),
            format!("Périphériques PCI : {}", self.devices.len()),
            format!(
                "Extinction : {}",
                if self.can_power_off { "par ACPI, depuis le menu" } else { "pas de registre ACPI, arrêt seulement" }
            ),
        ]
    }
}

/// How much the input devices have said since boot, for the panel and for
/// Paramètres. On a machine where the pointer will not move, these numbers
/// are the whole diagnosis.
#[derive(Clone, Copy, Default)]
pub struct Input {
    pub mouse_bytes: u32,
    pub key_bytes: u32,
    pub mouse_irq: u32,
    pub key_irq: u32,
    pub swept: u32,
    pub lost: u32,
    pub packets: u32,
    pub keys: u32,
    /// Seconds since the kernel started.
    pub uptime: u32,
}

/// What the desktop asks the kernel to do.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Reboot,
    PowerOff,
}

// ---- Applications ---------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum App {
    Terminal,
    Notes,
    Settings,
    About,
}

const APPS: [App; 4] = [App::Terminal, App::Notes, App::Settings, App::About];

impl App {
    fn index(self) -> usize {
        self as usize
    }

    fn title(self) -> &'static str {
        match self {
            App::Terminal => "Terminal",
            App::Notes => "Bloc-notes",
            App::Settings => "Paramètres",
            App::About => "À propos de grenOS",
        }
    }

    fn short(self) -> &'static str {
        match self {
            App::About => "À propos",
            other => other.title(),
        }
    }

    fn letter(self) -> &'static str {
        match self {
            App::Terminal => ">",
            App::Notes => "N",
            App::Settings => "P",
            App::About => "i",
        }
    }

    fn tint(self) -> Rgb {
        match self {
            App::Terminal => Rgb(0x25, 0x2C, 0x3A),
            App::Notes => Rgb(0x2A, 0x3D, 0x2E),
            App::Settings => ACCENT_SOFT,
            App::About => Rgb(0x3A, 0x2E, 0x22),
        }
    }
}

/// One window: where it is, and whether it is on screen.
#[derive(Clone, Copy)]
struct Window {
    open: bool,
    area: Rect,
    /// Where to go back to when it stops filling the screen.
    restore: Rect,
    full: bool,
    space: u8,
}

/// A line of the menu.
#[derive(Clone, Copy)]
enum Item {
    Open(App),
    Reboot,
    Off,
}

const ITEMS: [(&str, &str, Item); 6] = [
    ("Terminal", "La ligne de commande de grenOS", Item::Open(App::Terminal)),
    ("Bloc-notes", "Écrire, sur le tas du noyau", Item::Open(App::Notes)),
    ("Paramètres", "Système, souris, écran, mise à jour", Item::Open(App::Settings)),
    ("À propos", "La version, et qui a écrit tout ça", Item::Open(App::About)),
    ("Redémarrer", "Relancer la machine", Item::Reboot),
    ("Éteindre", "Arrêter la machine", Item::Off),
];

const SECTIONS: [&str; 5] = ["Système", "Souris et clavier", "Écran", "Matériel", "Mise à jour"];

/// What the current version brought, shown in Mise à jour. One line per
/// change, and the list grows with each release.
const CHANGES: [&str; 6] = [
    "Interface sombre : panneau, menu, fenêtres à ombre portée",
    "Police Noto Sans Mono lissée, accents compris",
    "Souris réparée : les octets sont triés par le registre d'état",
    "Le minuteur ramasse ce qu'une interruption manquée a laissé",
    "Terminal avec commandes, Paramètres, À propos",
    "Mémoire, pagination, tas, énumération PCI, extinction ACPI",
];

/// What is not there yet, so the human knows what the next version brings.
const NEXT: [&str; 4] = [
    "Disque SATA (AHCI) et système de fichiers",
    "Carte réseau, puis la vérification en ligne des mises à jour",
    "Plusieurs bureaux avec leurs fenêtres, et le redimensionnement",
    "Clavier USB pour les PC sans PS/2",
];

const ABOUT: [&str; 6] = [
    "grenOS est un noyau x86_64 écrit en Rust, sans système",
    "d'exploitation dessous : il démarre avec Limine, parle au",
    "matériel lui-même et dessine ce bureau dans le framebuffer.",
    "",
    "Il est écrit par une équipe d'agents autonomes, pilotée depuis",
    "grenos-dev.vercel.app, et rien n'arrive ici sans une CI verte.",
];

/// The mouse pointer: `X` is its outline, `.` its inside.
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

const NOTES_CAP: usize = 8192;
const NOTES_HELLO: &str = "Ce texte vit sur le tas du noyau.\nTapez, effacez : c'est de la mémoire allouée par grenOS.\n\n";

pub struct Desktop {
    width: usize,
    height: usize,
    panel: usize,
    now: DateTime,
    input: Input,
    machine: Machine,
    windows: [Window; 4],
    /// Back to front.
    order: [App; 4],
    space: u8,
    menu: bool,
    filter: String,
    choice: usize,
    power_menu: bool,
    shell: Shell,
    notes: Vec<char>,
    section: usize,
    update_note: usize,
    off: bool,
    pointer: (usize, usize),
    shown: Option<(usize, usize)>,
    grab: Option<(App, usize, usize)>,
    left_down: bool,
    dirty: Rect,
}

impl Desktop {
    pub fn new(screen: &Screen, now: DateTime, machine: Machine) -> Self {
        let (width, height) = (screen.width(), screen.height());
        let panel = font::height(Font::Body) + 16;
        let windows = APPS.map(|app| {
            let area = default_rect(app, width, height, panel);
            Window { open: matches!(app, App::Terminal | App::About), area, restore: area, full: false, space: 1 }
        });
        let shell = Shell::new(&machine.log);
        let mut desktop = Desktop {
            width,
            height,
            panel,
            now,
            input: Input::default(),
            machine,
            windows,
            order: [App::Settings, App::Notes, App::About, App::Terminal],
            space: 1,
            menu: false,
            filter: String::new(),
            choice: 0,
            power_menu: false,
            shell,
            notes: NOTES_HELLO.chars().collect(),
            section: 0,
            update_note: 0,
            off: false,
            pointer: (width * 9 / 16, height * 5 / 8),
            shown: None,
            grab: None,
            left_down: false,
            dirty: Rect::new(0, 0, width, height),
        };
        desktop.section = 0;
        desktop
    }

    // ---- What the kernel calls --------------------------------------------

    /// A new second: the clock, and the counters in the panel.
    pub fn tick(&mut self, now: DateTime, input: Input) {
        let same_counts = input.mouse_bytes == self.input.mouse_bytes && input.key_bytes == self.input.key_bytes;
        if self.off || (now.second == self.now.second && same_counts) {
            return;
        }
        self.now = now;
        self.input = input;
        let panel = Rect::new(0, 0, self.width, self.panel);
        self.damage(panel);
        // Paramètres shows the same numbers, live.
        if self.showing(App::Settings) && self.section == 1 {
            let area = self.windows[App::Settings.index()].area;
            self.damage(area);
        }
    }

    /// Where the pointer is: the kernel says so on the serial line when the
    /// first packet arrives, and the preview harness follows it.
    pub fn pointer(&self) -> (usize, usize) {
        self.pointer
    }

    /// Paints what changed, and puts the pointer back on top.
    pub fn frame(&mut self, screen: &mut Screen) {
        let dirty = core::mem::replace(&mut self.dirty, Rect::empty());
        let moved = self.shown != Some(self.pointer);
        if dirty.is_empty() && !moved {
            return;
        }
        if !dirty.is_empty() {
            let previous = screen.set_clip(dirty);
            self.paint(screen);
            screen.set_clip(previous);
            screen.present(dirty);
        }
        if let Some((x, y)) = self.shown {
            if moved {
                // The pointer lives on the framebuffer alone: putting the back
                // buffer back where it was wipes it.
                screen.present(Rect::new(x, y, POINTER_W, POINTER_H));
            }
        }
        if !self.off {
            self.draw_pointer(screen);
            self.shown = Some(self.pointer);
        }
    }

    fn damage(&mut self, area: Rect) {
        self.dirty = self.dirty.union(area.intersect(Rect::new(0, 0, self.width, self.height)));
    }

    fn damage_all(&mut self) {
        self.dirty = Rect::new(0, 0, self.width, self.height);
    }

    // ---- Windows ----------------------------------------------------------

    fn showing(&self, app: App) -> bool {
        let window = self.windows[app.index()];
        window.open && window.space == self.space
    }

    fn front(&self) -> Option<App> {
        self.order.iter().rev().copied().find(|&app| self.showing(app))
    }

    fn visible(&self) -> impl Iterator<Item = App> + '_ {
        APPS.into_iter().filter(|&app| self.showing(app))
    }

    fn raise(&mut self, app: App) {
        if let Some(at) = self.order.iter().position(|&other| other == app) {
            self.order[at..].rotate_left(1);
        }
    }

    fn open(&mut self, app: App) {
        let window = &mut self.windows[app.index()];
        window.open = true;
        window.space = self.space;
        self.raise(app);
        self.damage_all();
    }

    fn close(&mut self, app: App) {
        self.windows[app.index()].open = false;
        self.damage_all();
    }

    // ---- Geometry ---------------------------------------------------------

    fn line_h(&self) -> usize {
        font::height(Font::Body) + 4
    }

    fn title_h(&self) -> usize {
        font::height(Font::Head) + 14
    }

    pub fn logo_rect(&self) -> Rect {
        let text = Screen::text_width("grenOS", Font::Head);
        Rect::new(4, 3, text + self.panel + 8, self.panel - 6)
    }

    pub fn quick_rect(&self, index: usize) -> Rect {
        let side = self.panel - 10;
        let left = self.logo_rect().right() + 10 + index * (side + 4);
        Rect::new(left, 5, side, side)
    }

    fn space_rect(&self, index: usize) -> Rect {
        let side = self.panel - 12;
        let left = self.quick_rect(2).right() + 14 + index * (side + 4);
        Rect::new(left, 6, side, side)
    }

    /// The buttons of the open windows, and how wide each one is.
    fn task_rect(&self, index: usize) -> Rect {
        let left = self.space_rect(3).right() + 14;
        let width = Screen::text_width("Paramètres", Font::Body) + 40;
        let room = self.tray_left().saturating_sub(left);
        let width = width.min(room / 4).max(font::width(Font::Body) * 6);
        Rect::new(left + index * (width + 4), 5, width, self.panel - 10)
    }

    pub fn power_rect(&self) -> Rect {
        let side = self.panel - 12;
        Rect::new(self.width - side - 8, 6, side, side)
    }

    fn clock_text(&self) -> String {
        format!(
            "{} {} {} {:02}:{:02}:{:02}",
            self.now.day_name(),
            self.now.day,
            self.now.month_name(),
            self.now.hour,
            self.now.minute,
            self.now.second
        )
    }

    fn clock_rect(&self) -> Rect {
        let width = Screen::text_width(&self.clock_text(), Font::Body) + 12;
        Rect::new(self.power_rect().x.saturating_sub(width + 6), 0, width, self.panel)
    }

    /// Where the right-hand end of the panel starts.
    fn tray_left(&self) -> usize {
        let counters = Screen::text_width("00000", Font::Small) * 2 + 90;
        self.clock_rect().x.saturating_sub(counters)
    }

    fn menu_rect(&self) -> Rect {
        let row = self.menu_row();
        let width = Screen::text_width("Paramètres, écran, mise à jour ", Font::Small) + 90;
        let height = row * self.filtered().count().max(1) + self.search_h() + self.line_h() + 26;
        Rect::new(6, self.panel + 6, width.min(self.width - 12), height)
    }

    fn menu_row(&self) -> usize {
        font::height(Font::Body) + font::height(Font::Small) + 12
    }

    fn search_h(&self) -> usize {
        font::height(Font::Body) + 14
    }

    fn menu_item_rect(&self, index: usize) -> Rect {
        let menu = self.menu_rect();
        let top = menu.y + self.search_h() + 12 + index * self.menu_row();
        Rect::new(menu.x + 6, top, menu.w - 12, self.menu_row())
    }

    /// The menu entries the search box keeps.
    fn filtered(&self) -> impl Iterator<Item = (usize, &'static (&'static str, &'static str, Item))> + '_ {
        ITEMS.iter().enumerate().filter(|(_, (name, _, _))| matches(name, &self.filter))
    }

    fn power_menu_rect(&self) -> Rect {
        let width = Screen::text_width("Redémarrer", Font::Body) + 60;
        Rect::new(self.width - width - 8, self.panel + 6, width, 2 * self.line_h() + 20)
    }

    fn power_item_rect(&self, index: usize) -> Rect {
        let menu = self.power_menu_rect();
        Rect::new(menu.x + 6, menu.y + 10 + index * self.line_h(), menu.w - 12, self.line_h())
    }

    pub fn title_rect(&self, app: App) -> Rect {
        let area = self.windows[app.index()].area;
        Rect::new(area.x, area.y, area.w, self.title_h())
    }

    fn client_rect(&self, app: App) -> Rect {
        let area = self.windows[app.index()].area;
        let top = area.y + self.title_h();
        Rect::new(area.x + 1, top, area.w.saturating_sub(2), area.bottom().saturating_sub(top + 1))
    }

    /// The three buttons at the right of a title bar, right to left: close,
    /// maximise, minimise.
    fn button_rect(&self, app: App, index: usize) -> Rect {
        let title = self.title_rect(app);
        let side = font::height(Font::Body) + 4;
        let top = title.y + (title.h - side) / 2;
        Rect::new(title.right().saturating_sub((index + 1) * (side + 6)), top, side, side)
    }

    pub fn side_rect(&self, index: usize) -> Rect {
        let client = self.client_rect(App::Settings);
        let width = Screen::text_width("Souris et clavier", Font::Body) + 28;
        Rect::new(client.x, client.y + 8 + index * self.line_h(), width.min(client.w / 2), self.line_h())
    }

    fn settings_body(&self) -> Rect {
        let client = self.client_rect(App::Settings);
        let left = self.side_rect(0).w;
        Rect::new(client.x + left + 16, client.y + 12, client.w.saturating_sub(left + 28), client.h.saturating_sub(20))
    }

    pub fn update_button_rect(&self) -> Rect {
        let body = self.settings_body();
        let width = Screen::text_width("Vérifier les mises à jour", Font::Body) + 28;
        Rect::new(body.x, body.y + font::height(Font::Title) + 12 + 4 * self.line_h(), width, self.line_h() + 8)
    }

    // ---- Painting ---------------------------------------------------------

    fn paint(&mut self, screen: &mut Screen) {
        if self.off {
            self.paint_off(screen);
            return;
        }
        self.paint_wallpaper(screen);
        let front = self.front();
        for app in self.order {
            if self.showing(app) {
                self.paint_window(screen, app, Some(app) == front);
            }
        }
        self.paint_panel(screen);
        if self.menu {
            self.paint_menu(screen);
        }
        if self.power_menu {
            self.paint_power_menu(screen);
        }
    }

    fn paint_wallpaper(&self, screen: &mut Screen) {
        screen.gradient(Rect::new(0, 0, self.width, self.height), WALL_TOP, WALL_BOTTOM);
        // The wordmark, large and barely there, in the corner the windows
        // leave free.
        let scale = if self.width >= 1600 { 3 } else { 2 };
        let mark = "grenOS";
        let width = Screen::text_width(mark, Font::Logo) * scale;
        let x = self.width.saturating_sub(width + 56);
        let y = self.height * 3 / 4;
        screen.text_scaled(x, y, mark, WALL_MARK, Font::Logo, scale);
        let line = "un noyau écrit par des agents, vérifié par la CI";
        let below = y + font::height(Font::Logo) * scale + 12;
        let left = self.width.saturating_sub(Screen::text_width(line, Font::Body) + 56);
        screen.text(left, below, line, WALL_MARK, Font::Body);
    }

    fn paint_panel(&self, screen: &mut Screen) {
        screen.fill(Rect::new(0, 0, self.width, self.panel), PANEL);
        screen.fill(Rect::new(0, self.panel - 1, self.width, 1), LINE);

        // The menu button: the grenOS mark, and no flag of anybody else's.
        let logo = self.logo_rect();
        if self.menu {
            screen.round(logo, 6, ACCENT_SOFT);
        }
        let badge = Rect::new(logo.x + 6, logo.y + 3, logo.h - 6, logo.h - 6);
        screen.round(badge, 5, ACCENT);
        let letter = "g";
        screen.text(
            badge.x + (badge.w - Screen::text_width(letter, Font::Head)) / 2,
            badge.y + (badge.h - font::height(Font::Head)) / 2,
            letter,
            WHITE,
            Font::Head,
        );
        screen.text(badge.right() + 8, centre_y(logo, Font::Head), "grenOS", TEXT, Font::Head);

        for (index, app) in [App::Terminal, App::Notes, App::Settings].into_iter().enumerate() {
            let area = self.quick_rect(index);
            screen.round(area, 5, if self.showing(app) { SURFACE } else { PANEL });
            self.paint_icon(screen, area.inset(3), app);
        }

        for index in 0..4 {
            let area = self.space_rect(index);
            let here = index as u8 + 1 == self.space;
            screen.round(area, 4, if here { ACCENT } else { SURFACE_ALT });
            let text = format!("{}", index + 1);
            screen.text(
                area.x + (area.w - Screen::text_width(&text, Font::Small)) / 2,
                centre_y(area, Font::Small),
                &text,
                if here { WHITE } else { TEXT_FAINT },
                Font::Small,
            );
        }

        let front = self.front();
        for (index, app) in self.visible().enumerate() {
            let area = self.task_rect(index);
            if area.right() > self.tray_left() {
                break;
            }
            let active = Some(app) == front;
            screen.round(area, 5, if active { SURFACE } else { PANEL });
            if active {
                screen.fill(Rect::new(area.x + 6, area.bottom() - 2, area.w - 12, 2), ACCENT);
            }
            let room = (area.w - 16) / font::width(Font::Body);
            let label = app.short();
            let end = label.char_indices().nth(room).map_or(label.len(), |(at, _)| at);
            screen.text(area.x + 8, centre_y(area, Font::Body), &label[..end], if active { TEXT } else { TEXT_DIM }, Font::Body);
        }

        self.paint_counters(screen);
        screen.text(self.clock_rect().x + 6, centre_y(self.clock_rect(), Font::Body), &self.clock_text(), TEXT, Font::Body);

        let power = self.power_rect();
        if self.power_menu {
            screen.round(power, 5, ACCENT_SOFT);
        }
        power_glyph(screen, power.inset(3), TEXT_DIM, if self.power_menu { ACCENT_SOFT } else { PANEL });
    }

    /// The two little devices at the right of the panel, with what they have
    /// sent. Green when bytes arrive, red when none ever have: that is the
    /// first thing to look at when the pointer will not move.
    fn paint_counters(&self, screen: &mut Screen) {
        let top = (self.panel - 12) / 2;
        let mouse_ok = self.input.mouse_bytes > 0;
        let key_ok = self.input.key_bytes > 0;

        let x = self.tray_left();
        let body = Rect::new(x, top, 9, 13);
        let colour = if mouse_ok { GREEN } else { RED };
        screen.round(body, 4, colour);
        screen.fill(Rect::new(body.x + 4, body.y + 2, 1, 4), PANEL);
        let count = format!("{}", self.input.mouse_bytes.min(99_999));
        let after = body.right() + 5;
        screen.text(after, centre_y(Rect::new(0, 0, 0, self.panel), Font::Small), &count, colour, Font::Small);

        let x = after + Screen::text_width(&count, Font::Small) + 14;
        let board = Rect::new(x, top + 2, 15, 9);
        let colour = if key_ok { GREEN } else { TEXT_FAINT };
        screen.round(board, 2, colour);
        for dot in 0..3 {
            screen.fill(Rect::new(board.x + 3 + dot * 3, board.y + 3, 2, 2), PANEL);
        }
        let count = format!("{}", self.input.key_bytes.min(99_999));
        screen.text(board.right() + 5, centre_y(Rect::new(0, 0, 0, self.panel), Font::Small), &count, colour, Font::Small);
    }

    fn paint_icon(&self, screen: &mut Screen, area: Rect, app: App) {
        screen.round(area, 4, app.tint());
        let letter = app.letter();
        screen.text(
            area.x + (area.w.saturating_sub(Screen::text_width(letter, Font::Head))) / 2,
            centre_y(area, Font::Head),
            letter,
            if app == App::Terminal { GREEN } else { TEXT },
            Font::Head,
        );
    }

    fn paint_window(&self, screen: &mut Screen, app: App, active: bool) {
        let area = self.windows[app.index()].area;
        screen.shadow(area, if active { 7 } else { 4 });
        screen.round(area, 8, SURFACE);

        let title = self.title_rect(app);
        let previous = screen.set_clip(title.intersect(screen.clip()));
        screen.round(Rect::new(title.x, title.y, title.w, title.h + 8), 8, if active { SURFACE_ALT } else { PANEL });
        screen.set_clip(previous);
        screen.fill(Rect::new(title.x, title.bottom() - 1, title.w, 1), LINE);

        self.paint_icon(screen, Rect::new(title.x + 10, title.y + (title.h - 18) / 2, 18, 18), app);
        screen.text(
            title.x + 36,
            centre_y(title, Font::Head),
            app.title(),
            if active { TEXT } else { TEXT_DIM },
            Font::Head,
        );

        for index in 0..3 {
            let button = self.button_rect(app, index);
            let (cx, cy) = (button.x + button.w / 2, button.y + button.h / 2);
            match index {
                0 => {
                    // Close: a cross, in red so it is never hit by mistake.
                    for step in 0..7 {
                        screen.fill(Rect::new(cx - 3 + step, cy - 3 + step, 1, 1), RED);
                        screen.fill(Rect::new(cx + 3 - step, cy - 3 + step, 1, 1), RED);
                    }
                }
                1 => screen.border(Rect::new(cx - 4, cy - 4, 9, 9), TEXT_DIM),
                _ => screen.fill(Rect::new(cx - 4, cy + 3, 9, 1), TEXT_DIM),
            }
        }

        // Nothing a window draws leaves the window: a long line of text, a
        // list too tall for it, stop at its edge instead of on the wallpaper.
        let client = self.client_rect(app);
        let previous = screen.set_clip(client.intersect(screen.clip()));
        match app {
            App::Terminal => self.paint_terminal(screen, client),
            App::Notes => self.paint_notes(screen, client),
            App::Settings => self.paint_settings(screen, client),
            App::About => self.paint_about(screen, client),
        }
        screen.set_clip(previous);
    }

    fn paint_terminal(&self, screen: &mut Screen, client: Rect) {
        screen.fill(client, TERMINAL_BG);
        let step = font::height(Font::Body) + 2;
        let rows = (client.h.saturating_sub(16)) / step;
        let lines = self.shell.lines();
        let start = lines.len().saturating_sub(rows.saturating_sub(1));
        let mut y = client.y + 8;
        for line in &lines[start..] {
            match line {
                Line::Command(text) => {
                    let x = self.paint_prompt(screen, client.x + 10, y);
                    screen.text(x, y, text, TEXT, Font::Body);
                }
                Line::Output(text) => {
                    screen.text(client.x + 10, y, text, TEXT_DIM, Font::Body);
                }
            }
            y += step;
        }
        if y + step <= client.bottom() {
            let x = self.paint_prompt(screen, client.x + 10, y);
            let x = screen.text(x, y, self.shell.input(), TEXT, Font::Body);
            screen.fill(Rect::new(x + 1, y + 2, font::width(Font::Body) - 1, font::height(Font::Body) - 4), ACCENT);
        }
    }

    fn paint_prompt(&self, screen: &mut Screen, x: usize, y: usize) -> usize {
        let x = screen.text(x, y, "root", RED, Font::Body);
        let x = screen.text(x, y, "@grenos", GREEN, Font::Body);
        let x = screen.text(x, y, ":", TEXT_FAINT, Font::Body);
        let x = screen.text(x, y, "~", ACCENT, Font::Body);
        screen.text(x, y, "# ", TEXT_FAINT, Font::Body)
    }

    /// The notepad, wrapped between words, with the caret at the end.
    fn paint_notes(&self, screen: &mut Screen, client: Rect) {
        screen.fill(client, NOTES_BG);
        let glyph = font::width(Font::Body);
        let step = font::height(Font::Body) + 2;
        let columns = (client.w.saturating_sub(24) / glyph).max(1);
        let text = &self.notes[..];
        let (mut column, mut row) = (0, 0);
        let mut buffer = [0u8; 4];
        for (i, &c) in text.iter().enumerate() {
            if c == '\n' {
                column = 0;
                row += 1;
                continue;
            }
            let starts_word = c != ' ' && (i == 0 || text[i - 1] == ' ' || text[i - 1] == '\n');
            if starts_word {
                let word = text[i..].iter().take_while(|&&next| next != ' ' && next != '\n').count();
                if column > 0 && column + word > columns && word <= columns {
                    column = 0;
                    row += 1;
                }
            }
            if column == columns {
                column = 0;
                row += 1;
            }
            let y = client.y + 10 + row * step;
            if y + step > client.bottom() {
                return;
            }
            screen.text(client.x + 12 + column * glyph, y, c.encode_utf8(&mut buffer), TEXT, Font::Body);
            column += 1;
        }
        if column == columns {
            column = 0;
            row += 1;
        }
        let y = client.y + 10 + row * step;
        if y + step <= client.bottom() {
            screen.fill(Rect::new(client.x + 12 + column * glyph, y + 2, glyph - 1, font::height(Font::Body) - 4), ACCENT);
        }
    }

    fn paint_about(&self, screen: &mut Screen, client: Rect) {
        let badge = Rect::new(client.x + 20, client.y + 18, 46, 46);
        screen.round(badge, 10, ACCENT);
        screen.text(
            badge.x + (badge.w - Screen::text_width("g", Font::Title)) / 2,
            badge.y + (badge.h - font::height(Font::Title)) / 2,
            "g",
            WHITE,
            Font::Title,
        );
        let left = badge.right() + 16;
        screen.text(left, client.y + 18, "grenOS", TEXT, Font::Title);
        let version = format!("version {} · build {}", self.machine.version, self.machine.build);
        screen.text(left, client.y + 20 + font::height(Font::Title), &version, TEXT_DIM, Font::Small);

        let mut y = badge.bottom() + 16;
        for line in ABOUT {
            screen.text(client.x + 20, y, line, TEXT_DIM, Font::Body);
            y += self.line_h();
        }
        y += 6;
        screen.fill(Rect::new(client.x + 20, y, client.w.saturating_sub(40), 1), LINE);
        y += 12;
        screen.text(client.x + 20, y, &format!("Compilé le {}", self.machine.built_at), TEXT_FAINT, Font::Small);
    }

    fn paint_settings(&self, screen: &mut Screen, client: Rect) {
        let side = Rect::new(client.x, client.y, self.side_rect(0).w, client.h);
        screen.fill(side, SURFACE_ALT);
        screen.fill(Rect::new(side.right(), client.y, 1, client.h), LINE);
        for (index, name) in SECTIONS.iter().enumerate() {
            let row = self.side_rect(index);
            let here = index == self.section;
            if here {
                screen.round(Rect::new(row.x + 4, row.y, row.w - 8, row.h), 4, ACCENT_SOFT);
                screen.fill(Rect::new(row.x + 4, row.y + 3, 2, row.h - 6), ACCENT);
            }
            screen.text(row.x + 14, centre_y(row, Font::Body), name, if here { TEXT } else { TEXT_DIM }, Font::Body);
        }

        let body = self.settings_body();
        screen.text(body.x, body.y, SECTIONS[self.section], TEXT, Font::Title);
        let top = body.y + font::height(Font::Title) + 12;
        match self.section {
            0 => {
                let mut lines = self.machine.lines();
                lines.push(format!("Allumé depuis : {}", uptime(self.input.uptime)));
                self.paint_rows(screen, body, top, &lines);
            }
            1 => self.paint_input_section(screen, body, top),
            2 => self.paint_rows(screen, body, top, &self.screen_lines()),
            3 => self.paint_devices(screen, body, top),
            _ => self.paint_update(screen, body, top),
        }
    }

    fn screen_lines(&self) -> Vec<String> {
        alloc::vec![
            format!("Mode : {}", self.machine.screen),
            format!("Bureau : panneau de {} pixels, {} fenêtres", self.panel, APPS.len()),
            "Dessin : tampon en mémoire, puis une seule copie vers la carte".to_string(),
            "Police : Noto Sans Mono, lissée sur 256 niveaux".to_string(),
        ]
    }

    fn paint_rows(&self, screen: &mut Screen, body: Rect, top: usize, lines: &[String]) {
        // One column for the labels, as wide as the longest of them: a fixed
        // width had the value written over the end of its own label.
        let column = lines
            .iter()
            .filter_map(|line| line.split_once(" : "))
            .map(|(label, _)| Screen::text_width(label, Font::Body) + 20)
            .max()
            .unwrap_or(0)
            .min(body.w / 2);
        let mut y = top;
        for line in lines {
            if y + self.line_h() > body.bottom() {
                return;
            }
            match line.split_once(" : ") {
                Some((label, value)) => {
                    screen.text(body.x, y, label, TEXT_FAINT, Font::Body);
                    screen.text(body.x + column, y, value, TEXT, Font::Body);
                }
                None => {
                    screen.text(body.x, y, line, TEXT_DIM, Font::Body);
                }
            }
            y += self.line_h();
        }
    }

    fn paint_input_section(&self, screen: &mut Screen, body: Rect, top: usize) {
        let input = self.input;
        let alive = input.mouse_bytes > 0;
        let state = if !self.machine.mouse {
            "aucune réponse à l'allumage"
        } else if alive {
            "détectée, et elle parle"
        } else {
            "détectée, mais silencieuse"
        };
        let lines = alloc::vec![
            format!("Souris PS/2 : {}", state),
            format!("Octets reçus : {} souris, {} clavier", input.mouse_bytes, input.key_bytes),
            format!("Interruptions : IRQ12 {}, IRQ1 {}", input.mouse_irq, input.key_irq),
            format!("Ramassés par le minuteur : {}", input.swept),
            format!("Paquets souris : {}, touches : {}", input.packets, input.keys),
            format!("Événements perdus : {}", input.lost),
            format!("Disposition du clavier : français (AZERTY)"),
        ];
        self.paint_rows(screen, body, top, &lines);
        let mut y = top + lines.len() * self.line_h() + 10;
        let hints = [
            "Dans VirtualBox, la souris est capturée : la touche Ctrl droite la rend.",
            "Au clavier : F1 ouvre le menu, Tab passe d'une fenêtre à l'autre, Échap ferme.",
        ];
        for hint in hints {
            if y + self.line_h() > body.bottom() {
                return;
            }
            screen.text(body.x, y, hint, if alive { TEXT_FAINT } else { AMBER }, Font::Small);
            y += self.line_h();
        }
    }

    fn paint_devices(&self, screen: &mut Screen, body: Rect, top: usize) {
        let mut y = top;
        if self.machine.devices.is_empty() {
            screen.text(body.x, y, "Aucun périphérique PCI trouvé.", TEXT_DIM, Font::Body);
            return;
        }
        for line in &self.machine.devices {
            if y + font::height(Font::Small) + 4 > body.bottom() {
                screen.text(body.x, y, "…", TEXT_FAINT, Font::Small);
                return;
            }
            screen.text(body.x, y, line, TEXT_DIM, Font::Small);
            y += font::height(Font::Small) + 4;
        }
    }

    fn paint_update(&self, screen: &mut Screen, body: Rect, top: usize) {
        let lines = alloc::vec![
            format!("Version installée : grenOS {}", self.machine.version),
            format!("Build : {}", self.machine.build),
            format!("Compilée le : {}", self.machine.built_at),
            format!("Canal : principal (main), publié après une CI verte"),
        ];
        self.paint_rows(screen, body, top, &lines);

        let button = self.update_button_rect();
        screen.round(button, 6, ACCENT);
        screen.text(
            button.x + 14,
            centre_y(button, Font::Body),
            "Vérifier les mises à jour",
            WHITE,
            Font::Body,
        );

        let mut y = button.bottom() + 10;
        let note = match self.update_note {
            0 => "grenOS ne sait pas encore aller sur le réseau : le pilote arrive.",
            _ => "Pas de réseau pour l'instant. La dernière image est sur grenos-dev.vercel.app/download",
        };
        screen.text(body.x, y, note, if self.update_note == 0 { TEXT_FAINT } else { AMBER }, Font::Small);
        y += self.line_h() + 8;

        for (title, list) in [("Nouveautés de cette version", &CHANGES[..]), ("Prochaines étapes", &NEXT[..])] {
            if y + self.line_h() > body.bottom() {
                return;
            }
            screen.text(body.x, y, title, TEXT, Font::Head);
            y += self.line_h() + 2;
            for line in list {
                if y + font::height(Font::Small) + 4 > body.bottom() {
                    return;
                }
                screen.fill(Rect::new(body.x + 3, y + font::height(Font::Small) / 2, 3, 3), ACCENT);
                screen.text(body.x + 14, y, line, TEXT_DIM, Font::Small);
                y += font::height(Font::Small) + 5;
            }
            y += 8;
        }
    }

    fn paint_menu(&self, screen: &mut Screen) {
        let menu = self.menu_rect();
        screen.shadow(menu, 8);
        screen.round(menu, 10, SURFACE);
        screen.fill(Rect::new(menu.x, menu.y + self.search_h() + 4, menu.w, 1), LINE);

        let search = Rect::new(menu.x + 8, menu.y + 7, menu.w - 16, self.search_h() - 10);
        screen.round(search, 5, SUNKEN);
        let (text, colour) = if self.filter.is_empty() {
            ("Rechercher une application", TEXT_FAINT)
        } else {
            (self.filter.as_str(), TEXT)
        };
        screen.text(search.x + 10, centre_y(search, Font::Body), text, colour, Font::Body);

        for (row, (index, (name, about, item))) in self.filtered().enumerate() {
            let area = self.menu_item_rect(row);
            if area.bottom() > menu.bottom() {
                break;
            }
            if index == self.choice {
                screen.round(area, 6, ACCENT_SOFT);
            }
            let icon = Rect::new(area.x + 8, area.y + (area.h - 26) / 2, 26, 26);
            match item {
                Item::Open(app) => self.paint_icon(screen, icon, *app),
                Item::Reboot => {
                    screen.round(icon, 5, Rgb(0x33, 0x2C, 0x1E));
                    power_glyph(screen, icon.inset(5), AMBER, Rgb(0x33, 0x2C, 0x1E));
                }
                Item::Off => {
                    screen.round(icon, 5, Rgb(0x36, 0x22, 0x22));
                    power_glyph(screen, icon.inset(5), RED, Rgb(0x36, 0x22, 0x22));
                }
            }
            screen.text(icon.right() + 12, area.y + 6, name, TEXT, Font::Head);
            screen.text(icon.right() + 12, area.y + 8 + font::height(Font::Head), about, TEXT_FAINT, Font::Small);
        }

        let footer = Rect::new(menu.x + 10, menu.bottom() - self.line_h() - 8, menu.w - 20, self.line_h());
        screen.text(footer.x, footer.y, "root@grenos", TEXT_FAINT, Font::Small);
        let hint = "F1 ouvre ce menu";
        screen.text(
            footer.right().saturating_sub(Screen::text_width(hint, Font::Small)),
            footer.y,
            hint,
            TEXT_FAINT,
            Font::Small,
        );
    }

    fn paint_power_menu(&self, screen: &mut Screen) {
        let menu = self.power_menu_rect();
        screen.shadow(menu, 6);
        screen.round(menu, 8, SURFACE);
        for (index, (name, colour)) in [("Redémarrer", AMBER), ("Éteindre", RED)].into_iter().enumerate() {
            let row = self.power_item_rect(index);
            screen.text(row.x + 10, centre_y(row, Font::Body), name, colour, Font::Body);
        }
    }

    /// The screen after Éteindre, while the machine stops.
    fn paint_off(&self, screen: &mut Screen) {
        screen.fill(Rect::new(0, 0, self.width, self.height), BLACK);
        let lines: [(&str, Font, Rgb); 3] = [
            ("grenOS", Font::Title, TEXT),
            ("Le système est arrêté.", Font::Body, TEXT_DIM),
            ("Vous pouvez fermer la fenêtre ou éteindre la machine.", Font::Small, TEXT_FAINT),
        ];
        let mut y = self.height / 2 - font::height(Font::Title);
        for (text, style, colour) in lines {
            let x = self.width.saturating_sub(Screen::text_width(text, style)) / 2;
            screen.text(x, y, text, colour, style);
            y += font::height(style) + 12;
        }
    }

    fn draw_pointer(&self, screen: &mut Screen) {
        let (x, y) = self.pointer;
        for (dy, line) in POINTER.iter().enumerate() {
            for (dx, dot) in line.bytes().enumerate() {
                let colour = match dot {
                    b'X' => BLACK,
                    b'.' => WHITE,
                    _ => continue,
                };
                screen.front_pixel(x + dx, y + dy, colour);
            }
        }
    }

    // ---- The mouse --------------------------------------------------------

    pub fn mouse(&mut self, packet: Packet) -> Option<Action> {
        if self.off {
            return None;
        }
        if packet.dx != 0 || packet.dy != 0 {
            let x = (self.pointer.0 as i64 + i64::from(packet.dx)).clamp(0, self.width as i64 - 1);
            let y = (self.pointer.1 as i64 + i64::from(packet.dy)).clamp(0, self.height as i64 - 1);
            self.pointer = (x as usize, y as usize);
            if let Some((app, offset_x, offset_y)) = self.grab {
                let area = self.windows[app.index()].area;
                let left = self.pointer.0.saturating_sub(offset_x).min(self.width.saturating_sub(60));
                let top = self.pointer.1.saturating_sub(offset_y).clamp(self.panel, self.height.saturating_sub(40));
                let moved = Rect::new(left, top, area.w, area.h);
                self.windows[app.index()].area = moved;
                self.damage(area.grow(8));
                self.damage(moved.grow(8));
            }
        }
        let pressed = packet.left && !self.left_down;
        let released = !packet.left && self.left_down;
        self.left_down = packet.left;
        if released {
            self.grab = None;
        }
        if pressed {
            return self.click();
        }
        None
    }

    fn click(&mut self) -> Option<Action> {
        let (x, y) = self.pointer;
        let hit = |area: Rect| area.contains(x, y);

        if self.power_menu {
            self.power_menu = false;
            self.damage_all();
            if hit(self.power_item_rect(0)) {
                return Some(Action::Reboot);
            }
            if hit(self.power_item_rect(1)) {
                return self.shut_down();
            }
            return None;
        }
        if self.menu {
            if hit(self.menu_rect()) {
                let chosen = self.filtered().enumerate().find(|(row, _)| hit(self.menu_item_rect(*row)));
                if let Some((_, (_, (_, _, item)))) = chosen {
                    let item = *item;
                    return self.launch(item);
                }
                return None;
            }
            self.menu = false;
            self.damage_all();
        }

        if y < self.panel {
            return self.click_panel(x, y);
        }

        let order = self.order;
        for app in order.into_iter().rev() {
            if !self.showing(app) {
                continue;
            }
            let area = self.windows[app.index()].area;
            if !hit(area) {
                continue;
            }
            if self.front() != Some(app) {
                self.raise(app);
                self.damage_all();
            }
            if hit(self.button_rect(app, 0)) {
                self.close(app);
                return None;
            }
            if hit(self.button_rect(app, 1)) {
                self.toggle_full(app);
                return None;
            }
            if hit(self.button_rect(app, 2)) {
                self.close(app);
                return None;
            }
            if hit(self.title_rect(app)) {
                self.grab = Some((app, x - area.x, y - area.y));
                return None;
            }
            if app == App::Settings {
                self.click_settings(x, y);
            }
            return None;
        }
        None
    }

    fn click_panel(&mut self, x: usize, y: usize) -> Option<Action> {
        let hit = |area: Rect| area.contains(x, y);
        if hit(self.logo_rect()) {
            self.menu = !self.menu;
            self.filter.clear();
            self.choice = 0;
            self.damage_all();
            return None;
        }
        if hit(self.power_rect()) {
            self.power_menu = true;
            self.damage_all();
            return None;
        }
        for (index, app) in [App::Terminal, App::Notes, App::Settings].into_iter().enumerate() {
            if hit(self.quick_rect(index)) {
                self.open(app);
                return None;
            }
        }
        for index in 0..4 {
            if hit(self.space_rect(index)) {
                self.space = index as u8 + 1;
                self.damage_all();
                return None;
            }
        }
        let buttons: Vec<App> = self.visible().collect();
        for (index, app) in buttons.into_iter().enumerate() {
            if hit(self.task_rect(index)) {
                if self.front() == Some(app) {
                    self.close(app);
                } else {
                    self.raise(app);
                    self.damage_all();
                }
                return None;
            }
        }
        None
    }

    fn click_settings(&mut self, x: usize, y: usize) {
        for index in 0..SECTIONS.len() {
            if self.side_rect(index).contains(x, y) {
                self.section = index;
                self.update_note = 0;
                self.damage(self.windows[App::Settings.index()].area);
                return;
            }
        }
        if self.section == SECTIONS.len() - 1 && self.update_button_rect().contains(x, y) {
            self.update_note = 1;
            self.damage(self.windows[App::Settings.index()].area);
        }
    }

    fn toggle_full(&mut self, app: App) {
        let window = &mut self.windows[app.index()];
        if window.full {
            window.area = window.restore;
            window.full = false;
        } else {
            window.restore = window.area;
            window.area = Rect::new(0, self.panel, self.width, self.height - self.panel);
            window.full = true;
        }
        self.damage_all();
    }

    fn launch(&mut self, item: Item) -> Option<Action> {
        self.menu = false;
        self.filter.clear();
        self.choice = 0;
        match item {
            Item::Open(app) => {
                self.open(app);
                None
            }
            Item::Reboot => Some(Action::Reboot),
            Item::Off => self.shut_down(),
        }
    }

    fn shut_down(&mut self) -> Option<Action> {
        self.off = true;
        self.damage_all();
        Some(Action::PowerOff)
    }

    // ---- The keyboard -----------------------------------------------------

    pub fn key(&mut self, key: Key) -> Option<Action> {
        if self.off {
            return None;
        }
        if key == Key::Menu {
            self.menu = !self.menu;
            self.filter.clear();
            self.choice = 0;
            self.damage_all();
            return None;
        }
        if self.power_menu {
            if key == Key::Escape {
                self.power_menu = false;
                self.damage_all();
            }
            return None;
        }
        if self.menu {
            return self.key_menu(key);
        }
        if key == Key::Tab {
            if let Some(front) = self.front() {
                let next = self.visible().find(|&app| app != front).unwrap_or(front);
                self.raise(next);
                self.damage_all();
            }
            return None;
        }
        match self.front() {
            Some(App::Terminal) => {
                let context = Context { machine: &self.machine, now: self.now, input: self.input };
                let action = self.shell.key(key, &context);
                self.damage(self.windows[App::Terminal.index()].area);
                if action == Some(Action::PowerOff) {
                    return self.shut_down();
                }
                action
            }
            Some(App::Notes) => {
                match key {
                    Key::Char(c) if self.notes.len() < NOTES_CAP => self.notes.push(c),
                    Key::Enter if self.notes.len() < NOTES_CAP => self.notes.push('\n'),
                    Key::Backspace => {
                        self.notes.pop();
                    }
                    _ => return None,
                }
                self.damage(self.windows[App::Notes.index()].area);
                None
            }
            Some(App::Settings) => {
                match key {
                    Key::Up => self.section = self.section.saturating_sub(1),
                    Key::Down => self.section = (self.section + 1).min(SECTIONS.len() - 1),
                    Key::Enter if self.section == SECTIONS.len() - 1 => self.update_note = 1,
                    _ => return None,
                }
                self.damage(self.windows[App::Settings.index()].area);
                None
            }
            _ => None,
        }
    }

    fn key_menu(&mut self, key: Key) -> Option<Action> {
        let rows: Vec<usize> = self.filtered().map(|(index, _)| index).collect();
        match key {
            Key::Escape => {
                self.menu = false;
                self.filter.clear();
            }
            Key::Up => {
                let at = rows.iter().position(|&index| index == self.choice).unwrap_or(0);
                self.choice = rows.get(at.saturating_sub(1)).copied().unwrap_or(self.choice);
            }
            Key::Down => {
                let at = rows.iter().position(|&index| index == self.choice).unwrap_or(0);
                self.choice = rows.get(at + 1).copied().unwrap_or(self.choice);
            }
            Key::Enter => {
                let chosen = rows.first().copied().unwrap_or(0);
                let index = if rows.contains(&self.choice) { self.choice } else { chosen };
                let item = ITEMS[index].2;
                return self.launch(item);
            }
            Key::Backspace => {
                self.filter.pop();
                let first = self.filtered().map(|(index, _)| index).next().unwrap_or(0);
                self.choice = first;
            }
            Key::Char(c) => {
                self.filter.push(c);
                let first = self.filtered().map(|(index, _)| index).next().unwrap_or(0);
                self.choice = first;
            }
            _ => return None,
        }
        self.damage_all();
        None
    }
}

/// Where a window sits when it opens.
fn default_rect(app: App, width: usize, height: usize, panel: usize) -> Rect {
    let top = panel + height / 24;
    match app {
        App::Terminal => Rect::new(width / 40, top + height / 5, width * 5 / 9, height * 5 / 9),
        App::Notes => Rect::new(width / 4, top + height / 8, width * 2 / 5, height / 2),
        App::Settings => Rect::new(width / 6, top, width * 2 / 3, height * 2 / 3),
        App::About => Rect::new(width / 2, top, width * 4 / 9, height * 4 / 11),
    }
}

/// Seconds since boot, in words.
fn uptime(seconds: u32) -> String {
    match seconds {
        0..=59 => format!("{seconds} s"),
        60..=3599 => format!("{} min {:02} s", seconds / 60, seconds % 60),
        _ => format!("{} h {:02} min", seconds / 3600, seconds % 3600 / 60),
    }
}

/// The y that centres a line of `style` in `area`.
fn centre_y(area: Rect, style: Font) -> usize {
    area.y + area.h.saturating_sub(font::height(style)) / 2
}

/// Does `name` hold `filter`, ignoring case and accents on the ASCII letters?
fn matches(name: &str, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let fold = |text: &str| text.chars().map(|c| c.to_ascii_lowercase()).collect::<String>();
    fold(name).contains(&fold(filter))
}

/// The power symbol: a ring with a gap at the top, and a bar through it.
fn power_glyph(screen: &mut Screen, area: Rect, colour: Rgb, behind: Rgb) {
    let side = area.w.min(area.h);
    let area = Rect::new(area.x, area.y, side, side);
    screen.round(area, side / 2, colour);
    screen.round(area.inset(2), side / 2, behind);
    let centre = area.x + side / 2;
    screen.fill(Rect::new(centre - 2, area.y, 4, side / 3), behind);
    screen.fill(Rect::new(centre - 1, area.y + 1, 2, side / 2), colour);
}
