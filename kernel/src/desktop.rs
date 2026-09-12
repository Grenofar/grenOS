//! The grenOS desktop: a dark panel, an application menu, windows with a
//! shadow, and six applications. It answers the mouse and the keyboard, and
//! everything it knows about the machine comes from the kernel in a
//! [`Machine`].
//!
//! Nothing here touches the hardware: the desktop changes its own state, says
//! which part of the screen that spoiled, and `frame` paints it. Movement is
//! measured in milliseconds, never in frames, so an animation lasts as long
//! whether the machine draws it 24 times a second or 360.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::anim::{self, FULL};
use crate::fb::{Rect, Rgb, Screen};
use crate::font::{self, Font};
use crate::fs::{self, Fs, Kind};
use crate::icons::{self, Icon};
use crate::keyboard::Key;
use crate::mouse::Packet;
use crate::shell::{Context, Line, Shell};
use crate::time::DateTime;
use crate::web;

// ---- Colours --------------------------------------------------------------

const WALL_TOP: Rgb = Rgb(0x10, 0x18, 0x2A);
const WALL_BOTTOM: Rgb = Rgb(0x05, 0x07, 0x0D);
const WALL_MARK: Rgb = Rgb(0x1B, 0x27, 0x40);
const PANEL: Rgb = Rgb(0x13, 0x17, 0x21);
const HOVER: Rgb = Rgb(0x20, 0x26, 0x33);
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
const PAPER: Rgb = Rgb(0x12, 0x16, 0x1F);
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
    /// The boot log, which the terminal and the file system carry.
    pub log: Vec<String>,
    /// The mouse answered when the kernel set it up.
    pub mouse: bool,
    /// ACPI gave the registers that turn the machine off.
    pub can_power_off: bool,
    /// What the network card driver has to say, when there is one.
    pub network: String,
}

impl Machine {
    /// The summary Paramètres and `grenfetch` show.
    pub fn lines(&self) -> Vec<String> {
        alloc::vec![
            format!("Processeur : {}", self.cpu),
            format!("Mode : 64 bits (long mode), x86_64"),
            format!("Mémoire : {} Mo, dont {} Mo libres", self.memory.0, self.memory.1),
            format!("Tas du noyau : {} Ko sur {} Ko", self.heap.0 / 1024, self.heap.1 / 1024),
            format!("Écran : {}", self.screen),
            format!("Pagination : {}", self.paging),
            format!("Micrologiciel : {}", self.acpi),
            format!("Réseau : {}", self.network),
            format!("Périphériques PCI : {}", self.devices.len()),
            format!(
                "Extinction : {}",
                if self.can_power_off { "par ACPI, depuis le menu" } else { "pas de registre ACPI, arrêt seulement" }
            ),
        ]
    }
}

/// How much the input devices have said since boot, for Paramètres.
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
    /// Handled inside the desktop; never reaches the kernel.
    Lock,
}

// ---- Applications ---------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum App {
    Welcome,
    Terminal,
    Files,
    Browser,
    Notes,
    Settings,
    About,
    Security,
}

const APPS: [App; 8] =
    [App::Welcome, App::Terminal, App::Files, App::Browser, App::Notes, App::Settings, App::About, App::Security];
/// What the panel offers at a click, in order.
const QUICK: [App; 5] = [App::Terminal, App::Files, App::Browser, App::Notes, App::Settings];

impl App {
    fn index(self) -> usize {
        self as usize
    }

    fn title(self) -> &'static str {
        match self {
            App::Welcome => "Bienvenue dans grenOS",
            App::Terminal => "Terminal",
            App::Files => "Fichiers",
            App::Browser => "Navigateur",
            App::Notes => "Bloc-notes",
            App::Settings => "Paramètres",
            App::About => "À propos de grenOS",
            App::Security => "Sécurité",
        }
    }

    fn short(self) -> &'static str {
        match self {
            App::Welcome => "Bienvenue",
            App::About => "À propos",
            other => other.title(),
        }
    }

    fn icon(self) -> Icon {
        match self {
            App::Welcome => Icon::Home,
            App::Terminal => Icon::Terminal,
            App::Files => Icon::Files,
            App::Browser => Icon::Browser,
            App::Notes => Icon::Notes,
            App::Settings => Icon::Settings,
            App::About => Icon::About,
            App::Security => Icon::Lock,
        }
    }

    fn tint(self) -> Rgb {
        match self {
            App::Welcome => GREEN,
            App::Terminal => GREEN,
            App::Files => AMBER,
            App::Browser => Rgb(0x6C, 0xA8, 0xF0),
            App::Notes => Rgb(0xD8, 0xDE, 0xEA),
            App::Settings => Rgb(0x9C, 0xA7, 0xB9),
            App::About => Rgb(0x8A, 0xB4, 0xF8),
            App::Security => GREEN,
        }
    }
}

/// A window on the move: where it comes from, when it started, and why.
#[derive(Clone, Copy)]
struct Motion {
    from: Rect,
    start: u64,
    length: u64,
    closing: bool,
}

#[derive(Clone, Copy)]
struct Window {
    open: bool,
    /// Open but not drawn: reduced to the panel.
    hidden: bool,
    area: Rect,
    /// Where it goes back to when it stops filling the screen.
    restore: Rect,
    full: bool,
    motion: Option<Motion>,
}

/// A line of the menu.
#[derive(Clone, Copy)]
enum Item {
    Open(App),
    Lock,
    Reboot,
    Off,
}

const ITEMS: [(&str, &str, Item, Icon); 10] = [
    ("Terminal", "La ligne de commande de grenOS", Item::Open(App::Terminal), Icon::Terminal),
    ("Fichiers", "Parcourir les fichiers de la machine", Item::Open(App::Files), Icon::Files),
    ("Navigateur", "Les pages du système, et le web à venir", Item::Open(App::Browser), Icon::Browser),
    ("Bloc-notes", "Écrire, et enregistrer", Item::Open(App::Notes), Icon::Notes),
    ("Paramètres", "Système, souris, écran, compte, mise à jour", Item::Open(App::Settings), Icon::Settings),
    ("Sécurité", "Protection, intégrité du noyau, analyse", Item::Open(App::Security), Icon::Lock),
    ("À propos", "La version, et qui a écrit tout ça", Item::Open(App::About), Icon::About),
    ("Verrouiller", "Demander le mot de passe", Item::Lock, Icon::Lock),
    ("Redémarrer", "Relancer la machine", Item::Reboot, Icon::Restart),
    ("Éteindre", "Arrêter la machine", Item::Off, Icon::Power),
];

const SECTIONS: [(&str, Icon); 7] = [
    ("Système", Icon::About),
    ("Souris et clavier", Icon::Search),
    ("Écran", Icon::Browser),
    ("Matériel", Icon::Settings),
    ("Réseau", Icon::Browser),
    ("Compte", Icon::Lock),
    ("Mise à jour", Icon::Save),
];

/// What the current version brought, shown in Mise à jour.
const CHANGES: [&str; 6] = [
    "Le réseau : carte Intel 8254x, ARP, IPv4, ICMP, UDP, DHCP, DNS, TCP",
    "Le navigateur ouvre les vraies pages en http",
    "Paramètres, Réseau : adresse, passerelle, trames envoyées et reçues, ping",
    "Explorateur de fichiers, navigateur, écran de connexion, animations",
    "Réduire ne ferme plus la fenêtre : elle reste dans le panneau",
    "Un système de fichiers en mémoire, où le bloc-notes enregistre",
];

const NEXT: [&str; 4] = [
    "TLS, sans quoi la plupart des sites refusent de répondre en http",
    "Disque SATA (AHCI) : garder les fichiers d'une fois sur l'autre",
    "Protection : NX, SMEP, intégrité du noyau, analyse des fichiers",
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

const WELCOME: [&str; 5] = [
    "Ce bureau est écrit par des agents, et vérifié par une CI :",
    "chaque image publiée a démarré dans QEMU avant d'arriver ici.",
    "",
    "Le bouton en haut à gauche ouvre les applications ; F1 aussi.",
    "Elle ne s'ouvre qu'au premier démarrage. Sans disque, chacun en est un.",
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

const NOTES_CAP: usize = 16384;
/// How long a window takes to open, and a menu to unfold.
const WINDOW_MS: u64 = 170;
const MENU_MS: u64 = 130;
const SPLASH_MS: u64 = 900;
const CHECK_MS: u64 = 1400;

/// Where the update check has got to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Update {
    Idle,
    Checking(u64),
    Done,
}

/// What the pointer is over, so it can light up.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Hover {
    None,
    Logo,
    Quick(usize),
    Space(usize),
    Task(usize),
    Power,
    MenuItem(usize),
    Button(App, usize),
}

pub struct Desktop {
    width: usize,
    height: usize,
    panel: usize,
    now: DateTime,
    ms: u64,
    /// When the last animated frame was painted: movement is redrawn about
    /// sixty times a second whatever the timer's pace, and never faster than
    /// the machine can follow.
    painted: u64,
    input: Input,
    machine: Machine,
    fs: Fs,
    windows: [Window; 8],
    /// Back to front.
    order: [App; 8],
    space: u8,
    menu: bool,
    menu_since: u64,
    filter: String,
    choice: usize,
    power_menu: bool,
    hover: Hover,
    shell: Shell,
    notes: Vec<char>,
    notes_file: Option<String>,
    notes_note: String,
    dir: String,
    picked: Option<String>,
    address: String,
    page: String,
    trail: Vec<String>,
    status: String,
    /// What the network says about itself, refreshed every second by the
    /// kernel: the desktop itself knows nothing about cards.
    net_lines: Vec<String>,
    /// A page the browser wants fetched from the network, taken by the kernel.
    pending: Option<String>,
    /// What came back for it.
    fetched: Option<String>,
    /// What the protection has to say, and what it found; and what the
    /// Sécurité window is asking the kernel to do (scan, verify).
    sec_lines: Vec<String>,
    sec_threats: Vec<String>,
    sec_ask: Option<(bool, bool)>,
    section: usize,
    update: Update,
    password: Option<String>,
    typed: String,
    locked: bool,
    wrong: bool,
    off: bool,
    pointer: (usize, usize),
    shown: Option<(usize, usize)>,
    grab: Option<(App, usize, usize)>,
    left_down: bool,
    dirty: Rect,
}

impl Desktop {
    /// `first` is true when nothing says this machine has been started before:
    /// the welcome window then opens by itself, and only then.
    pub fn new(screen: &Screen, now: DateTime, machine: Machine, fs: Fs, first: bool) -> Self {
        let (width, height) = (screen.width(), screen.height());
        let panel = (font::height(Font::Body) + 16).max(30);
        let windows = APPS.map(|app| {
            let area = default_rect(app, width, height, panel);
            Window { open: false, hidden: false, area, restore: area, full: false, motion: None }
        });
        let mut desktop = Desktop {
            width,
            height,
            panel,
            now,
            ms: 0,
            painted: 0,
            input: Input::default(),
            machine,
            fs,
            windows,
            order: [
                App::Security,
                App::Settings,
                App::About,
                App::Notes,
                App::Browser,
                App::Files,
                App::Terminal,
                App::Welcome,
            ],
            space: 1,
            menu: false,
            menu_since: 0,
            filter: String::new(),
            choice: 0,
            power_menu: false,
            hover: Hover::None,
            shell: Shell::new(),
            notes: Vec::new(),
            notes_file: None,
            notes_note: String::new(),
            dir: "/".to_string(),
            picked: None,
            address: web::HOME.to_string(),
            page: web::HOME.to_string(),
            trail: Vec::new(),
            status: String::new(),
            net_lines: Vec::new(),
            pending: None,
            fetched: None,
            sec_lines: Vec::new(),
            sec_threats: Vec::new(),
            sec_ask: None,
            section: 0,
            update: Update::Idle,
            password: None,
            typed: String::new(),
            locked: false,
            wrong: false,
            off: false,
            pointer: (width / 2, height / 2),
            shown: None,
            grab: None,
            left_down: false,
            dirty: Rect::new(0, 0, width, height),
        };
        if first {
            desktop.launch_window(App::Welcome);
        }
        desktop
    }

    // ---- What the kernel calls --------------------------------------------

    /// A new second: the clock, and whatever shows a counter.
    pub fn tick(&mut self, now: DateTime, input: Input) {
        self.input = input;
        if self.off || now.second == self.now.second {
            return;
        }
        self.now = now;
        if self.locked {
            self.damage_all();
            return;
        }
        let clock = self.clock_rect();
        self.damage(Rect::new(clock.x, 0, self.width - clock.x, self.panel));
        if self.showing(App::Settings) && (self.section == 1 || self.section == 0) {
            let area = self.windows[App::Settings.index()].area;
            self.damage(area);
        }
    }

    /// The clock in milliseconds, from the kernel, every time round the loop:
    /// this is what moves the animations, and nothing else does.
    pub fn advance(&mut self, ms: u64) {
        self.ms = ms;
        if self.off || self.locked && ms > SPLASH_MS + 260 && self.painted + 400 > ms {
            return;
        }
        let blinked = (self.painted % 1000 < 600) != (ms % 1000 < 600);
        let moving = self.windows.iter().any(|window| window.motion.is_some())
            || (self.menu && ms < self.menu_since + MENU_MS)
            || matches!(self.update, Update::Checking(_))
            || ms <= SPLASH_MS + 260;
        if !moving && !blinked {
            return;
        }
        // Sixty frames a second at most: the animations are measured against
        // the clock, so drawing fewer of them makes them no faster and no
        // slower — only less smooth.
        if ms < self.painted + 16 {
            return;
        }
        self.painted = ms;

        if ms <= SPLASH_MS + 260 {
            self.damage_all();
        }
        for index in 0..self.windows.len() {
            let Some(motion) = self.windows[index].motion else {
                continue;
            };
            if ms >= motion.start + motion.length {
                self.windows[index].motion = None;
                if motion.closing {
                    self.windows[index].open = false;
                }
                self.damage_all();
            } else {
                let area = self.windows[index].area;
                self.damage(area.grow(10));
                self.damage(motion.from.grow(10));
            }
        }
        if self.menu && ms < self.menu_since + MENU_MS {
            self.damage(self.menu_rect().grow(10));
        }
        if let Update::Checking(start) = self.update {
            if ms >= start + CHECK_MS {
                self.update = Update::Done;
            }
            if self.showing(App::Settings) {
                self.damage(self.windows[App::Settings.index()].area);
            }
        }
        if blinked {
            // Only the caret of the window being typed in.
            if let Some(app @ (App::Terminal | App::Notes)) = self.front() {
                self.damage(self.windows[app.index()].area);
            }
            if self.locked || (self.showing(App::Settings) && self.section == 5) {
                self.damage_all();
            }
        }
    }

    /// Where the pointer is: the kernel says so on the serial line when the
    /// first packet arrives, and the preview harness follows it.
    pub fn pointer(&self) -> (usize, usize) {
        self.pointer
    }

    /// The address the browser is waiting for, handed over once. The desktop
    /// asks; the kernel, which owns the card, answers.
    pub fn wants_page(&mut self) -> Option<String> {
        self.pending.take()
    }

    /// What the network made of that address.
    pub fn page_result(&mut self, status: String, text: Option<String>) {
        self.status = status;
        if let Some(text) = text {
            self.fetched = Some(text);
        }
        if self.showing(App::Browser) {
            let area = self.windows[App::Browser.index()].area;
            self.damage(area);
        }
    }

    /// What Paramètres shows under Réseau, from the kernel's own stack.
    pub fn set_network(&mut self, lines: Vec<String>) {
        if lines == self.net_lines {
            return;
        }
        self.net_lines = lines;
        if self.showing(App::Settings) && self.section == 4 {
            let area = self.windows[App::Settings.index()].area;
            self.damage(area);
        }
    }

    /// What the Sécurité window is asking for: (scan the files, measure the
    /// kernel again). Handed over once, like the browser's page.
    pub fn wants_security(&mut self) -> Option<(bool, bool)> {
        self.sec_ask.take()
    }

    /// What the protection reports, and what it found.
    pub fn set_security(&mut self, lines: Vec<String>, threats: Vec<String>) {
        if lines == self.sec_lines && threats == self.sec_threats {
            return;
        }
        self.sec_lines = lines;
        self.sec_threats = threats;
        if self.showing(App::Security) {
            let area = self.windows[App::Security.index()].area;
            self.damage(area);
        }
    }

    /// The files, for the one thing that has to reach into them from outside:
    /// the scanner, which lives in the kernel because it reads the kernel too.
    pub fn files_mut(&mut self) -> &mut Fs {
        &mut self.fs
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
        window.open && !window.hidden
    }

    fn front(&self) -> Option<App> {
        self.order.iter().rev().copied().find(|&app| self.showing(app))
    }

    fn open_apps(&self) -> Vec<App> {
        APPS.into_iter().filter(|&app| self.windows[app.index()].open).collect()
    }

    fn raise(&mut self, app: App) {
        if let Some(at) = self.order.iter().position(|&other| other == app) {
            self.order[at..].rotate_left(1);
        }
    }

    /// Opens a window, growing it out of the panel button it belongs to.
    fn launch_window(&mut self, app: App) {
        let index = app.index();
        let already = self.windows[index].open && !self.windows[index].hidden;
        self.windows[index].open = true;
        self.windows[index].hidden = false;
        if !already {
            let from = self.spring(app);
            let ms = self.ms;
            self.windows[index].motion = Some(Motion { from, start: ms, length: WINDOW_MS, closing: false });
        }
        self.raise(app);
        self.damage_all();
    }

    /// The little rectangle a window grows from, and shrinks back to: its
    /// button in the panel when it has one, its own middle otherwise.
    fn spring(&self, app: App) -> Rect {
        if let Some(index) = self.open_apps().iter().position(|&other| other == app) {
            let button = self.task_rect(index);
            if button.right() < self.width {
                return button;
            }
        }
        let area = self.windows[app.index()].area;
        Rect::new(area.x + area.w / 3, area.y + area.h / 3, area.w / 3, area.h / 3)
    }

    fn close(&mut self, app: App) {
        let from = self.windows[app.index()].area;
        let ms = self.ms;
        self.windows[app.index()].hidden = false;
        self.windows[app.index()].motion = Some(Motion { from, start: ms, length: WINDOW_MS, closing: true });
        self.damage_all();
    }

    /// Reduce: the window leaves the screen but keeps its place in the panel,
    /// and a click on its button brings it back. It used to be a second close
    /// button, which lost what was in it.
    fn minimise(&mut self, app: App) {
        self.windows[app.index()].hidden = true;
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
        let left = self.quick_rect(QUICK.len() - 1).right() + 14 + index * (side + 4);
        Rect::new(left, 6, side, side)
    }

    /// The button of the `index`th open window.
    fn task_rect(&self, index: usize) -> Rect {
        let left = self.space_rect(3).right() + 14;
        let wide = Screen::text_width("Navigateur", Font::Body) + 46;
        let room = self.tray_left().saturating_sub(left).max(80);
        let count = self.open_apps().len().max(1);
        let width = wide.min(room / count).max(font::width(Font::Body) * 4);
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
        self.clock_rect().x
    }

    fn menu_row(&self) -> usize {
        font::height(Font::Body) + font::height(Font::Small) + 14
    }

    fn search_h(&self) -> usize {
        font::height(Font::Body) + 14
    }

    /// The menu, at its full size; `paint_menu` unfolds it.
    fn menu_rect(&self) -> Rect {
        let row = self.menu_row();
        let width = (Screen::text_width("Système, souris, écran, compte, mise à jour", Font::Small) + 80)
            .min(self.width.saturating_sub(12));
        let height = row * self.filtered().count().max(1) + self.search_h() + self.line_h() + 26;
        Rect::new(6, self.panel + 6, width, height.min(self.height.saturating_sub(self.panel + 16)))
    }

    pub fn menu_item_rect(&self, index: usize) -> Rect {
        let menu = self.menu_rect();
        let top = menu.y + self.search_h() + 12 + index * self.menu_row();
        Rect::new(menu.x + 6, top, menu.w - 12, self.menu_row())
    }

    /// The menu entries the search box keeps.
    fn filtered(&self) -> impl Iterator<Item = (usize, &'static (&'static str, &'static str, Item, Icon))> + '_ {
        ITEMS.iter().enumerate().filter(|(_, (name, _, _, _))| matches(name, &self.filter))
    }

    fn power_menu_rect(&self) -> Rect {
        let width = Screen::text_width("Redémarrer", Font::Body) + 70;
        Rect::new(self.width - width - 8, self.panel + 6, width, 3 * self.line_h() + 20)
    }

    pub fn power_item_rect(&self, index: usize) -> Rect {
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

    /// The three buttons at the right of a title bar: 0 close, 1 maximise,
    /// 2 reduce.
    pub fn button_rect(&self, app: App, index: usize) -> Rect {
        let title = self.title_rect(app);
        let side = font::height(Font::Body) + 6;
        let top = title.y + (title.h - side) / 2;
        Rect::new(title.right().saturating_sub((index + 1) * (side + 6)), top, side, side)
    }

    /// A toolbar across the top of a window's client area.
    fn bar_rect(&self, app: App) -> Rect {
        let client = self.client_rect(app);
        Rect::new(client.x, client.y, client.w, self.line_h() + 12)
    }

    pub fn bar_button(&self, app: App, index: usize, width: usize) -> Rect {
        let bar = self.bar_rect(app);
        let mut x = bar.x + 8;
        for step in 0..index {
            x += self.bar_width(app, step) + 6;
        }
        Rect::new(x, bar.y + 6, width, bar.h - 12)
    }

    pub fn bar_width(&self, app: App, index: usize) -> usize {
        let label = match (app, index) {
            (App::Notes, 0) => "Enregistrer",
            (App::Notes, 1) => "Nouveau",
            (App::Files, 0) => "Dossier parent",
            (App::Files, 1) => "Nouveau dossier",
            (App::Files, 2) => "Effacer",
            _ => "",
        };
        // The file buttons carry an icon as well as their name.
        Screen::text_width(label, Font::Body) + 24 + if app == App::Files { 20 } else { 0 }
    }

    pub fn side_rect(&self, index: usize) -> Rect {
        let client = self.client_rect(App::Settings);
        let width = (Screen::text_width("Souris et clavier", Font::Body) + 46).min(client.w / 2);
        Rect::new(client.x, client.y + 8 + index * self.line_h(), width, self.line_h())
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

    /// Where a file sits in the explorer's list.
    pub fn file_rect(&self, index: usize) -> Rect {
        let client = self.client_rect(App::Files);
        let top = self.bar_rect(App::Files).bottom() + 6 + index * self.line_h();
        Rect::new(client.x + 6, top, client.w.saturating_sub(12), self.line_h())
    }

    fn address_rect(&self) -> Rect {
        let bar = self.bar_rect(App::Browser);
        let side = bar.h - 12;
        Rect::new(bar.x + 2 * side + 16, bar.y + 6, bar.w.saturating_sub(3 * side + 30), side)
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
            let window = self.windows[app.index()];
            if window.motion.is_some() {
                self.paint_ghost(screen, app);
            } else if self.showing(app) {
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
        if self.locked {
            self.paint_login(screen);
        }
        // The first second belongs to the splash, which fades out.
        if self.ms < SPLASH_MS + 250 {
            self.paint_splash(screen);
        }
    }

    fn paint_wallpaper(&self, screen: &mut Screen) {
        screen.gradient(Rect::new(0, 0, self.width, self.height), WALL_TOP, WALL_BOTTOM);
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

    /// The screen the machine shows while the desktop is still being built.
    fn paint_splash(&self, screen: &mut Screen) {
        let fade = anim::progress(self.ms, SPLASH_MS, 250);
        let cover = 255 - (fade * 255 / FULL) as u8;
        if cover == 0 {
            return;
        }
        screen.wash(Rect::new(0, 0, self.width, self.height), Rgb(0x05, 0x07, 0x0D), cover);
        if fade > FULL / 2 {
            return;
        }
        let badge = 64;
        let x = (self.width - badge) / 2;
        let y = self.height / 2 - badge;
        screen.round(Rect::new(x, y, badge, badge), 14, ACCENT);
        icons::draw(screen, Rect::new(x + 14, y + 14, badge - 28, badge - 28), Icon::Home, WHITE, ACCENT);
        let name = "grenOS";
        screen.text(
            (self.width - Screen::text_width(name, Font::Title)) / 2,
            y + badge + 18,
            name,
            TEXT,
            Font::Title,
        );
        // A bar that fills while the kernel finishes waking up.
        let bar = Rect::new(self.width / 2 - 90, y + badge + 60, 180, 4);
        screen.round(bar, 2, SURFACE);
        let done = anim::progress(self.ms, 0, SPLASH_MS);
        screen.round(Rect::new(bar.x, bar.y, anim::mix(0, bar.w, done), bar.h), 2, ACCENT);
    }

    fn paint_panel(&self, screen: &mut Screen) {
        screen.fill(Rect::new(0, 0, self.width, self.panel), PANEL);
        screen.fill(Rect::new(0, self.panel - 1, self.width, 1), LINE);

        let logo = self.logo_rect();
        if self.menu || self.hover == Hover::Logo {
            screen.round(logo, 6, if self.menu { ACCENT_SOFT } else { HOVER });
        }
        let badge = Rect::new(logo.x + 6, logo.y + 3, logo.h - 6, logo.h - 6);
        screen.round(badge, 5, ACCENT);
        icons::draw(screen, badge.inset(4), Icon::Home, WHITE, ACCENT);
        screen.text(badge.right() + 8, centre_y(logo, Font::Head), "grenOS", TEXT, Font::Head);

        for (index, app) in QUICK.into_iter().enumerate() {
            let area = self.quick_rect(index);
            let lit = self.hover == Hover::Quick(index);
            if lit || self.windows[app.index()].open {
                screen.round(area, 5, if lit { HOVER } else { SURFACE_ALT });
            }
            icons::draw(screen, area.inset(4), app.icon(), app.tint(), PANEL);
        }

        for index in 0..4 {
            let area = self.space_rect(index);
            let here = index as u8 + 1 == self.space;
            let lit = self.hover == Hover::Space(index);
            screen.round(area, 4, if here { ACCENT } else if lit { HOVER } else { SURFACE_ALT });
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
        for (index, app) in self.open_apps().into_iter().enumerate() {
            let area = self.task_rect(index);
            if area.right() > self.tray_left() {
                break;
            }
            let active = Some(app) == front;
            let lit = self.hover == Hover::Task(index);
            if active || lit {
                screen.round(area, 5, if active { SURFACE } else { HOVER });
            }
            if active {
                screen.fill(Rect::new(area.x + 6, area.bottom() - 2, area.w - 12, 2), ACCENT);
            }
            let icon = Rect::new(area.x + 6, area.y + (area.h - 16) / 2, 16, 16);
            icons::draw(screen, icon, app.icon(), app.tint(), if active { SURFACE } else { PANEL });
            let room = (area.w.saturating_sub(30)) / font::width(Font::Body);
            let label = app.short();
            let end = label.char_indices().nth(room).map_or(label.len(), |(at, _)| at);
            let colour = if self.windows[app.index()].hidden { TEXT_FAINT } else if active { TEXT } else { TEXT_DIM };
            screen.text(icon.right() + 6, centre_y(area, Font::Body), &label[..end], colour, Font::Body);
        }

        let clock = self.clock_rect();
        screen.text(clock.x + 6, centre_y(clock, Font::Body), &self.clock_text(), TEXT, Font::Body);

        let power = self.power_rect();
        if self.power_menu || self.hover == Hover::Power {
            screen.round(power, 5, if self.power_menu { ACCENT_SOFT } else { HOVER });
        }
        let behind = if self.power_menu { ACCENT_SOFT } else if self.hover == Hover::Power { HOVER } else { PANEL };
        icons::draw(screen, power.inset(3), Icon::Power, TEXT_DIM, behind);
    }

    /// A window on its way in or out: the frame alone, sliding and growing.
    fn paint_ghost(&self, screen: &mut Screen, app: App) {
        let Some(motion) = self.windows[app.index()].motion else {
            return;
        };
        let raw = anim::progress(self.ms, motion.start, motion.length);
        let p = if motion.closing { anim::ease_in(raw) } else { anim::ease_out(raw) };
        let (from, to) = if motion.closing {
            (self.windows[app.index()].area, motion.from)
        } else {
            (motion.from, self.windows[app.index()].area)
        };
        let area = Rect::new(
            anim::mix(from.x, to.x, p),
            anim::mix(from.y, to.y, p),
            anim::mix(from.w, to.w, p).max(8),
            anim::mix(from.h, to.h, p).max(8),
        );
        screen.shadow(area, 4);
        screen.round(area, 8, SURFACE);
        let title = Rect::new(area.x, area.y, area.w, (self.title_h() * area.h / self.height.max(1)).max(6).min(area.h));
        screen.round(title, 6, SURFACE_ALT);
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

        let icon = Rect::new(title.x + 10, title.y + (title.h - 18) / 2, 18, 18);
        icons::draw(screen, icon, app.icon(), app.tint(), if active { SURFACE_ALT } else { PANEL });
        // The browser names the page it is on, the way a tab would.
        let heading = match app {
            App::Browser => web::find(&self.page)
                .map_or_else(|| app.title().to_string(), |page| format!("{} · {}", app.title(), page.title)),
            other => other.title().to_string(),
        };
        screen.text(icon.right() + 10, centre_y(title, Font::Head), &heading, if active { TEXT } else { TEXT_DIM }, Font::Head);

        for index in 0..3 {
            let button = self.button_rect(app, index);
            let lit = self.hover == Hover::Button(app, index);
            if lit {
                screen.round(button, 4, if index == 0 { RED } else { HOVER });
            }
            let colour = if lit && index == 0 { WHITE } else if index == 0 { RED } else { TEXT_DIM };
            let (cx, cy) = (button.x + button.w / 2, button.y + button.h / 2);
            match index {
                0 => {
                    for step in 0..7 {
                        screen.fill(Rect::new(cx - 3 + step, cy - 3 + step, 1, 1), colour);
                        screen.fill(Rect::new(cx + 3 - step, cy - 3 + step, 1, 1), colour);
                    }
                }
                1 => screen.border(Rect::new(cx - 4, cy - 4, 9, 9), colour),
                _ => screen.fill(Rect::new(cx - 4, cy + 3, 9, 1), colour),
            }
        }

        // Nothing a window draws leaves the window.
        let client = self.client_rect(app);
        let previous = screen.set_clip(client.intersect(screen.clip()));
        match app {
            App::Welcome => self.paint_welcome(screen, client),
            App::Terminal => self.paint_terminal(screen, client),
            App::Files => self.paint_files(screen, client),
            App::Browser => self.paint_browser(screen, client),
            App::Notes => self.paint_notes(screen, client),
            App::Settings => self.paint_settings(screen, client),
            App::About => self.paint_about(screen, client),
            App::Security => self.paint_security(screen, client),
        }
        screen.set_clip(previous);
    }

    fn paint_welcome(&self, screen: &mut Screen, client: Rect) {
        let badge = Rect::new(client.x + 24, client.y + 20, 52, 52);
        screen.round(badge, 12, ACCENT);
        icons::draw(screen, badge.inset(12), Icon::Home, WHITE, ACCENT);
        screen.text(badge.right() + 16, client.y + 24, "Bienvenue", TEXT, Font::Title);
        screen.text(
            badge.right() + 16,
            client.y + 28 + font::height(Font::Title),
            &format!("grenOS {} · build {}", self.machine.version, self.machine.build),
            TEXT_DIM,
            Font::Small,
        );
        let mut y = badge.bottom() + 18;
        for line in WELCOME {
            screen.text(client.x + 24, y, line, TEXT_DIM, Font::Body);
            y += self.line_h();
        }
        for (index, app) in [App::Terminal, App::Files, App::Browser].into_iter().enumerate() {
            let card = Rect::new(client.x + 24 + index * 130, y + 12, 118, 46);
            screen.round(card, 8, SURFACE_ALT);
            icons::draw(screen, Rect::new(card.x + 12, card.y + 13, 20, 20), app.icon(), app.tint(), SURFACE_ALT);
            screen.text(card.x + 40, centre_y(card, Font::Small), app.short(), TEXT_DIM, Font::Small);
        }
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
                Line::Command(cwd, text) => {
                    let x = prompt(screen, client.x + 10, y, cwd);
                    screen.text(x, y, text, TEXT, Font::Body);
                }
                Line::Output(text) => {
                    screen.text(client.x + 10, y, text, TEXT_DIM, Font::Body);
                }
            }
            y += step;
        }
        if y + step <= client.bottom() {
            let x = prompt(screen, client.x + 10, y, self.shell.cwd());
            let x = screen.text(x, y, self.shell.input(), TEXT, Font::Body);
            // The caret blinks on the millisecond clock, like everything else.
            if self.ms % 1000 < 600 {
                screen.fill(Rect::new(x + 1, y + 2, font::width(Font::Body) - 1, font::height(Font::Body) - 4), ACCENT);
            }
        }
    }

    fn paint_files(&self, screen: &mut Screen, client: Rect) {
        screen.fill(client, SURFACE);
        let bar = self.bar_rect(App::Files);
        screen.fill(bar, SURFACE_ALT);
        screen.fill(Rect::new(bar.x, bar.bottom() - 1, bar.w, 1), LINE);
        for (index, (label, icon)) in
            [("Dossier parent", Icon::Back), ("Nouveau dossier", Icon::Plus), ("Effacer", Icon::Trash)]
                .into_iter()
                .enumerate()
        {
            let button = self.bar_button(App::Files, index, self.bar_width(App::Files, index));
            screen.round(button, 5, SUNKEN);
            let mark = Rect::new(button.x + 8, button.y + (button.h - 14) / 2, 14, 14);
            icons::draw(screen, mark, icon, TEXT_DIM, SUNKEN);
            screen.text(mark.right() + 6, centre_y(button, Font::Body), label, TEXT_DIM, Font::Body);
        }
        let path = format!("  {}", self.dir);
        screen.text(
            self.bar_button(App::Files, 3, 0).x + 6,
            centre_y(bar, Font::Small),
            &path,
            TEXT_FAINT,
            Font::Small,
        );

        let entries = self.fs.list(&self.dir);
        if entries.is_empty() {
            screen.text(client.x + 16, self.file_rect(0).y, "Ce dossier est vide.", TEXT_FAINT, Font::Body);
        }
        for (index, entry) in entries.iter().enumerate() {
            let row = self.file_rect(index);
            if row.bottom() > client.bottom() {
                break;
            }
            let picked = self.picked.as_deref() == Some(entry.path.as_str());
            if picked {
                screen.round(row, 5, ACCENT_SOFT);
            }
            let icon = Rect::new(row.x + 6, row.y + (row.h - 16) / 2, 16, 16);
            let (kind, tint) = match entry.kind {
                Kind::Dir => (Icon::Folder, AMBER),
                Kind::File => (Icon::File, Rgb(0xD8, 0xDE, 0xEA)),
            };
            icons::draw(screen, icon, kind, tint, if picked { ACCENT_SOFT } else { SURFACE });
            screen.text(icon.right() + 10, centre_y(row, Font::Body), entry.name(), TEXT, Font::Body);
            let detail = match entry.kind {
                Kind::Dir => "dossier".to_string(),
                Kind::File => format!("{} octets", entry.size()),
            };
            screen.text(
                row.right().saturating_sub(Screen::text_width(&detail, Font::Small) + 12),
                centre_y(row, Font::Small),
                &detail,
                TEXT_FAINT,
                Font::Small,
            );
        }
        let note = "En mémoire : tout disparaît à l'extinction.";
        screen.text(client.x + 12, client.bottom().saturating_sub(font::height(Font::Small) + 8), note, TEXT_FAINT, Font::Small);
    }

    fn paint_browser(&self, screen: &mut Screen, client: Rect) {
        screen.fill(client, SURFACE);
        let bar = self.bar_rect(App::Browser);
        screen.fill(bar, SURFACE_ALT);
        screen.fill(Rect::new(bar.x, bar.bottom() - 1, bar.w, 1), LINE);
        let side = bar.h - 12;
        icons::draw(screen, Rect::new(bar.x + 8, bar.y + 6, side, side), Icon::Back, if self.trail.is_empty() { TEXT_FAINT } else { TEXT_DIM }, SURFACE_ALT);
        icons::draw(screen, Rect::new(bar.x + 12 + side, bar.y + 6, side, side), Icon::Home, TEXT_DIM, SURFACE_ALT);
        let address = self.address_rect();
        screen.round(address, 5, SUNKEN);
        screen.text(address.x + 10, centre_y(address, Font::Body), &self.address, TEXT, Font::Body);
        let go = Rect::new(address.right() + 8, address.y, address.h, address.h);
        icons::draw(screen, go.inset(4), Icon::Forward, TEXT_DIM, SURFACE_ALT);

        let page = client.y + bar.h + 10;
        let width = client.w.saturating_sub(48) / font::width(Font::Body);
        let mut y = page;
        if let Some(rest) = self.page.strip_prefix("fichier:") {
            self.paint_browser_files(screen, client, rest, &mut y);
        } else if let Some(found) = web::find(&self.page) {
            for block in web::parse(found.body) {
                if y + self.line_h() > client.bottom() {
                    break;
                }
                y = paint_block(screen, client.x + 24, y, width, block, self.line_h());
            }
        } else if self.page.starts_with("http") {
            // A page from the network: the text the kernel brought back, or
            // where it has got to.
            match self.fetched.as_deref() {
                Some(text) if !text.is_empty() => {
                    for line in text.lines().flat_map(|line| wrap(line, width)) {
                        if y + self.line_h() > client.bottom() {
                            break;
                        }
                        screen.text(client.x + 24, y, line, TEXT_DIM, Font::Body);
                        y += self.line_h();
                    }
                }
                _ => {
                    screen.text(client.x + 24, y, &self.status, TEXT_DIM, Font::Body);
                }
            }
        } else {
            screen.text(client.x + 24, y, "Page introuvable.", TEXT_DIM, Font::Body);
            y += self.line_h();
            screen.text(client.x + 24, y, &self.status, AMBER, Font::Small);
        }
        if !self.status.is_empty() {
            let bottom = client.bottom().saturating_sub(font::height(Font::Small) + 8);
            screen.text(client.x + 12, bottom, &self.status, TEXT_FAINT, Font::Small);
        }
    }

    /// `fichier:/chemin` in the address bar: the browser shows the file system.
    fn paint_browser_files(&self, screen: &mut Screen, client: Rect, path: &str, y: &mut usize) {
        if let Some(text) = self.fs.read(path) {
            screen.text(client.x + 24, *y, fs::name_of(path), TEXT, Font::Title);
            *y += font::height(Font::Title) + 10;
            for line in text.lines() {
                if *y + self.line_h() > client.bottom() {
                    return;
                }
                screen.text(client.x + 24, *y, line, TEXT_DIM, Font::Body);
                *y += self.line_h();
            }
            return;
        }
        screen.text(client.x + 24, *y, path, TEXT, Font::Title);
        *y += font::height(Font::Title) + 10;
        for entry in self.fs.list(path) {
            if *y + self.line_h() > client.bottom() {
                return;
            }
            let icon = Rect::new(client.x + 24, *y + 2, 14, 14);
            let (kind, tint) = match entry.kind {
                Kind::Dir => (Icon::Folder, AMBER),
                Kind::File => (Icon::File, Rgb(0xD8, 0xDE, 0xEA)),
            };
            icons::draw(screen, icon, kind, tint, SURFACE);
            screen.text(icon.right() + 10, *y, entry.name(), ACCENT, Font::Body);
            *y += self.line_h();
        }
    }

    /// The notepad, wrapped between words, with the caret at the end.
    fn paint_notes(&self, screen: &mut Screen, client: Rect) {
        screen.fill(client, PAPER);
        let bar = self.bar_rect(App::Notes);
        screen.fill(bar, SURFACE_ALT);
        screen.fill(Rect::new(bar.x, bar.bottom() - 1, bar.w, 1), LINE);
        for (index, label) in ["Enregistrer", "Nouveau"].into_iter().enumerate() {
            let button = self.bar_button(App::Notes, index, self.bar_width(App::Notes, index));
            screen.round(button, 5, if index == 0 { ACCENT } else { SUNKEN });
            screen.text(button.x + 12, centre_y(button, Font::Body), label, if index == 0 { WHITE } else { TEXT_DIM }, Font::Body);
        }
        let name = self.notes_file.clone().unwrap_or_else(|| "(pas encore enregistré)".to_string());
        let after = self.bar_button(App::Notes, 2, 0).x + 8;
        screen.text(after, centre_y(bar, Font::Small), &name, TEXT_FAINT, Font::Small);
        if !self.notes_note.is_empty() {
            let width = Screen::text_width(&self.notes_note, Font::Small);
            screen.text(bar.right().saturating_sub(width + 12), centre_y(bar, Font::Small), &self.notes_note, GREEN, Font::Small);
        }

        let area = Rect::new(client.x, bar.bottom(), client.w, client.h.saturating_sub(bar.h));
        let glyph = font::width(Font::Body);
        let step = font::height(Font::Body) + 2;
        let columns = (area.w.saturating_sub(24) / glyph).max(1);
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
            let y = area.y + 10 + row * step;
            if y + step > area.bottom() {
                return;
            }
            screen.text(area.x + 12 + column * glyph, y, c.encode_utf8(&mut buffer), TEXT, Font::Body);
            column += 1;
        }
        if column == columns {
            column = 0;
            row += 1;
        }
        let y = area.y + 10 + row * step;
        if y + step <= area.bottom() && self.ms % 1000 < 600 {
            screen.fill(Rect::new(area.x + 12 + column * glyph, y + 2, glyph - 1, font::height(Font::Body) - 4), ACCENT);
        }
    }

    fn paint_about(&self, screen: &mut Screen, client: Rect) {
        let badge = Rect::new(client.x + 20, client.y + 18, 46, 46);
        screen.round(badge, 10, ACCENT);
        icons::draw(screen, badge.inset(10), Icon::Home, WHITE, ACCENT);
        let left = badge.right() + 16;
        screen.text(left, client.y + 18, "grenOS", TEXT, Font::Title);
        let version = format!("version {} · build {} · 64 bits", self.machine.version, self.machine.build);
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

    /// Where the two buttons of the Sécurité window are.
    pub fn security_button(&self, index: usize) -> Rect {
        let client = self.client_rect(App::Security);
        let width = Screen::text_width("Analyser les fichiers", Font::Body) + 28;
        let top = (client.y + 96 + 12 * self.line_h()).min(client.bottom().saturating_sub(2 * self.line_h() + 20));
        Rect::new(client.x + 20 + index * (width + 10), top, width, self.line_h() + 8)
    }

    /// Protection: what the processor enforces, what the kernel measures of
    /// itself, and what the scanner found in the files.
    fn paint_security(&self, screen: &mut Screen, client: Rect) {
        screen.fill(client, SURFACE);
        let alarmed = !self.sec_threats.is_empty() || self.sec_lines.iter().any(|line| line.contains("MODIFIÉ"));
        let colour = if alarmed { RED } else { GREEN };
        let badge = Rect::new(client.x + 20, client.y + 18, 46, 46);
        screen.round(badge, 10, colour);
        icons::draw(screen, badge.inset(10), Icon::Lock, WHITE, colour);
        let (title, under) = if alarmed {
            ("Attention", "Une menace ou une modification a été trouvée.")
        } else {
            ("Protection active", "Le processeur, le noyau et les fichiers ont été vérifiés.")
        };
        screen.text(badge.right() + 16, client.y + 18, title, TEXT, Font::Title);
        screen.text(badge.right() + 16, client.y + 22 + font::height(Font::Title), under, TEXT_DIM, Font::Small);

        let body = Rect::new(client.x + 20, badge.bottom() + 16, client.w.saturating_sub(40), client.h);
        let lines = if self.sec_lines.is_empty() {
            alloc::vec!["Analyse en cours...".to_string()]
        } else {
            self.sec_lines.clone()
        };
        self.paint_rows(screen, body, body.y, &lines);

        for (index, label) in ["Analyser les fichiers", "Vérifier l'intégrité"].into_iter().enumerate() {
            let button = self.security_button(index);
            screen.round(button, 6, if index == 0 { ACCENT } else { SUNKEN });
            screen.text(
                button.x + 14,
                centre_y(button, Font::Body),
                label,
                if index == 0 { WHITE } else { TEXT_DIM },
                Font::Body,
            );
        }

        let mut y = self.security_button(0).bottom() + 12;
        if self.sec_threats.is_empty() {
            screen.text(client.x + 20, y, "Aucune menace dans les fichiers de la machine.", TEXT_FAINT, Font::Small);
            return;
        }
        for line in &self.sec_threats {
            if y + self.line_h() > client.bottom() {
                return;
            }
            screen.fill(Rect::new(client.x + 20, y + font::height(Font::Small) / 2, 4, 4), RED);
            screen.text(client.x + 32, y, line, RED, Font::Small);
            y += self.line_h();
        }
    }

    fn paint_settings(&self, screen: &mut Screen, client: Rect) {
        screen.fill(client, SURFACE);
        let side = Rect::new(client.x, client.y, self.side_rect(0).w, client.h);
        screen.fill(side, SURFACE_ALT);
        screen.fill(Rect::new(side.right(), client.y, 1, client.h), LINE);
        for (index, (name, icon)) in SECTIONS.iter().enumerate() {
            let row = self.side_rect(index);
            let here = index == self.section;
            if here {
                screen.round(Rect::new(row.x + 4, row.y, row.w - 8, row.h), 4, ACCENT_SOFT);
                screen.fill(Rect::new(row.x + 4, row.y + 3, 2, row.h - 6), ACCENT);
            }
            let mark = Rect::new(row.x + 12, row.y + (row.h - 14) / 2, 14, 14);
            icons::draw(screen, mark, *icon, if here { TEXT } else { TEXT_FAINT }, if here { ACCENT_SOFT } else { SURFACE_ALT });
            screen.text(mark.right() + 8, centre_y(row, Font::Body), name, if here { TEXT } else { TEXT_DIM }, Font::Body);
        }

        let body = self.settings_body();
        screen.text(body.x, body.y, SECTIONS[self.section].0, TEXT, Font::Title);
        let top = body.y + font::height(Font::Title) + 12;
        match self.section {
            0 => {
                let mut lines = self.machine.lines();
                lines.push(format!("Allumé depuis : {}", uptime(self.input.uptime)));
                lines.push(format!("Fichiers : {} pour {} octets", self.fs.files(), self.fs.used()));
                self.paint_rows(screen, body, top, &lines);
            }
            1 => self.paint_input_section(screen, body, top),
            2 => self.paint_rows(screen, body, top, &self.screen_lines()),
            3 => self.paint_devices(screen, body, top),
            4 => self.paint_rows(screen, body, top, &self.network_lines()),
            5 => self.paint_account(screen, body, top),
            _ => self.paint_update(screen, body, top),
        }
    }

    fn screen_lines(&self) -> Vec<String> {
        alloc::vec![
            format!("Mode : {}", self.machine.screen),
            format!("Bureau : panneau de {} pixels, {} applications", self.panel, QUICK.len() + 2),
            "Dessin : tampon en mémoire, puis une seule copie vers la carte".to_string(),
            "Police : Noto Sans Mono, lissée sur 256 niveaux".to_string(),
            "Animations : mesurées en millisecondes, donc identiques à 24 ou 360 images par seconde".to_string(),
        ]
    }

    fn network_lines(&self) -> Vec<String> {
        // What the stack itself reports, once there is one.
        if !self.net_lines.is_empty() {
            return self.net_lines.clone();
        }
        alloc::vec![
            format!("État : {}", self.machine.network),
            format!(
                "Carte : {}",
                self.machine
                    .devices
                    .iter()
                    .find(|line| line.contains("réseau"))
                    .cloned()
                    .unwrap_or_else(|| "aucune carte réseau sur le bus PCI".to_string())
            ),
            "Prochaine étape : pilote e1000, puis ARP, DHCP, DNS et TCP".to_string(),
            "Le navigateur ouvrira alors les pages en http, et la mise à jour se fera en ligne".to_string(),
        ]
    }

    fn paint_rows(&self, screen: &mut Screen, body: Rect, top: usize, lines: &[String]) {
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
            "Disposition du clavier : français (AZERTY)".to_string(),
        ];
        self.paint_rows(screen, body, top, &lines);
        let mut y = top + lines.len() * self.line_h() + 10;
        for hint in [
            "Dans VirtualBox, la souris est capturée : la touche Ctrl droite la rend.",
            "Au clavier : F1 ouvre le menu, Tab passe d'une fenêtre à l'autre, Échap ferme.",
        ] {
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
                screen.text(body.x, y, "...", TEXT_FAINT, Font::Small);
                return;
            }
            screen.text(body.x, y, line, TEXT_DIM, Font::Small);
            y += font::height(Font::Small) + 4;
        }
    }

    fn paint_account(&self, screen: &mut Screen, body: Rect, top: usize) {
        let set = self.password.is_some();
        let lines = alloc::vec![
            "Utilisateur : root".to_string(),
            format!("Mot de passe : {}", if set { "défini" } else { "aucun, le démarrage ne demande rien" }),
            format!("Au démarrage : {}", if set { "l'écran de connexion s'affiche" } else { "le bureau s'ouvre directement" }),
        ];
        self.paint_rows(screen, body, top, &lines);
        let mut y = top + lines.len() * self.line_h() + 12;
        screen.text(body.x, y, "Nouveau mot de passe", TEXT, Font::Head);
        y += self.line_h() + 4;
        let field = Rect::new(body.x, y, (Screen::text_width("00000000000000000000", Font::Body) + 24).min(body.w), self.line_h() + 8);
        screen.round(field, 5, SUNKEN);
        let caret = dots(screen, field.x + 12, field.y + field.h / 2, self.typed.chars().count(), TEXT);
        if self.ms % 1000 < 600 {
            screen.fill(Rect::new(caret, field.y + 6, 2, field.h - 12), ACCENT);
        }
        y = field.bottom() + 10;
        let button = Rect::new(body.x, y, Screen::text_width("Appliquer", Font::Body) + 28, self.line_h() + 8);
        screen.round(button, 5, ACCENT);
        screen.text(button.x + 14, centre_y(button, Font::Body), "Appliquer", WHITE, Font::Body);
        let clear = Rect::new(button.right() + 8, y, Screen::text_width("Retirer", Font::Body) + 28, self.line_h() + 8);
        screen.round(clear, 5, SUNKEN);
        screen.text(clear.x + 14, centre_y(clear, Font::Body), "Retirer", TEXT_DIM, Font::Body);
        y = button.bottom() + 12;
        screen.text(
            body.x,
            y,
            "Tapez, puis Appliquer. Sans disque, le mot de passe ne survit pas à l'extinction.",
            TEXT_FAINT,
            Font::Small,
        );
    }

    /// Where the account section's three targets are, in the order used above.
    fn account_rects(&self) -> (Rect, Rect, Rect) {
        let body = self.settings_body();
        let top = body.y + font::height(Font::Title) + 12;
        let y = top + 3 * self.line_h() + 12 + self.line_h() + 4;
        let field = Rect::new(body.x, y, (Screen::text_width("00000000000000000000", Font::Body) + 24).min(body.w), self.line_h() + 8);
        let apply = Rect::new(body.x, field.bottom() + 10, Screen::text_width("Appliquer", Font::Body) + 28, self.line_h() + 8);
        let clear = Rect::new(apply.right() + 8, apply.y, Screen::text_width("Retirer", Font::Body) + 28, self.line_h() + 8);
        (field, apply, clear)
    }

    fn paint_update(&self, screen: &mut Screen, body: Rect, top: usize) {
        let lines = alloc::vec![
            format!("Version installée : grenOS {}", self.machine.version),
            format!("Build : {}", self.machine.build),
            format!("Compilée le : {}", self.machine.built_at),
            "Canal : principal (main), publié après une CI verte".to_string(),
        ];
        self.paint_rows(screen, body, top, &lines);

        let button = self.update_button_rect();
        let checking = matches!(self.update, Update::Checking(_));
        screen.round(button, 6, if checking { SURFACE_ALT } else { ACCENT });
        let label = if checking { "Vérification..." } else { "Vérifier les mises à jour" };
        screen.text(button.x + 14, centre_y(button, Font::Body), label, if checking { TEXT_DIM } else { WHITE }, Font::Body);

        let mut y = button.bottom() + 8;
        if let Update::Checking(start) = self.update {
            // A bar that fills in step with the clock, not with the frames.
            let bar = Rect::new(body.x, y + 4, button.w, 4);
            screen.round(bar, 2, SURFACE_ALT);
            let done = anim::progress(self.ms, start, CHECK_MS);
            screen.round(Rect::new(bar.x, bar.y, anim::mix(0, bar.w, done), bar.h), 2, ACCENT);
            y += 16;
        }
        // What the check can honestly say depends on whether the card got an
        // address at all.
        let online = self.net_lines.iter().any(|line| line.starts_with("Adresse : ") && !line.ends_with("0.0.0.0"));
        let note: &str = match (self.update, online) {
            (Update::Idle, true) => "Le réseau répond. Le site, lui, n'accepte que https, que grenOS ne parle pas encore.",
            (Update::Idle, false) => "La vérification interroge la carte réseau ; elle n'a pas encore d'adresse.",
            (Update::Checking(_), _) => "Recherche du serveur de mise à jour...",
            (Update::Done, true) => "Réseau actif, mais grenos-dev.vercel.app n'accepte que https : TLS reste à écrire.",
            (Update::Done, false) => "Aucune adresse réseau : impossible de joindre grenos-dev.vercel.app.",
        };
        screen.text(body.x, y, note, if self.update == Update::Done { AMBER } else { TEXT_FAINT }, Font::Small);
        y += self.line_h();
        if self.update == Update::Done {
            screen.text(body.x, y, "En attendant, l'image se télécharge sur le site depuis un autre ordinateur.", TEXT_FAINT, Font::Small);
            y += self.line_h();
        }
        y += 6;

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
        let full = self.menu_rect();
        // The menu unfolds downwards: its height follows the clock.
        let p = anim::ease_out(anim::progress(self.ms, self.menu_since, MENU_MS));
        let menu = Rect::new(full.x, full.y, full.w, anim::mix(full.h / 3, full.h, p).max(12));
        screen.shadow(menu, 8);
        screen.round(menu, 10, SURFACE);
        let previous = screen.set_clip(menu.intersect(screen.clip()));
        screen.fill(Rect::new(menu.x, menu.y + self.search_h() + 4, menu.w, 1), LINE);

        let search = Rect::new(menu.x + 8, menu.y + 7, menu.w - 16, self.search_h() - 10);
        screen.round(search, 5, SUNKEN);
        let glass = Rect::new(search.x + 8, search.y + (search.h - 14) / 2, 14, 14);
        icons::draw(screen, glass, Icon::Search, TEXT_FAINT, SUNKEN);
        let (text, colour) = if self.filter.is_empty() {
            ("Rechercher une application", TEXT_FAINT)
        } else {
            (self.filter.as_str(), TEXT)
        };
        screen.text(glass.right() + 8, centre_y(search, Font::Body), text, colour, Font::Body);

        for (row, (index, (name, about, _, icon))) in self.filtered().enumerate() {
            let area = self.menu_item_rect(row);
            if area.bottom() > menu.bottom() {
                break;
            }
            if index == self.choice || self.hover == Hover::MenuItem(row) {
                screen.round(area, 6, if index == self.choice { ACCENT_SOFT } else { HOVER });
            }
            let behind = if index == self.choice { ACCENT_SOFT } else { SURFACE };
            let mark = Rect::new(area.x + 10, area.y + (area.h - 26) / 2, 26, 26);
            let tint = match ITEMS[index].2 {
                Item::Open(app) => app.tint(),
                Item::Lock => TEXT_DIM,
                Item::Reboot => AMBER,
                Item::Off => RED,
            };
            icons::draw(screen, mark, *icon, tint, behind);
            screen.text(mark.right() + 12, area.y + 8, name, TEXT, Font::Head);
            screen.text(mark.right() + 12, area.y + 10 + font::height(Font::Head), about, TEXT_FAINT, Font::Small);
        }

        let footer = Rect::new(menu.x + 12, menu.bottom() - self.line_h() - 8, menu.w - 24, self.line_h());
        screen.text(footer.x, footer.y, "root@grenos", TEXT_FAINT, Font::Small);
        let hint = "F1 ouvre ce menu";
        screen.text(footer.right().saturating_sub(Screen::text_width(hint, Font::Small)), footer.y, hint, TEXT_FAINT, Font::Small);
        screen.set_clip(previous);
    }

    fn paint_power_menu(&self, screen: &mut Screen) {
        let menu = self.power_menu_rect();
        screen.shadow(menu, 6);
        screen.round(menu, 8, SURFACE);
        for (index, (name, colour, icon)) in
            [("Verrouiller", TEXT_DIM, Icon::Lock), ("Redémarrer", AMBER, Icon::Restart), ("Éteindre", RED, Icon::Power)]
                .into_iter()
                .enumerate()
        {
            let row = self.power_item_rect(index);
            let mark = Rect::new(row.x + 8, row.y + (row.h - 14) / 2, 14, 14);
            icons::draw(screen, mark, icon, colour, SURFACE);
            screen.text(mark.right() + 10, centre_y(row, Font::Body), name, colour, Font::Body);
        }
    }

    /// The screen that asks for the password: at boot when one is set, and
    /// whenever the machine is locked.
    fn paint_login(&self, screen: &mut Screen) {
        screen.gradient(Rect::new(0, 0, self.width, self.height), WALL_TOP, WALL_BOTTOM);
        let card = Rect::new(self.width / 2 - 170, self.height / 2 - 110, 340, 220);
        screen.shadow(card, 8);
        screen.round(card, 12, SURFACE);
        let badge = Rect::new(card.x + card.w / 2 - 26, card.y + 22, 52, 52);
        screen.round(badge, 26, ACCENT);
        icons::draw(screen, badge.inset(12), Icon::Lock, WHITE, ACCENT);
        let name = "root";
        screen.text((self.width - Screen::text_width(name, Font::Head)) / 2, badge.bottom() + 12, name, TEXT, Font::Head);

        let field = Rect::new(card.x + 40, badge.bottom() + 44, card.w - 80, self.line_h() + 10);
        screen.round(field, 6, SUNKEN);
        let caret = dots(screen, field.x + 14, field.y + field.h / 2, self.typed.chars().count(), TEXT);
        if self.ms % 1000 < 600 {
            screen.fill(Rect::new(caret, field.y + 6, 2, field.h - 12), ACCENT);
        }
        let hint = if self.wrong { "Mot de passe incorrect" } else { "Mot de passe, puis Entrée" };
        let colour = if self.wrong { RED } else { TEXT_FAINT };
        screen.text((self.width - Screen::text_width(hint, Font::Small)) / 2, field.bottom() + 12, hint, colour, Font::Small);
        let clock = self.clock_text();
        screen.text((self.width - Screen::text_width(&clock, Font::Body)) / 2, card.bottom() + 24, &clock, TEXT_DIM, Font::Body);
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
            } else {
                self.follow();
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

    /// Lights up whatever the pointer is over.
    fn follow(&mut self) {
        let (x, y) = self.pointer;
        let hit = |area: Rect| area.contains(x, y);
        let mut found = Hover::None;
        if self.menu {
            if let Some(row) = (0..self.filtered().count()).find(|&row| hit(self.menu_item_rect(row))) {
                found = Hover::MenuItem(row);
            }
        }
        if found == Hover::None && y < self.panel {
            found = if hit(self.logo_rect()) {
                Hover::Logo
            } else if hit(self.power_rect()) {
                Hover::Power
            } else if let Some(index) = (0..QUICK.len()).find(|&index| hit(self.quick_rect(index))) {
                Hover::Quick(index)
            } else if let Some(index) = (0..4).find(|&index| hit(self.space_rect(index))) {
                Hover::Space(index)
            } else if let Some(index) = (0..self.open_apps().len()).find(|&index| hit(self.task_rect(index))) {
                Hover::Task(index)
            } else {
                Hover::None
            };
        }
        if found == Hover::None {
            if let Some(app) = self.front() {
                if let Some(index) = (0..3).find(|&index| hit(self.button_rect(app, index))) {
                    found = Hover::Button(app, index);
                }
            }
        }
        if found == self.hover {
            return;
        }
        // Only what lit up, and what stopped being lit, is redrawn.
        for state in [self.hover, found] {
            match state {
                Hover::None => {}
                Hover::Logo => self.damage(self.logo_rect()),
                Hover::Quick(index) => self.damage(self.quick_rect(index)),
                Hover::Space(index) => self.damage(self.space_rect(index)),
                Hover::Task(index) => self.damage(self.task_rect(index)),
                Hover::Power => self.damage(self.power_rect()),
                Hover::MenuItem(row) => self.damage(self.menu_item_rect(row)),
                Hover::Button(app, index) => self.damage(self.button_rect(app, index).grow(2)),
            }
        }
        self.hover = found;
    }

    fn click(&mut self) -> Option<Action> {
        let (x, y) = self.pointer;
        let hit = |area: Rect| area.contains(x, y);

        if self.locked {
            return None;
        }
        if self.power_menu {
            self.power_menu = false;
            self.damage_all();
            if hit(self.power_item_rect(0)) {
                self.lock();
                return None;
            }
            if hit(self.power_item_rect(1)) {
                return Some(Action::Reboot);
            }
            if hit(self.power_item_rect(2)) {
                return self.shut_down();
            }
            return None;
        }
        if self.menu {
            if hit(self.menu_rect()) {
                let chosen = self.filtered().enumerate().find(|(row, _)| hit(self.menu_item_rect(*row)));
                if let Some((_, (_, (_, _, item, _)))) = chosen {
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
                self.minimise(app);
                return None;
            }
            if hit(self.title_rect(app)) {
                self.grab = Some((app, x - area.x, y - area.y));
                return None;
            }
            match app {
                App::Settings => self.click_settings(x, y),
                App::Files => self.click_files(x, y),
                App::Browser => self.click_browser(x, y),
                App::Notes => self.click_notes(x, y),
                App::Security => self.click_security(x, y),
                _ => {}
            }
            return None;
        }
        None
    }

    fn click_panel(&mut self, x: usize, y: usize) -> Option<Action> {
        let hit = |area: Rect| area.contains(x, y);
        if hit(self.logo_rect()) {
            self.toggle_menu();
            return None;
        }
        if hit(self.power_rect()) {
            self.power_menu = true;
            self.damage_all();
            return None;
        }
        for (index, app) in QUICK.into_iter().enumerate() {
            if hit(self.quick_rect(index)) {
                self.launch_window(app);
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
        for (index, app) in self.open_apps().into_iter().enumerate() {
            if hit(self.task_rect(index)) {
                if self.windows[app.index()].hidden {
                    self.launch_window(app);
                } else if self.front() == Some(app) {
                    self.minimise(app);
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
                self.update = Update::Idle;
                self.damage(self.windows[App::Settings.index()].area);
                return;
            }
        }
        if self.section == 5 {
            let (_, apply, clear) = self.account_rects();
            if apply.contains(x, y) {
                self.password = (!self.typed.is_empty()).then(|| self.typed.clone());
                self.typed.clear();
                self.damage_all();
            } else if clear.contains(x, y) {
                self.password = None;
                self.typed.clear();
                self.damage_all();
            }
            return;
        }
        if self.section == SECTIONS.len() - 1 && self.update_button_rect().contains(x, y) {
            self.update = Update::Checking(self.ms);
            self.damage(self.windows[App::Settings.index()].area);
        }
    }

    fn click_files(&mut self, x: usize, y: usize) {
        let bar = self.bar_rect(App::Files);
        if bar.contains(x, y) {
            for index in 0..3 {
                if self.bar_button(App::Files, index, self.bar_width(App::Files, index)).contains(x, y) {
                    match index {
                        0 => {
                            self.dir = fs::parent_of(&self.dir).to_string();
                            self.picked = None;
                        }
                        1 => {
                            let name = self.fs.free_name(&self.dir, "Nouveau dossier", "");
                            let path = fs::join(&self.dir, &name);
                            self.fs.make_dir(&path);
                        }
                        _ => {
                            if let Some(path) = self.picked.clone() {
                                self.fs.remove(&path);
                                self.picked = None;
                            }
                        }
                    }
                    self.damage(self.windows[App::Files.index()].area);
                    return;
                }
            }
            return;
        }
        let entries: Vec<(String, Kind)> =
            self.fs.list(&self.dir).iter().map(|entry| (entry.path.clone(), entry.kind)).collect();
        for (index, (path, kind)) in entries.into_iter().enumerate() {
            if !self.file_rect(index).contains(x, y) {
                continue;
            }
            let same = self.picked.as_deref() == Some(path.as_str());
            if same {
                // A second click opens: a directory, or a file in the notepad.
                match kind {
                    Kind::Dir => {
                        self.dir = path;
                        self.picked = None;
                    }
                    Kind::File => self.open_file(&path),
                }
            } else {
                self.picked = Some(path);
            }
            self.damage_all();
            return;
        }
    }

    fn open_file(&mut self, path: &str) {
        let text = self.fs.read(path).unwrap_or_default().to_string();
        self.notes = text.chars().collect();
        self.notes_file = Some(path.to_string());
        self.notes_note = format!("{} ouvert", fs::name_of(path));
        self.launch_window(App::Notes);
    }

    fn click_browser(&mut self, x: usize, y: usize) {
        let bar = self.bar_rect(App::Browser);
        let side = bar.h - 12;
        if Rect::new(bar.x + 8, bar.y + 6, side, side).contains(x, y) {
            if let Some(previous) = self.trail.pop() {
                self.page = previous.clone();
                self.address = previous;
                self.damage(self.windows[App::Browser.index()].area);
            }
            return;
        }
        if Rect::new(bar.x + 12 + side, bar.y + 6, side, side).contains(x, y) {
            self.go(web::HOME.to_string());
            return;
        }
        let address = self.address_rect();
        if Rect::new(address.right() + 8, address.y, address.h, address.h).contains(x, y) {
            let target = self.address.clone();
            self.go(target);
            return;
        }
        // A link on the page.
        let client = self.client_rect(App::Browser);
        let mut cursor = client.y + bar.h + 10;
        let width = client.w.saturating_sub(48) / font::width(Font::Body);
        if let Some(rest) = self.page.clone().strip_prefix("fichier:") {
            cursor += font::height(Font::Title) + 10;
            let entries: Vec<String> = self.fs.list(rest).iter().map(|entry| entry.path.clone()).collect();
            for path in entries {
                if Rect::new(client.x, cursor, client.w, self.line_h()).contains(x, y) {
                    self.go(format!("fichier:{path}"));
                    return;
                }
                cursor += self.line_h();
            }
            return;
        }
        let Some(found) = web::find(&self.page) else {
            return;
        };
        let targets: Vec<(usize, String)> = {
            let mut out = Vec::new();
            let mut y_at = cursor;
            for block in web::parse(found.body) {
                let next = block_height(&block, width, self.line_h());
                if let web::Block::Link(_, url) = block {
                    out.push((y_at, url.to_string()));
                }
                y_at += next;
            }
            out
        };
        for (top, url) in targets {
            if Rect::new(client.x, top, client.w, self.line_h()).contains(x, y) {
                self.go(url);
                return;
            }
        }
    }

    /// Goes to an address, remembering where it came from.
    fn go(&mut self, url: String) {
        if url == self.page {
            return;
        }
        self.trail.push(self.page.clone());
        self.fetched = None;
        if url.starts_with("http") {
            self.status = format!("chargement de {url}...");
            self.pending = Some(url.clone());
        } else {
            self.status = String::new();
        }
        self.page = url.clone();
        self.address = url;
        self.damage(self.windows[App::Browser.index()].area);
    }

    fn click_notes(&mut self, x: usize, y: usize) {
        if self.bar_button(App::Notes, 0, self.bar_width(App::Notes, 0)).contains(x, y) {
            self.save_note();
        } else if self.bar_button(App::Notes, 1, self.bar_width(App::Notes, 1)).contains(x, y) {
            self.notes.clear();
            self.notes_file = None;
            self.notes_note = "nouveau".to_string();
        } else {
            return;
        }
        self.damage(self.windows[App::Notes.index()].area);
    }

    fn click_security(&mut self, x: usize, y: usize) {
        for index in 0..2 {
            if self.security_button(index).contains(x, y) {
                // The kernel does the work: it owns the guard, and the files
                // are read by the same pass.
                self.sec_ask = Some((index == 0, index == 1));
                self.damage(self.windows[App::Security.index()].area);
                return;
            }
        }
    }

    fn save_note(&mut self) {
        let text: String = self.notes.iter().collect();
        let path = match self.notes_file.clone() {
            Some(path) => path,
            None => {
                let name = self.fs.free_name("/Documents", "note", ".txt");
                fs::join("/Documents", &name)
            }
        };
        if self.fs.write(&path, &text) {
            self.notes_file = Some(path.clone());
            self.notes_note = format!("enregistré · {} octets", text.len());
        } else {
            self.notes_note = "impossible d'enregistrer".to_string();
        }
    }

    fn toggle_menu(&mut self) {
        self.menu = !self.menu;
        self.menu_since = self.ms;
        self.filter.clear();
        self.choice = 0;
        self.damage_all();
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
                self.launch_window(app);
                None
            }
            Item::Lock => {
                self.lock();
                None
            }
            Item::Reboot => Some(Action::Reboot),
            Item::Off => self.shut_down(),
        }
    }

    /// Locks the screen. With no password set there is nothing to ask, so it
    /// says so rather than shutting the human out of their own machine.
    fn lock(&mut self) {
        if self.password.is_none() {
            self.launch_window(App::Settings);
            self.section = 5;
            self.damage_all();
            return;
        }
        self.locked = true;
        self.typed.clear();
        self.wrong = false;
        self.damage_all();
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
        if self.locked {
            match key {
                Key::Char(c) => self.typed.push(c),
                Key::Backspace => {
                    self.typed.pop();
                }
                Key::Enter => {
                    if self.password.as_deref() == Some(self.typed.as_str()) {
                        self.locked = false;
                        self.wrong = false;
                    } else {
                        self.wrong = true;
                    }
                    self.typed.clear();
                }
                _ => return None,
            }
            self.damage_all();
            return None;
        }
        if key == Key::Menu {
            self.toggle_menu();
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
                let next = APPS.into_iter().filter(|&app| self.showing(app)).find(|&app| app != front).unwrap_or(front);
                self.raise(next);
                self.damage_all();
            }
            return None;
        }
        match self.front() {
            Some(App::Terminal) => {
                let Desktop { shell, machine, fs, now, input, .. } = self;
                let mut context = Context { machine, fs, now: *now, input: *input };
                let action = shell.key(key, &mut context);
                self.damage(self.windows[App::Terminal.index()].area);
                match action {
                    Some(Action::PowerOff) => self.shut_down(),
                    Some(Action::Lock) => {
                        self.lock();
                        None
                    }
                    other => other,
                }
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
                self.notes_note.clear();
                self.damage(self.windows[App::Notes.index()].area);
                None
            }
            Some(App::Browser) => {
                match key {
                    Key::Char(c) => self.address.push(c),
                    Key::Backspace => {
                        self.address.pop();
                    }
                    Key::Enter => {
                        let target = self.address.clone();
                        self.go(target);
                    }
                    _ => return None,
                }
                self.damage(self.windows[App::Browser.index()].area);
                None
            }
            Some(App::Settings) => {
                match key {
                    Key::Up => self.section = self.section.saturating_sub(1),
                    Key::Down => self.section = (self.section + 1).min(SECTIONS.len() - 1),
                    Key::Char(c) if self.section == 5 => self.typed.push(c),
                    Key::Backspace if self.section == 5 => {
                        self.typed.pop();
                    }
                    Key::Enter if self.section == 5 => {
                        self.password = (!self.typed.is_empty()).then(|| self.typed.clone());
                        self.typed.clear();
                    }
                    Key::Enter if self.section == SECTIONS.len() - 1 => self.update = Update::Checking(self.ms),
                    _ => return None,
                }
                self.damage(self.windows[App::Settings.index()].area);
                None
            }
            Some(App::Files) => {
                if key == Key::Backspace {
                    self.dir = fs::parent_of(&self.dir).to_string();
                    self.picked = None;
                    self.damage(self.windows[App::Files.index()].area);
                }
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

/// Where a window sits when it opens, from the size of the screen alone: the
/// same layout holds from 640x480 to 4K.
fn default_rect(app: App, width: usize, height: usize, panel: usize) -> Rect {
    let top = panel + height / 24;
    let rect = match app {
        App::Welcome => Rect::new(width / 6, top + height / 6, width * 2 / 3, height * 2 / 5),
        App::Terminal => Rect::new(width / 40, top + height / 5, width * 5 / 9, height * 5 / 9),
        App::Files => Rect::new(width / 8, top + height / 10, width / 2, height / 2),
        App::Browser => Rect::new(width / 5, top, width * 3 / 5, height * 2 / 3),
        App::Notes => Rect::new(width / 4, top + height / 8, width * 2 / 5, height / 2),
        App::Settings => Rect::new(width / 6, top, width * 2 / 3, height * 2 / 3),
        App::About => Rect::new(width / 2, top, width * 4 / 9, height * 4 / 11),
        App::Security => Rect::new(width / 5, top + height / 12, width * 3 / 5, height * 3 / 5),
    };
    // Nothing may be wider than the screen, or too small to hold its title;
    // and the arithmetic must hold at 640x480 as well as at 4K.
    let widest = width.saturating_sub(8).max(120);
    let tallest = height.saturating_sub(panel + 8).max(120);
    let w = rect.w.min(widest).max(300.min(widest));
    let h = rect.h.min(tallest).max(180.min(tallest));
    Rect::new(rect.x.min(width.saturating_sub(w + 4)), rect.y.min(height.saturating_sub(h + 4)).max(panel + 4), w, h)
}

/// The y that centres a line of `style` in `area`.
fn centre_y(area: Rect, style: Font) -> usize {
    area.y + area.h.saturating_sub(font::height(style)) / 2
}

/// Seconds since boot, in words.
fn uptime(seconds: u32) -> String {
    match seconds {
        0..=59 => format!("{seconds} s"),
        60..=3599 => format!("{} min {:02} s", seconds / 60, seconds % 60),
        _ => format!("{} h {:02} min", seconds / 3600, seconds % 3600 / 60),
    }
}

/// Does `name` hold `filter`, ignoring case?
fn matches(name: &str, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let fold = |text: &str| text.chars().map(|c| c.to_ascii_lowercase()).collect::<String>();
    fold(name).contains(&fold(filter))
}

/// The terminal's prompt, coloured, ending where the command starts.
fn prompt(screen: &mut Screen, x: usize, y: usize, cwd: &str) -> usize {
    let x = screen.text(x, y, "root", RED, Font::Body);
    let x = screen.text(x, y, "@grenos", GREEN, Font::Body);
    let x = screen.text(x, y, ":", TEXT_FAINT, Font::Body);
    let x = screen.text(x, y, cwd, ACCENT, Font::Body);
    screen.text(x, y, "# ", TEXT_FAINT, Font::Body)
}

/// How tall a page block is, so a click can find the link it landed on.
fn block_height(block: &web::Block, width: usize, line: usize) -> usize {
    match block {
        web::Block::Title(_) => font::height(Font::Title) + 12,
        web::Block::Head(_) => font::height(Font::Head) + 10,
        web::Block::Rule => 14,
        web::Block::Space => line / 2,
        web::Block::Text(text) => wrap(text, width).len() * line,
        web::Block::Item(text) => wrap(text, width.saturating_sub(3)).len() * line,
        web::Block::Link(_, _) => line,
    }
}

/// Splits a paragraph into lines of at most `width` characters, at spaces. A
/// page wider than its window used to be cut off mid-sentence.
fn wrap(text: &str, width: usize) -> Vec<&str> {
    let width = width.max(8);
    let mut lines = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if rest.chars().count() <= width {
            lines.push(rest);
            break;
        }
        let end = rest.char_indices().nth(width).map_or(rest.len(), |(at, _)| at);
        let cut = rest[..end].rfind(' ').unwrap_or(end);
        let (line, tail) = rest.split_at(cut);
        lines.push(line);
        rest = tail.trim_start();
    }
    if lines.is_empty() {
        lines.push("");
    }
    lines
}

/// A row of filled circles: what a password field shows instead of the
/// letters. Drawn rather than typed, the font having no such character.
fn dots(screen: &mut Screen, x: usize, middle: usize, count: usize, colour: Rgb) -> usize {
    let size = 6;
    for index in 0..count.min(40) {
        let left = x + index * (size + 4);
        screen.round(Rect::new(left, middle - size / 2, size, size), size / 2, colour);
    }
    x + count.min(40) * (size + 4)
}

/// Draws one block of a page and returns the y of the next.
fn paint_block(screen: &mut Screen, x: usize, y: usize, width: usize, block: web::Block, line: usize) -> usize {
    let height = block_height(&block, width, line);
    match block {
        web::Block::Title(text) => {
            screen.text(x, y, text, TEXT, Font::Title);
        }
        web::Block::Head(text) => {
            screen.text(x, y, text, TEXT, Font::Head);
        }
        web::Block::Text(text) => {
            for (row, part) in wrap(text, width).into_iter().enumerate() {
                screen.text(x, y + row * line, part, TEXT_DIM, Font::Body);
            }
        }
        web::Block::Item(text) => {
            screen.fill(Rect::new(x + 3, y + font::height(Font::Body) / 2, 3, 3), ACCENT);
            for (row, part) in wrap(text, width.saturating_sub(3)).into_iter().enumerate() {
                screen.text(x + 14, y + row * line, part, TEXT_DIM, Font::Body);
            }
        }
        web::Block::Link(text, _) => {
            let end = screen.text(x, y, text, ACCENT, Font::Body);
            screen.fill(Rect::new(x, y + font::height(Font::Body) - 2, end - x, 1), ACCENT);
        }
        web::Block::Rule => {
            screen.fill(Rect::new(x, y + 6, width * font::width(Font::Body), 1), LINE);
        }
        web::Block::Space => {}
    }
    y + height
}
