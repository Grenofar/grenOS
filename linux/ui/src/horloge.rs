//! Horloge : l'heure ici et ailleurs, un minuteur, un chronomètre (todo G33,
//! G34 — 18 novembre).
//!
//! Tout le compte se fait dans la page. Rust ne sert qu'à une chose : prévenir
//! quand le minuteur arrive au bout, par une vraie notification (dunst,
//! notify-send) — visible même si la fenêtre est cachée derrière une autre.

use crate::systeme::{detacher, dire};

#[tauri::command]
pub fn horloge_sonner(texte: String) -> Result<(), String> {
    let texte: String = texte.chars().filter(|c| !c.is_control()).take(120).collect();
    dire("horloge : minuteur termine");
    detacher("notify-send", &["--app-name=Horloge", "--urgency=critical", "--icon=grenos-horloge", "Minuteur", &texte])
}
