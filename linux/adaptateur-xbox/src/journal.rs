// grenos-adaptateur-xbox — le pilote de l'adaptateur sans fil Xbox.
// Copyright (C) 2019 Medusalix (xow, dont ce programme est la réécriture)
// Copyright (C) 2026 grenOS
//
// Ce programme est un logiciel libre ; vous pouvez le redistribuer ou le
// modifier selon les termes de la GNU General Public License, version 2 ou
// (à votre choix) toute version ultérieure, publiée par la Free Software
// Foundation. Il est distribué SANS AUCUNE GARANTIE. Voir le fichier LICENSE.

//! Le journal : la sortie d'erreur, que journald range sous le nom du service.
//! `GRENOS_XBOX_DETAILS=1` ajoute les détails (registres, canaux, vibrations).

use std::sync::OnceLock;

fn details_voulus() -> bool {
    static D: OnceLock<bool> = OnceLock::new();
    *D.get_or_init(|| std::env::var_os("GRENOS_XBOX_DETAILS").is_some())
}

// JAMAIS eprintln! : il plante si la sortie d'erreur est fermée. Vu le
// 25 novembre : Ctrl+C a tué le `tee` qui recopiait le journal, et le pilote
// est mort en écrivant « arrêt… » — sans éteindre la manette ni la radio.
// Un journal qu'on ne peut plus écrire ne doit rien arrêter.
fn ecrire(texte: std::fmt::Arguments) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "{texte}");
}

pub fn info(texte: std::fmt::Arguments) {
    ecrire(texte);
}

pub fn erreur(texte: std::fmt::Arguments) {
    ecrire(format_args!("erreur : {texte}"));
}

pub fn details(texte: std::fmt::Arguments) {
    if details_voulus() {
        ecrire(format_args!("détail : {texte}"));
    }
}

#[macro_export]
macro_rules! info {
    ($($t:tt)*) => { $crate::journal::info(format_args!($($t)*)) };
}

#[macro_export]
macro_rules! erreur {
    ($($t:tt)*) => { $crate::journal::erreur(format_args!($($t)*)) };
}

#[macro_export]
macro_rules! details {
    ($($t:tt)*) => { $crate::journal::details(format_args!($($t)*)) };
}

/// « 62:45:bd:12:34:56 »
pub fn adresse(octets: &[u8]) -> String {
    octets.iter().map(|o| format!("{o:02x}")).collect::<Vec<_>>().join(":")
}
