//! The update check: which image is the newest one published, read out of the
//! index the release workflow keeps in storage, and whether this machine is
//! running it.
//!
//! It reads the same `index.json` the download page reads, over https, with
//! the kernel's own TLS. What it offers to install is trusted for one reason
//! only: the manifest of the image is signed by the release workflow's key,
//! whose public half is below (docs/specs/disk-and-updates.md §6), and the
//! kernel file matches the manifest's size and SHA-256.

use alloc::format;
use alloc::string::{String, ToString};

/// Where the published files live. The release workflow writes them; /download reads them.
pub const BASE: &str = "https://tpqzhzuoyqpfairatdrw.supabase.co/storage/v1/object/public/releases/";

/// The index of the published images, newest first.
pub const INDEX: &str = "https://tpqzhzuoyqpfairatdrw.supabase.co/storage/v1/object/public/releases/index.json";

/// The release workflow's Ed25519 public key: the only key an update is
/// accepted from. Its private half lives in a private storage bucket and never
/// enters the repository.
pub const PUBLIC_KEY: [u8; 32] = [
    0x4d, 0x2a, 0x2d, 0x34, 0x91, 0x05, 0x7d, 0x2a, 0x64, 0xab, 0x69, 0x59, 0x43, 0x5f, 0xb6, 0x1e, 0x28, 0x8f, 0x5c, 0x2a,
    0xa7, 0xe7, 0xe9, 0x9a, 0xfd, 0xc9, 0x09, 0xd3, 0x48, 0x73, 0xe9, 0x14,
];

/// The largest kernel a slot holds (make-disk.sh pads each to 8 MiB).
pub const SLOT_BYTES: u64 = 8 * 1024 * 1024;

/// The newest published image.
#[derive(Clone, PartialEq, Eq)]
pub struct Latest {
    pub build: String,
    pub commit: String,
    pub published: String,
    pub size: u64,
    /// Paths of the signed manifest and its signature, when the image has them.
    pub manifest: Option<String>,
    pub signature: Option<String>,
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
        manifest: string_field(entry, "manifest"),
        signature: string_field(entry, "signature"),
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

/// What a signed manifest says about a published kernel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub build: String,
    pub commit: String,
    pub kernel: String,
    pub size: u64,
    pub sha256: [u8; 32],
}

/// Reads a manifest, strictly: every line present, in order, with nothing
/// before or after, as release.yml writes it. Pure.
pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest, &'static str> {
    let text = core::str::from_utf8(bytes).map_err(|_| "the manifest is not text")?;
    let mut lines = text.split('\n');
    let mut field = |name: &str| -> Result<String, &'static str> {
        let line = lines.next().ok_or("the manifest is cut short")?;
        let (key, value) = line.split_once(' ').ok_or("a manifest line has no value")?;
        if key != name || value.is_empty() {
            return Err("the manifest's lines are not the expected ones");
        }
        Ok(value.to_string())
    };
    if field("grenos-update")? != "1" {
        return Err("unknown manifest version");
    }
    let build = field("build")?;
    let commit = field("commit")?;
    let kernel = field("kernel")?;
    let size: u64 = field("size")?.parse().map_err(|_| "the manifest's size is not a number")?;
    let digest = field("sha256")?;
    if lines.next() != Some("") || lines.next().is_some() {
        return Err("the manifest has more than it should");
    }
    if stamp(&build).is_none() || commit.len() != 40 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("the manifest's build or commit is malformed");
    }
    if kernel.contains("..") || !kernel.starts_with("builds/") {
        return Err("the manifest's kernel path is not a published build");
    }
    if size == 0 || size > SLOT_BYTES {
        return Err("the kernel does not fit a slot");
    }
    let mut sha256 = [0u8; 32];
    if digest.len() != 64 {
        return Err("the manifest's sha256 is malformed");
    }
    for (byte, pair) in sha256.iter_mut().zip(digest.as_bytes().chunks_exact(2)) {
        let high = (pair[0] as char).to_digit(16).ok_or("the manifest's sha256 is malformed")?;
        let low = (pair[1] as char).to_digit(16).ok_or("the manifest's sha256 is malformed")?;
        *byte = (high * 16 + low) as u8;
    }
    Ok(Manifest { build, commit, kernel, size, sha256 })
}

/// The `YYYYMMDD-HHMM` at the start of a build name, when it is one. Pure.
pub fn stamp(build: &str) -> Option<&str> {
    let head = build.get(..13)?;
    let shape = head.bytes().enumerate().all(|(i, b)| if i == 8 { b == b'-' } else { b.is_ascii_digit() });
    shape.then_some(head)
}

/// Whether `candidate` is strictly newer than the running kernel's stamp. The
/// format sorts as text. An update is never allowed to go back in time: a
/// signed but older image is still an older image. Pure.
pub fn is_newer(candidate: &str, running: &str) -> bool {
    match (stamp(candidate), stamp(running)) {
        (Some(new), Some(old)) => new > old,
        _ => false,
    }
}
