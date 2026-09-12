//! The terminal's shell: a prompt, a working directory, and commands that
//! reach the real machine — the memory it found, the devices on its bus, the
//! files it holds.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::desktop::{Action, Input, Machine};
use crate::fs::{self, Fs};
use crate::keyboard::Key;
use crate::time::DateTime;

/// What the commands are allowed to look at, and to change.
pub struct Context<'a> {
    pub machine: &'a Machine,
    pub fs: &'a mut Fs,
    pub now: DateTime,
    pub input: Input,
}

#[derive(Clone)]
pub enum Line {
    /// A command the human typed, shown after the prompt, with the directory
    /// it was typed in.
    Command(String, String),
    /// What came back.
    Output(String),
}

pub struct Shell {
    lines: Vec<Line>,
    input: String,
    cwd: String,
    /// What was typed before, for the up arrow.
    history: Vec<String>,
    recall: usize,
}

/// Lines kept; older ones scroll away for good.
const HISTORY: usize = 500;
const INPUT_CAP: usize = 160;

const HELP: [&str; 18] = [
    "Commandes :",
    "  aide        cette liste",
    "  ls [dossier]   ce que contient un dossier",
    "  cd <dossier>   changer de dossier",
    "  cat <fichier>  afficher un fichier",
    "  ecrire <fichier> <texte>   écrire un fichier",
    "  mkdir <dossier>   créer un dossier",
    "  rm <chemin>    effacer",
    "  uname       le système et sa version",
    "  date        la date et l'heure du PC",
    "  mem         la mémoire et le tas du noyau",
    "  lspci       les périphériques PCI",
    "  souris      ce que la souris et le clavier ont envoyé",
    "  dmesg       le journal du démarrage",
    "  grenfetch   le résumé de la machine",
    "  maj         la version installée",
    "  verrouiller / redemarrer / eteindre",
    "  clear       efface l'écran",
];

const MARK: [&str; 5] = [
    "   __ _ _ __ ___ _ __   ___  ___",
    "  / _` | '__/ _ \\ '_ \\ / _ \\/ __|",
    " | (_| | | |  __/ | | | (_) \\__ \\",
    "  \\__, |_|  \\___|_| |_|\\___/|___/",
    "  |___/",
];

impl Shell {
    pub fn new() -> Self {
        Shell {
            lines: alloc::vec![Line::Output("grenOS · tapez aide pour la liste des commandes.".to_string())],
            input: String::new(),
            cwd: "/".to_string(),
            history: Vec::new(),
            recall: 0,
        }
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    pub fn key(&mut self, key: Key, context: &mut Context) -> Option<Action> {
        match key {
            Key::Char(c) if self.input.chars().count() < INPUT_CAP => self.input.push(c),
            Key::Backspace => {
                self.input.pop();
            }
            Key::Up if self.recall > 0 => {
                self.recall -= 1;
                self.input = self.history.get(self.recall).cloned().unwrap_or_default();
            }
            Key::Down => {
                self.recall = (self.recall + 1).min(self.history.len());
                self.input = self.history.get(self.recall).cloned().unwrap_or_default();
            }
            Key::Enter => return self.run(context),
            _ => {}
        }
        None
    }

    fn say(&mut self, text: &str) {
        self.lines.push(Line::Output(text.to_string()));
    }

    /// An argument turned into an absolute path, from the working directory.
    fn resolve(&self, argument: &str) -> String {
        match argument {
            "" | "." => self.cwd.clone(),
            ".." => fs::parent_of(&self.cwd).to_string(),
            path if path.starts_with('/') => path.trim_end_matches('/').to_string(),
            path => fs::join(&self.cwd, path.trim_end_matches('/')),
        }
    }

    fn run(&mut self, context: &mut Context) -> Option<Action> {
        let command = core::mem::take(&mut self.input);
        self.lines.push(Line::Command(self.cwd.clone(), command.clone()));
        if !command.trim().is_empty() {
            self.history.push(command.clone());
        }
        self.recall = self.history.len();
        let mut words = command.split_whitespace();
        let mut action = None;
        match words.next() {
            None => {}
            Some("aide" | "help") => {
                for line in HELP {
                    self.say(line);
                }
            }
            Some("clear" | "effacer") => self.lines.clear(),
            Some("ls" | "dir") => {
                let path = self.resolve(words.next().unwrap_or(""));
                let path = if path.is_empty() { "/".to_string() } else { path };
                if context.fs.get(&path).is_none() {
                    self.say(&format!("{path} : introuvable"));
                } else {
                    let listing: Vec<String> = context
                        .fs
                        .list(&path)
                        .iter()
                        .map(|entry| match entry.kind {
                            fs::Kind::Dir => format!("  {}/", entry.name()),
                            fs::Kind::File => format!("  {}   {} octets", entry.name(), entry.size()),
                        })
                        .collect();
                    if listing.is_empty() {
                        self.say("  (vide)");
                    }
                    for line in listing {
                        self.say(&line);
                    }
                }
            }
            Some("cd") => {
                let path = self.resolve(words.next().unwrap_or("/"));
                let path = if path.is_empty() { "/".to_string() } else { path };
                match context.fs.get(&path).map(|entry| entry.kind) {
                    Some(fs::Kind::Dir) => self.cwd = path,
                    Some(fs::Kind::File) => self.say(&format!("{path} : ce n'est pas un dossier")),
                    None => self.say(&format!("{path} : introuvable")),
                }
            }
            Some("cat") => {
                let path = self.resolve(words.next().unwrap_or(""));
                match context.fs.read(&path) {
                    Some(text) => {
                        let lines: Vec<String> = text.lines().map(ToString::to_string).collect();
                        for line in lines {
                            self.say(&line);
                        }
                    }
                    None => self.say(&format!("{path} : introuvable")),
                }
            }
            Some("ecrire" | "write") => {
                let name = words.next().unwrap_or("");
                let rest: Vec<&str> = words.collect();
                if name.is_empty() {
                    self.say("usage : ecrire <fichier> <texte>");
                } else {
                    let path = self.resolve(name);
                    if context.fs.write(&path, &rest.join(" ")) {
                        self.say(&format!("{path} : {} octets écrits", rest.join(" ").len()));
                    } else {
                        self.say(&format!("{path} : c'est un dossier"));
                    }
                }
            }
            Some("mkdir") => {
                let path = self.resolve(words.next().unwrap_or(""));
                if path.is_empty() || path == "/" {
                    self.say("usage : mkdir <dossier>");
                } else {
                    context.fs.make_dir(&path);
                    self.say(&format!("{path} : créé"));
                }
            }
            Some("rm" | "effacer-fichier") => {
                let path = self.resolve(words.next().unwrap_or(""));
                if context.fs.remove(&path) {
                    self.say(&format!("{path} : effacé"));
                } else {
                    self.say(&format!("{path} : impossible (absent, protégé, ou dossier non vide)"));
                }
            }
            Some("uname") => {
                let machine = context.machine;
                self.say(&format!("grenOS {} x86_64 64 bits (build {})", machine.version, machine.build));
            }
            Some("date") => {
                let now = context.now;
                self.say(&format!(
                    "{} {} {} {} {:02}:{:02}:{:02}",
                    now.day_name(),
                    now.day,
                    now.month_name(),
                    now.year,
                    now.hour,
                    now.minute,
                    now.second
                ));
            }
            Some("whoami") => self.say("root"),
            Some("dmesg") => {
                let log = context.machine.log.clone();
                for line in log {
                    self.say(&line);
                }
            }
            Some("mem" | "free") => {
                let machine = context.machine;
                self.say(&format!("Mémoire : {} Mo au total, {} Mo libres", machine.memory.0, machine.memory.1));
                self.say(&format!("Tas du noyau : {} Ko sur {} Ko", machine.heap.0 / 1024, machine.heap.1 / 1024));
                self.say(&format!("Fichiers : {} pour {} octets", context.fs.files(), context.fs.used()));
            }
            Some("lspci") => {
                let devices = context.machine.devices.clone();
                if devices.is_empty() {
                    self.say("Aucun périphérique PCI.");
                }
                for line in devices {
                    self.say(&line);
                }
            }
            Some("souris" | "input") => {
                let input = context.input;
                self.say(&format!("Souris : {} octets, {} paquets, IRQ12 {}", input.mouse_bytes, input.packets, input.mouse_irq));
                self.say(&format!("Clavier : {} octets, {} touches, IRQ1 {}", input.key_bytes, input.keys, input.key_irq));
                self.say(&format!("Ramassés par le minuteur : {}, perdus : {}", input.swept, input.lost));
            }
            Some("maj" | "update") => {
                let machine = context.machine;
                self.say(&format!("grenOS {} (build {}, compilée le {})", machine.version, machine.build, machine.built_at));
                self.say("Vérification en ligne : impossible, aucune carte réseau configurée.");
                self.say("La dernière image est sur grenos-dev.vercel.app/download");
            }
            Some("grenfetch" | "neofetch") => {
                for line in MARK {
                    self.say(line);
                }
                let machine = context.machine;
                self.say(&format!("  grenOS {} (build {})", machine.version, machine.build));
                for line in machine.lines() {
                    self.say(&format!("  {line}"));
                }
            }
            Some("echo") => {
                let rest: Vec<&str> = words.collect();
                self.say(&rest.join(" "));
            }
            Some("verrouiller" | "lock") => action = Some(Action::Lock),
            Some("redemarrer" | "reboot") => {
                self.say("Redémarrage...");
                action = Some(Action::Reboot);
            }
            Some("eteindre" | "poweroff" | "halt") => {
                self.say("Arrêt...");
                action = Some(Action::PowerOff);
            }
            Some(other) => self.say(&format!("{other} : commande introuvable (tapez aide)")),
        }
        if self.lines.len() > HISTORY {
            let extra = self.lines.len() - HISTORY;
            self.lines.drain(..extra);
        }
        action
    }
}
