//! The update check: which image is the newest one published, read out of the
//! index the release workflow keeps in storage, and whether this machine is
//! running it.
//!
//! It reads the same `index.json` the download page reads, over https, with
//! the kernel's own TLS. It does not install anything: without a disk driver
//! there is nowhere to put a new image, and the window says so.

use alloc::format;
use alloc::string::{String, ToString};

/// Where the index lives. The release workflow writes it; /download reads it.
pub const INDEX: &str = "https://tpqzhzuoyqpfairatdrw.supabase.co/storage/v1/object/public/releases/index.json";

/// The newest published image.
#[derive(Clone, PartialEq, Eq)]
pub struct Latest {
    pub build: String,
    pub commit: String,
    pub published: String,
    pub size: u64,
}

impl Latest {
    /// `2026-09-12 11:00`, from the workflow's `2026-09-12T11:00:18Z`.
    pub fn when(&self) -> String {
        let date = self.published.get(..10).unwrap_or("");
        let time = self.published.get(11..16).unwrap_or("");
        format!("{date} {time}")
    }
}

/// The first entry of the index, which the workflow keeps newest first. The
/// index is our own, with a known flat shape, so a scan for the few fields
/// needed is enough; a JSON parser would be a lot of kernel for four values.
pub fn latest(body: &[u8]) -> Option<Latest> {
    let text = core::str::from_utf8(body).ok()?;
    let entry = &text[text.find('{')?..];
    let entry = &entry[..entry.find('}')?];
    Some(Latest {
        build: string_field(entry, "build")?,
        commit: string_field(entry, "commit").unwrap_or_default(),
        published: string_field(entry, "published_at").unwrap_or_default(),
        size: number_field(entry, "size").unwrap_or(0),
    })
}

/// The value of `"name": "..."`. The quotes around the name matter: they keep
/// `"size"` from matching inside `"virtualbox_size"`.
fn string_field(entry: &str, name: &str) -> Option<String> {
    let key = format!("\"{name}\"");
    let after = &entry[entry.find(&key)? + key.len()..];
    let after = after.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    Some(after[..after.find('"')?].to_string())
}

fn number_field(entry: &str, name: &str) -> Option<u64> {
    let key = format!("\"{name}\"");
    let after = &entry[entry.find(&key)? + key.len()..];
    let after = after.trim_start().strip_prefix(':')?.trim_start();
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Whether `running` — the commit this kernel was built from, or "local" —
/// is the newest. None when it cannot be told: a local build has no commit.
pub fn is_current(latest: &Latest, running: &str) -> Option<bool> {
    if running.is_empty() || running == "local" {
        return None;
    }
    Some(latest.commit.starts_with(running))
}

/// Whether `running` is any of the images the index lists. Not being the
/// newest is only "behind" when it is: a build the CI made from a branch was
/// never published, and must not be told a newer one exists.
pub fn is_listed(body: &[u8], running: &str) -> bool {
    let Ok(mut text) = core::str::from_utf8(body) else {
        return false;
    };
    let key = "\"commit\"";
    while let Some(at) = text.find(key) {
        if string_field(text, "commit").is_some_and(|commit| commit.starts_with(running)) {
            return true;
        }
        text = &text[at + key.len()..];
    }
    false
}
