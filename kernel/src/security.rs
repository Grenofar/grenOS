//! Protection: what the processor is made to enforce, what the kernel checks
//! about itself, and what it looks for in the files it holds.
//!
//! Three honest things, and no theatre:
//!
//! - the processor's own defences are turned on and then *read back* — no
//!   execution of data (NX), no writing to read-only pages even from the
//!   kernel (CR0.WP), and SMEP and SMAP where the chip has them;
//! - the kernel's own code is measured at boot and can be measured again at
//!   any moment: a different sum means something rewrote it;
//! - the files are scanned for known-bad content, and what matches is moved
//!   to a quarantine folder where nothing will open it.
//!
//! The test signature used to prove the scanner works is the standard EICAR
//! string — kept **encoded**, never in the clear. A kernel image carrying it
//! verbatim would be quarantined by the antivirus of the machine downloading
//! the ISO, which is a fine way to lose a release.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::fs::{self, Fs, Kind};
use crate::memory::Frames;
use crate::paging;

unsafe extern "C" {
    static __text_start: u8;
    static __text_end: u8;
    static __rodata_start: u8;
    static __data_start: u8;
    static __kernel_end: u8;
}

/// Page table bits worth reporting.
const WRITABLE: u64 = 1 << 1;
const NO_EXECUTE: u64 = 1 << 63;

/// Where a file goes when it matches.
pub const QUARANTINE: &str = "/Système/Quarantaine";

/// One thing worth refusing, with the name shown to the human. The pattern is
/// stored scrambled: see the note at the top of this file.
struct Signature {
    name: &'static str,
    /// The pattern, each byte exclusive-ored with KEY.
    coded: &'static [u8],
}

const KEY: u8 = 0x5A;

/// The EICAR anti-malware test file, and two shapes of obvious mischief. The
/// first is the industry's own harmless test string: if the scanner does not
/// catch it, the scanner does not work.
const SIGNATURES: [Signature; 3] = [
    Signature {
        name: "EICAR-Test-File",
        // X5O!P%@AP[4\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*
        coded: &[
            0x02, 0x6f, 0x15, 0x7b, 0x0a, 0x7f, 0x1a, 0x1b, 0x0a, 0x01, 0x6e, 0x06, 0x0a, 0x00, 0x02, 0x6f,
            0x6e, 0x72, 0x0a, 0x04, 0x73, 0x6d, 0x19, 0x19, 0x73, 0x6d, 0x27, 0x7e, 0x1f, 0x13, 0x1b, 0x08,
            0x18, 0x77, 0x09, 0x0e, 0x08, 0x2e, 0x0c, 0x08, 0x1e, 0x0c, 0x77, 0x1f, 0x2e, 0x0e, 0x18, 0x0f,
            0x13, 0x18, 0x1f, 0x09, 0x77, 0x0e, 0x0b, 0x09, 0x0e, 0x77, 0x1c, 0x13, 0x16, 0x1b, 0x7b, 0x7e,
            0x12, 0x71, 0x12, 0x70,
        ],
    },
    Signature {
        name: "Shell.ForkBomb",
        // :(){ :|:& };:
        coded: &[0x60, 0x72, 0x67, 0x11, 0x7a, 0x60, 0x76, 0x60, 0x1c, 0x7a, 0x27, 0x79, 0x60],
    },
    Signature {
        name: "grenOS.TestThreat",
        // grenos-menace-de-test
        coded: &[
            0x3d, 0x28, 0x3f, 0x34, 0x35, 0x29, 0x77, 0x37, 0x3f, 0x34, 0x3b, 0x39, 0x3f, 0x77, 0x3e, 0x3f,
            0x77, 0x2e, 0x3f, 0x29, 0x2e,
        ],
    },
];

/// What one pass of the scanner found.
#[derive(Clone, Default)]
pub struct Scan {
    pub files: usize,
    pub bytes: usize,
    /// One line per file that matched, with what it matched.
    pub threats: Vec<String>,
    pub quarantined: usize,
}

pub struct Guard {
    /// The processor's defences, as read back after being asked for.
    pub nx: bool,
    pub write_protect: bool,
    pub smep: bool,
    pub smap: bool,
    /// What the page tables say about the kernel's own sections.
    pub text_read_only: bool,
    pub data_no_execute: bool,
    /// The kernel's code: where it is, how big, and its sum at boot.
    pub text: (u64, u64),
    pub sum: u32,
    pub checks: u32,
    pub tampered: bool,
    pub scans: u32,
    pub last: Scan,
}

/// Reads a model-specific register.
///
/// # Safety
///
/// `msr` must be one the processor has.
unsafe fn read_msr(msr: u32) -> u64 {
    let (low, high): (u32, u32);
    // SAFETY: the caller vouches for the register; reading EFER has no effect.
    unsafe { core::arch::asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags)) };
    u64::from(high) << 32 | u64::from(low)
}

/// # Safety
///
/// `msr` must be one the processor has, and `value` a value it accepts.
unsafe fn write_msr(msr: u32, value: u64) {
    // SAFETY: the caller vouches for both.
    unsafe {
        core::arch::asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") value as u32,
            in("edx") (value >> 32) as u32,
            options(nomem, nostack, preserves_flags)
        )
    };
}

/// Turns on the defences the processor offers, then reads them back and looks
/// at the kernel's own pages.
///
/// # Safety
///
/// Called once, early, with interrupts off. It changes CR0, CR4 and EFER,
/// which is exactly what it is for.
pub unsafe fn arm(frames: &Frames) -> Guard {
    const EFER: u32 = 0xC000_0080;
    const NXE: u64 = 1 << 11;
    const CR0_WP: u64 = 1 << 16;
    const CR4_SMEP: u64 = 1 << 20;
    const CR4_SMAP: u64 = 1 << 21;

    // SAFETY: EFER exists on every x86_64; NXE is what makes the no-execute
    // bit of the page tables mean anything.
    unsafe { write_msr(EFER, read_msr(EFER) | NXE) };

    let (mut cr0, mut cr4): (u64, u64);
    // SAFETY: reading the control registers has no effect.
    unsafe {
        core::arch::asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        core::arch::asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
    }
    cr0 |= CR0_WP;
    // SMEP and SMAP only exist on processors that say so, and setting a bit
    // the chip does not have raises a general protection fault.
    // SAFETY: leaf 7 is present on everything that supports long mode with
    // CPUID leaf 0 >= 7, and reading it has no effect.
    let features = unsafe { core::arch::x86_64::__cpuid_count(7, 0) }.ebx;
    let has_smep = features & (1 << 7) != 0;
    let has_smap = features & (1 << 20) != 0;
    if has_smep {
        cr4 |= CR4_SMEP;
    }
    if has_smap {
        cr4 |= CR4_SMAP;
    }
    // SAFETY: write protection and the two supervisor-access defences. The
    // kernel writes only to pages the tables mark writable, and there is no
    // user memory at all, so neither can bite anything legitimate.
    unsafe {
        core::arch::asm!("mov cr0, {}", in(reg) cr0, options(nomem, nostack, preserves_flags));
        core::arch::asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack, preserves_flags));
    }

    // Read back, rather than assume.
    // SAFETY: as above.
    let (cr0, cr4) = unsafe {
        let (a, b): (u64, u64);
        core::arch::asm!("mov {}, cr0", out(reg) a, options(nomem, nostack, preserves_flags));
        core::arch::asm!("mov {}, cr4", out(reg) b, options(nomem, nostack, preserves_flags));
        (a, b)
    };
    // SAFETY: EFER again.
    let efer = unsafe { read_msr(EFER) };

    let text = section(&raw const __text_start, &raw const __text_end);
    let rodata = section(&raw const __rodata_start, &raw const __data_start);
    let data = section(&raw const __data_start, &raw const __kernel_end);

    // What the tables actually say about our own code and our own data.
    let text_read_only = pages(text).all(|page| paging::flags_of(frames, page).is_none_or(|f| f & WRITABLE == 0));
    let data_no_execute = pages(rodata)
        .chain(pages(data))
        .all(|page| paging::flags_of(frames, page).is_none_or(|f| f & NO_EXECUTE != 0));

    let sum = measure(text);
    Guard {
        nx: efer & NXE != 0,
        write_protect: cr0 & CR0_WP != 0,
        smep: cr4 & CR4_SMEP != 0,
        smap: cr4 & CR4_SMAP != 0,
        text_read_only,
        data_no_execute,
        text,
        sum,
        checks: 0,
        tampered: false,
        scans: 0,
        last: Scan::default(),
    }
}

fn section(from: *const u8, to: *const u8) -> (u64, u64) {
    (from as u64, to as u64)
}

/// Every page of a range, by its first address.
fn pages((from, to): (u64, u64)) -> impl Iterator<Item = u64> {
    (from & !0xFFF..to).step_by(4096)
}

/// A sum over the kernel's code. Not a cryptographic hash — it catches a
/// change, not an attacker who knows the algorithm; the day the machine can
/// hash properly, this is the one line to replace.
pub fn measure((from, to): (u64, u64)) -> u32 {
    let mut sum: u32 = 0x811C_9DC5;
    let mut at = from;
    while at < to {
        // SAFETY: the range comes from the linker's own symbols, so it is the
        // kernel's own mapped code.
        let byte = unsafe { (at as *const u8).read_volatile() };
        sum = (sum ^ u32::from(byte)).wrapping_mul(0x0100_0193);
        at += 1;
    }
    sum
}

impl Guard {
    /// Measures the code again and says whether it still matches.
    pub fn verify(&mut self) -> bool {
        self.checks += 1;
        let now = measure(self.text);
        if now != self.sum {
            self.tampered = true;
        }
        !self.tampered
    }

    /// Looks through every file, and puts what matches out of reach.
    pub fn scan(&mut self, files: &mut Fs) -> Scan {
        self.scans += 1;
        let mut scan = Scan::default();
        let mut caught: Vec<(String, &'static str)> = Vec::new();
        for path in walk(files) {
            let Some(entry) = files.get(&path) else {
                continue;
            };
            if entry.kind != Kind::File || path.starts_with(QUARANTINE) {
                continue;
            }
            scan.files += 1;
            scan.bytes += entry.text.len();
            if let Some(name) = matched(entry.text.as_bytes()) {
                caught.push((path, name));
            }
        }
        for (path, name) in caught {
            let text = files.read(&path).unwrap_or_default().to_string();
            let kept = fs::join(QUARANTINE, fs::name_of(&path));
            files.make_dir(QUARANTINE);
            if files.write(&kept, &text) && files.remove(&path) {
                scan.quarantined += 1;
                scan.threats.push(format!("{name} dans {path}, mis en quarantaine"));
            } else {
                scan.threats.push(format!("{name} dans {path}, impossible à déplacer"));
            }
        }
        self.last = scan.clone();
        scan
    }

    /// One line per thing the human should know, for the Sécurité window.
    pub fn lines(&self) -> Vec<String> {
        alloc::vec![
            format!("Exécution des données (NX) : {}", yes(self.nx)),
            format!("Écriture du code interdite (CR0.WP) : {}", yes(self.write_protect)),
            format!("SMEP : {}", yes(self.smep)),
            format!("SMAP : {}", yes(self.smap)),
            format!("Code du noyau en lecture seule : {}", yes(self.text_read_only)),
            format!("Données non exécutables : {}", yes(self.data_no_execute)),
            format!("Taille du code : {} Ko", (self.text.1 - self.text.0) / 1024),
            format!("Empreinte du code : {:08x}", self.sum),
            format!(
                "Intégrité : {}",
                if self.tampered { "MODIFIÉ depuis le démarrage" } else { "conforme au démarrage" }
            ),
            format!("Vérifications : {}", self.checks),
            format!("Analyses de fichiers : {}", self.scans),
            format!("Derniere analyse : {} fichiers, {} menaces", self.last.files, self.last.threats.len()),
        ]
    }
}

fn yes(state: bool) -> &'static str {
    if state { "actif" } else { "inactif" }
}

/// Every path in the file system, deepest last.
fn walk(files: &Fs) -> Vec<String> {
    let mut out = Vec::new();
    let mut todo = alloc::vec!["/".to_string()];
    while let Some(dir) = todo.pop() {
        for entry in files.list(&dir) {
            out.push(entry.path.clone());
            if entry.kind == Kind::Dir {
                todo.push(entry.path.clone());
            }
        }
    }
    out
}

/// The name of the first signature this content matches.
fn matched(content: &[u8]) -> Option<&'static str> {
    SIGNATURES.iter().find(|signature| holds(content, signature)).map(|signature| signature.name)
}

fn holds(content: &[u8], signature: &Signature) -> bool {
    let length = signature.coded.len();
    if content.len() < length {
        return false;
    }
    content
        .windows(length)
        .any(|window| window.iter().zip(signature.coded).all(|(&byte, &coded)| byte == coded ^ KEY))
}
