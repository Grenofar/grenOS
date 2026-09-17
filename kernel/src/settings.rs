//! What grenOS keeps from one start to the next: the chosen wallpaper, the
//! password, how many times this disk has booted. One `key=value` a line, in
//! `/grenos/reglages.txt` on our own partition.
//!
//! Pure on purpose: reading and writing the file is `storage.rs`, which needs
//! the disk; everything here is text, so the host and the boot self-test can
//! check it.

use alloc::format;
use alloc::string::String;

/// Where the file lives on our partition.
pub const PATH: &str = "/grenos/reglages.txt";
/// What the file may grow to: settings, not documents.
pub const LIMIT: usize = 8 * 1024;

/// The value of `key`, or None. Spaces around the parts do not count.
pub fn get<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .find(|(name, _)| name.trim() == key)
        .map(|(_, value)| value.trim())
}

/// The value of `key` as a number, or None when it is missing or not one.
pub fn number(text: &str, key: &str) -> Option<u32> {
    get(text, key)?.parse().ok()
}

/// `text` with `key` set to `value`: the line replaced where it is, or added
/// at the end. A value never carries a line break.
pub fn set(text: &str, key: &str, value: &str) -> String {
    let value = value.replace(['\n', '\r'], " ");
    let mut out = String::with_capacity(text.len() + key.len() + value.len() + 2);
    let mut replaced = false;
    for line in text.lines() {
        let this = line.split_once('=').is_some_and(|(name, _)| name.trim() == key);
        if this && replaced {
            continue;
        }
        if this {
            out.push_str(&format!("{key}={value}\n"));
            replaced = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !replaced {
        out.push_str(&format!("{key}={value}\n"));
    }
    out
}

/// Checks the pair on fixed inputs: `Ok`, or the case that failed.
pub fn self_test() -> Result<(), &'static str> {
    let first = set("", "fond", "2");
    if first != "fond=2\n" {
        return Err("adding to an empty file");
    }
    let second = set(&first, "demarrages", "1");
    if second != "fond=2\ndemarrages=1\n" {
        return Err("adding a second line");
    }
    let third = set(&second, "fond", "5");
    if third != "fond=5\ndemarrages=1\n" {
        return Err("replacing a value in place");
    }
    if get(&third, "fond") != Some("5") || number(&third, "demarrages") != Some(1) {
        return Err("reading a value back");
    }
    if get(&third, "absent").is_some() || number("demarrages=x", "demarrages").is_some() {
        return Err("a missing or unreadable value");
    }
    let messy = " fond = 3 \n# une note\nsans_egal\ndemarrages=7\n";
    if get(messy, "fond") != Some("3") || number(messy, "demarrages") != Some(7) {
        return Err("spaces and lines that are not settings");
    }
    let kept = set(messy, "fond", "4");
    if !kept.contains("# une note\n") || !kept.contains("sans_egal\n") || get(&kept, "fond") != Some("4") {
        return Err("keeping the lines around a changed one");
    }
    if get(&set("fond=1\nfond=2\n", "fond", "9"), "fond") != Some("9") {
        return Err("a key written twice");
    }
    if set("", "note", "deux\nlignes") != "note=deux lignes\n" {
        return Err("a value with a line break in it");
    }
    Ok(())
}
