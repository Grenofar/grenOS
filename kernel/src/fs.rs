//! A file system in memory: what the file explorer browses and what the
//! notepad saves into.
//!
//! It lives on the kernel heap and it dies with the machine — there is no disk
//! driver yet, and pretending otherwise would lose someone's text without
//! telling them. Every window that writes here says so. When the SATA driver
//! lands, this is the interface it has to fill.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Dir,
    File,
}

#[derive(Clone)]
pub struct Entry {
    /// Absolute, with no trailing slash: `/`, `/Documents`, `/Documents/a.txt`.
    pub path: String,
    pub kind: Kind,
    pub text: String,
    /// True for what the kernel put there: the explorer refuses to delete it.
    pub system: bool,
}

impl Entry {
    /// The last part of the path, which is what a window shows.
    pub fn name(&self) -> &str {
        name_of(&self.path)
    }

    pub fn size(&self) -> usize {
        self.text.len()
    }
}

pub struct Fs {
    entries: Vec<Entry>,
}

/// The directory holding `path`, or `/`.
pub fn parent_of(path: &str) -> &str {
    match path.rfind('/') {
        Some(0) | None => "/",
        Some(at) => &path[..at],
    }
}

pub fn name_of(path: &str) -> &str {
    match path.rfind('/') {
        Some(at) if at + 1 < path.len() => &path[at + 1..],
        _ => "/",
    }
}

/// `dir` and `name` joined, with exactly one slash between them.
pub fn join(dir: &str, name: &str) -> String {
    if dir == "/" { format!("/{name}") } else { format!("{dir}/{name}") }
}

impl Fs {
    /// An empty tree with the three directories the desktop expects.
    pub fn new() -> Self {
        let mut fs = Fs { entries: Vec::new() };
        for dir in ["/", "/Documents", "/Système"] {
            fs.entries.push(Entry { path: dir.to_string(), kind: Kind::Dir, text: String::new(), system: true });
        }
        fs
    }

    /// Adds a file the kernel owns.
    pub fn add_system(&mut self, path: &str, text: &str) {
        self.put(path, text, true);
    }

    /// Writes a file, making it if it is not there. Returns false when the
    /// path names a directory.
    pub fn write(&mut self, path: &str, text: &str) -> bool {
        if self.entries.iter().any(|e| e.path == path && e.kind == Kind::Dir) {
            return false;
        }
        self.put(path, text, false);
        true
    }

    fn put(&mut self, path: &str, text: &str, system: bool) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.path == path) {
            entry.text = text.to_string();
            return;
        }
        self.make_dir(parent_of(path));
        self.entries.push(Entry {
            path: path.to_string(),
            kind: Kind::File,
            text: text.to_string(),
            system,
        });
    }

    /// Makes a directory, and the ones above it, if they are missing.
    pub fn make_dir(&mut self, path: &str) {
        if path == "/" || self.entries.iter().any(|e| e.path == path) {
            return;
        }
        self.make_dir(parent_of(path));
        self.entries.push(Entry { path: path.to_string(), kind: Kind::Dir, text: String::new(), system: false });
    }

    pub fn read(&self, path: &str) -> Option<&str> {
        self.entries.iter().find(|e| e.path == path && e.kind == Kind::File).map(|e| e.text.as_str())
    }

    pub fn get(&self, path: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.path == path)
    }

    pub fn exists(&self, path: &str) -> bool {
        self.entries.iter().any(|e| e.path == path)
    }

    /// What is directly inside `dir`: directories first, then files, each in
    /// alphabetical order.
    pub fn list(&self, dir: &str) -> Vec<&Entry> {
        let mut out: Vec<&Entry> =
            self.entries.iter().filter(|e| e.path != "/" && parent_of(&e.path) == dir).collect();
        out.sort_by(|a, b| match (a.kind, b.kind) {
            (Kind::Dir, Kind::File) => core::cmp::Ordering::Less,
            (Kind::File, Kind::Dir) => core::cmp::Ordering::Greater,
            _ => a.path.cmp(&b.path),
        });
        out
    }

    /// Removes a file, or an empty directory the kernel does not own.
    pub fn remove(&mut self, path: &str) -> bool {
        let Some(at) = self.entries.iter().position(|e| e.path == path) else {
            return false;
        };
        if self.entries[at].system || (self.entries[at].kind == Kind::Dir && !self.list(path).is_empty()) {
            return false;
        }
        self.entries.remove(at);
        true
    }

    /// A name nothing else in `dir` uses, from `stem` plus a number.
    pub fn free_name(&self, dir: &str, stem: &str, extension: &str) -> String {
        for number in 1..1000 {
            let name = if number == 1 { format!("{stem}{extension}") } else { format!("{stem} {number}{extension}") };
            if !self.exists(&join(dir, &name)) {
                return name;
            }
        }
        format!("{stem} {extension}")
    }

    pub fn files(&self) -> usize {
        self.entries.iter().filter(|e| e.kind == Kind::File).count()
    }

    /// Bytes of text held, which is all this file system costs.
    pub fn used(&self) -> usize {
        self.entries.iter().map(|e| e.text.len()).sum()
    }
}
