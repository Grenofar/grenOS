//! Installing an update from grenOS itself (docs/specs/disk-and-updates.md §7).
//!
//! In order, and nothing is written before every check has passed:
//!
//! 1. the signed manifest of the newest image, and its signature;
//! 2. the signature, verified with Ed25519 against the release key compiled
//!    into this kernel — the TLS connection is encrypted but the server is not
//!    authenticated, so this is the only trust;
//! 3. the manifest itself, read strictly, and its build newer than ours;
//! 4. our own disk, the one we booted from (storage.rs, rule 1);
//! 5. the kernel file, whose size and SHA-256 must be the manifest's;
//! 6. only then, the kernel written into the slot we did not boot from, read
//!    back and hashed again, and limine.conf switched to that slot.
//!
//! A failure at any step stops there and says why. If the new kernel does not
//! boot, the previous one is still in the other slot: holding a key during
//! Limine's one-second timeout brings the menu back.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::ahci::Disk;
use crate::desktop::Install;
use crate::dhcp::Resolver;
use crate::e1000::Nic;
use crate::http::{Fetch, Phase};
use crate::net::Stack;
use crate::sha256::Sha256;
use crate::storage::Store;
use crate::update::{self, Latest, Manifest};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Idle,
    Manifest,
    Signature,
    Kernel,
    Finished,
}

pub struct Installer {
    step: Step,
    fetch: Fetch,
    resolver: Resolver,
    signature_path: String,
    manifest_bytes: Vec<u8>,
    manifest: Option<Manifest>,
    /// Where the installation has got to, for the window and for main.rs.
    pub state: Option<Install>,
}

impl Installer {
    pub fn new() -> Self {
        Installer {
            step: Step::Idle,
            fetch: Fetch::new(),
            resolver: Resolver::new(),
            signature_path: String::new(),
            manifest_bytes: Vec::new(),
            manifest: None,
            state: None,
        }
    }

    pub fn busy(&self) -> bool {
        !matches!(self.step, Step::Idle | Step::Finished)
    }

    fn fail(&mut self, why: String) {
        self.step = Step::Finished;
        self.state = Some(Install::Failed(why));
    }

    fn working(&mut self, what: &str) {
        self.state = Some(Install::Working(what.to_string()));
    }

    /// Fetches a published file, binary, at most `limit` bytes.
    fn get(&mut self, stack: &mut Stack, nic: &mut Nic, path: &str, limit: usize, now: u64) -> bool {
        self.fetch = Fetch::new();
        self.fetch.wants_text = false;
        self.fetch.limit = limit;
        let url = format!("{}{}", update::BASE, path);
        self.fetch.start(stack, nic, &mut self.resolver, &url, now)
    }

    /// Starts installing `latest`, the newest image the check found.
    pub fn start(&mut self, latest: &Latest, stack: &mut Stack, nic: &mut Nic, now: u64) {
        if self.busy() {
            return;
        }
        let (Some(manifest), Some(signature)) = (latest.manifest.clone(), latest.signature.clone()) else {
            self.fail("cette version n'a pas de manifeste signé".to_string());
            return;
        };
        self.signature_path = signature;
        self.manifest_bytes.clear();
        self.manifest = None;
        self.working("Téléchargement du manifeste signé...");
        if self.get(stack, nic, &manifest, 4_096, now) {
            self.step = Step::Manifest;
        } else {
            let why = format!("téléchargement du manifeste : {}", self.fetch.status);
            self.fail(why);
        }
    }

    /// Moves the installation along; called every time round the main loop.
    /// `running` is this kernel's build stamp.
    pub fn poll(
        &mut self,
        stack: &mut Stack,
        nic: &mut Nic,
        now: u64,
        disks: &mut [Disk],
        store: Option<&Store>,
        running: &str,
    ) {
        if !self.busy() {
            return;
        }
        self.resolver.poll(stack, nic, now);
        self.fetch.poll(stack, nic, &mut self.resolver, now);
        match self.fetch.phase {
            Phase::Failed => {
                let why = format!("téléchargement : {}", self.fetch.status);
                self.fail(why);
                return;
            }
            Phase::Done => {}
            _ => {
                if self.step == Step::Kernel {
                    let size = self.manifest.as_ref().map_or(0, |manifest| manifest.size);
                    let line = format!(
                        "Téléchargement du noyau : {} Ko sur {} Ko",
                        self.fetch.received() / 1024,
                        size / 1024
                    );
                    if self.state != Some(Install::Working(line.clone())) {
                        self.state = Some(Install::Working(line));
                    }
                }
                return;
            }
        }
        if self.fetch.code != 200 {
            let why = format!("le serveur a répondu {}", self.fetch.code);
            self.fail(why);
            return;
        }
        let body = core::mem::take(&mut self.fetch.body);
        match self.step {
            Step::Manifest => {
                self.manifest_bytes = body;
                self.working("Téléchargement de la signature...");
                let path = self.signature_path.clone();
                if self.get(stack, nic, &path, 64, now) {
                    self.step = Step::Signature;
                } else {
                    let why = format!("téléchargement de la signature : {}", self.fetch.status);
                    self.fail(why);
                }
            }
            Step::Signature => self.check_and_fetch_kernel(&body, stack, nic, now, store, running),
            Step::Kernel => self.write(&body, disks, store),
            Step::Idle | Step::Finished => {}
        }
    }

    fn check_and_fetch_kernel(
        &mut self,
        signature: &[u8],
        stack: &mut Stack,
        nic: &mut Nic,
        now: u64,
        store: Option<&Store>,
        running: &str,
    ) {
        let Ok(signature) = <[u8; 64]>::try_from(signature) else {
            self.fail("la signature n'a pas 64 octets".to_string());
            return;
        };
        if !crate::ed25519::verify(&update::PUBLIC_KEY, &self.manifest_bytes, &signature) {
            self.fail("signature invalide : rien n'est installé".to_string());
            return;
        }
        let manifest = match update::parse_manifest(&self.manifest_bytes) {
            Ok(manifest) => manifest,
            Err(why) => {
                self.fail(format!("manifeste refusé : {why}"));
                return;
            }
        };
        if !update::is_newer(&manifest.build, running) {
            self.fail(format!("{} n'est pas plus récente que la version qui tourne", manifest.build));
            return;
        }
        if store.is_none() {
            self.fail("grenOS ne tourne pas depuis son disque (ISO ?) : il n'y a nulle part où installer".to_string());
            return;
        }
        let (path, size) = (manifest.kernel.clone(), manifest.size as usize);
        self.manifest = Some(manifest);
        self.working("Signature vérifiée. Téléchargement du noyau...");
        if self.get(stack, nic, &path, size, now) {
            self.step = Step::Kernel;
        } else {
            let why = format!("téléchargement du noyau : {}", self.fetch.status);
            self.fail(why);
        }
    }

    fn write(&mut self, kernel: &[u8], disks: &mut [Disk], store: Option<&Store>) {
        let (Some(manifest), Some(store)) = (self.manifest.clone(), store) else {
            self.fail("l'installation a perdu son manifeste ou son disque".to_string());
            return;
        };
        match install(kernel, &manifest, disks, store) {
            Ok(slot) => {
                self.step = Step::Finished;
                self.state = Some(Install::Installed { build: manifest.build, slot });
            }
            Err(why) => self.fail(why.to_string()),
        }
    }
}

/// Checks the kernel against its manifest, writes it into the other slot,
/// hashes it back from the disk, and makes Limine boot that slot.
fn install(kernel: &[u8], manifest: &Manifest, disks: &mut [Disk], store: &Store) -> Result<char, &'static str> {
    if kernel.len() as u64 != manifest.size {
        return Err("le noyau téléchargé n'a pas la taille annoncée");
    }
    if crate::sha256::sha256(kernel) != manifest.sha256 {
        return Err("le noyau téléchargé ne correspond pas au manifeste signé");
    }
    let target = if store.slot == 'a' { 'b' } else { 'a' };
    let disk = disks.get_mut(store.disk).ok_or("le disque a disparu")?;
    let volume = &store.volume;

    let slot = volume.find(disk, if target == 'a' { "/boot/kernel-a" } else { "/boot/kernel-b" })?;
    volume.overwrite(disk, &slot, kernel)?;
    disk.flush()?;
    let mut hash = Sha256::default();
    volume.each_chunk(disk, &slot, kernel.len(), |chunk| hash.update(chunk))?;
    if hash.finish() != manifest.sha256 {
        return Err("le noyau relu sur le disque ne correspond pas à celui écrit");
    }

    let conf = volume.find(disk, "/boot/limine/limine.conf")?;
    let text = volume.read(disk, &conf)?;
    let text = core::str::from_utf8(&text).map_err(|_| "limine.conf n'est pas du texte")?;
    let wanted = if target == 'a' { "default_entry: 1" } else { "default_entry: 2" };
    let other = if target == 'a' { "default_entry: 2" } else { "default_entry: 1" };
    let switched = if text.contains(wanted) { text.to_string() } else { text.replacen(other, wanted, 1) };
    if !switched.contains(wanted) || switched.len() != text.len() {
        return Err("limine.conf n'a pas la ligne default_entry attendue");
    }
    // Only the first data bytes change: the file keeps its size, so the rest
    // of it is written back as it was rather than zeroed.
    let mut full = switched.into_bytes();
    full.resize(conf.size as usize, 0);
    volume.overwrite(disk, &conf, &full)?;
    disk.flush()?;
    let back = volume.read(disk, &conf)?;
    if !core::str::from_utf8(&back).is_ok_and(|text| text.contains(wanted)) {
        return Err("limine.conf relu ne désigne pas le nouvel emplacement");
    }
    Ok(target)
}
