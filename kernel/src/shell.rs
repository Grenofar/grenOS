//! The terminal's shell: a prompt, a handful of commands, and the kernel's
//! boot log. Small on purpose, and meant to grow with the kernel.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::desktop::{Action, Input, Machine};
use crate::keyboard::Key;
use crate::time::DateTime;

/// What the commands are allowed to look at.
pub struct Context<'a> {
    pub machine: &'a Machine,
    pub now: DateTime,
    pub input: Input,
}

#[derive(Clone)]
pub enum Line {
    /// A command the human typed, shown after the prompt.
    Command(String),
    /// What came back.
    Output(String),
}

pub struct Shell {
    lines: Vec<Line>,
    input: String,
}

/// Lines kept; older ones scroll away for good.
const HISTORY: usize = 500;
const INPUT_CAP: usize = 120;

const HELP: [&str; 14] = [
    "Commandes :",
    "  aide        cette liste",
    "  uname       le système et sa version",
    "  date        la date et l'heure du PC",
    "  mem         la mémoire et le tas du noyau",
    "  lspci       les périphériques PCI",
    "  souris      ce que la souris et le clavier ont envoyé",
    "  dmesg       le journal du démarrage",
    "  grenfetch   le résumé de la machine",
    "  maj         la version installée",
    "  echo        répète ce qu'on lui donne",
    "  ls          les fichiers (il faut d'abord un disque)",
    "  clear       efface l'écran",
    "  redemarrer / eteindre",
];

const MARK: [&str; 5] = [
    "   __ _ _ __ ___ _ __   ___  ___",
    "  / _` | '__/ _ \\ '_ \\ / _ \\/ __|",
    " | (_| | | |  __/ | | | (_) \\__ \\",
    "  \\__, |_|  \\___|_| |_|\\___/|___/",
    "  |___/",
];

impl Shell {
    pub fn new(log: &[String]) -> Self {
        let mut lines = Vec::new();
        lines.push(Line::Command("dmesg".to_string()));
        lines.extend(log.iter().cloned().map(Line::Output));
        lines.push(Line::Output(String::new()));
        lines.push(Line::Output("Tapez aide pour la liste des commandes.".to_string()));
        Shell { lines, input: String::new() }
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn key(&mut self, key: Key, context: &Context) -> Option<Action> {
        match key {
            Key::Char(c) if self.input.chars().count() < INPUT_CAP => self.input.push(c),
            Key::Backspace => {
                self.input.pop();
            }
            Key::Enter => return self.run(context),
            _ => {}
        }
        None
    }

    fn say(&mut self, text: &str) {
        self.lines.push(Line::Output(text.to_string()));
    }

    fn run(&mut self, context: &Context) -> Option<Action> {
        let command = core::mem::take(&mut self.input);
        self.lines.push(Line::Command(command.clone()));
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
            Some("uname") => {
                let machine = context.machine;
                self.say(&format!("grenOS {} x86_64 (build {})", machine.version, machine.build));
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
                for line in &context.machine.log {
                    self.lines.push(Line::Output(line.clone()));
                }
            }
            Some("mem" | "free") => {
                let machine = context.machine;
                self.say(&format!("Mémoire : {} Mo au total, {} Mo libres", machine.memory.0, machine.memory.1));
                self.say(&format!("Tas du noyau : {} Ko sur {} Ko", machine.heap.0 / 1024, machine.heap.1 / 1024));
            }
            Some("lspci") => {
                if context.machine.devices.is_empty() {
                    self.say("Aucun périphérique PCI.");
                }
                for line in &context.machine.devices {
                    self.lines.push(Line::Output(line.clone()));
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
                self.say("Pas encore de réseau : la mise à jour se télécharge sur grenos-dev.vercel.app/download");
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
            Some("ls" | "dir") => self.say("Aucun disque monté : le pilote SATA et le système de fichiers arrivent."),
            Some("redemarrer" | "reboot") => {
                self.say("Redémarrage…");
                action = Some(Action::Reboot);
            }
            Some("eteindre" | "poweroff" | "halt") => {
                self.say("Arrêt…");
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
