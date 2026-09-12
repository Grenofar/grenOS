//! Fetching a page: a small TCP client, and just enough HTTP to ask for one
//! document and read the answer.
//!
//! It handles one conversation at a time, in order, with no retransmission of
//! its own data — a request fits in one segment, and a lost one is retried by
//! asking again. That is enough for a browser that opens a page when someone
//! clicks, and it keeps the whole thing readable.
//!
//! Only `http:`. `https:` needs TLS, which needs cryptography this kernel does
//! not have; the browser says so rather than hanging.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::dhcp::Resolver;
use crate::e1000::Nic;
use crate::net::{ANY, Ipv4, Stack, address_text, parse_address};

const FIN: u8 = 1 << 0;
const SYN: u8 = 1 << 1;
const RST: u8 = 1 << 2;
const PSH: u8 = 1 << 3;
const ACK: u8 = 1 << 4;

/// How long each step may take, and how much of a page is kept.
const PATIENCE: u64 = 6_000;
const BODY_CAP: usize = 60_000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Resolving,
    Connecting,
    Reading,
    Done,
    Failed,
}

pub struct Fetch {
    pub phase: Phase,
    pub host: String,
    pub path: String,
    pub port: u16,
    pub address: Ipv4,
    /// What came back, headers taken off and tags stripped.
    pub text: String,
    /// One line for the browser to show while it works.
    pub status: String,
    raw: Vec<u8>,
    local_port: u16,
    sequence: u32,
    acknowledged: u32,
    since: u64,
}

impl Fetch {
    pub fn new() -> Self {
        Fetch {
            phase: Phase::Idle,
            host: String::new(),
            path: String::new(),
            port: 80,
            address: ANY,
            text: String::new(),
            status: String::new(),
            raw: Vec::new(),
            local_port: 49152,
            sequence: 0,
            acknowledged: 0,
            since: 0,
        }
    }

    pub fn busy(&self) -> bool {
        matches!(self.phase, Phase::Resolving | Phase::Connecting | Phase::Reading)
    }

    /// Starts fetching `http://host[:port]/path`. False when the address is
    /// not one this kernel can reach.
    pub fn start(&mut self, stack: &mut Stack, nic: &mut Nic, resolver: &mut Resolver, url: &str, now: u64) -> bool {
        if url.starts_with("https:") {
            self.phase = Phase::Failed;
            self.status = "https demande TLS, que grenOS n'a pas encore. Essayez une adresse en http.".to_string();
            return false;
        }
        let rest = url.strip_prefix("http://").or_else(|| url.strip_prefix("http:")).unwrap_or(url);
        let rest = rest.trim_start_matches('/');
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let (host, port) = authority.split_once(':').unwrap_or((authority, "80"));
        if host.is_empty() {
            self.phase = Phase::Failed;
            self.status = "adresse vide".to_string();
            return false;
        }
        self.host = host.to_string();
        self.path = format!("/{path}");
        self.port = port.parse().unwrap_or(80);
        self.text.clear();
        self.raw.clear();
        self.since = now;
        self.local_port = 49152 + (now % 8000) as u16;
        self.sequence = (now as u32).wrapping_mul(2_654_435_761);
        if !stack.ready() {
            self.phase = Phase::Failed;
            self.status = "pas encore d'adresse : le réseau n'a pas répondu".to_string();
            return false;
        }
        match parse_address(host) {
            Some(address) => {
                self.address = address;
                self.connect(stack, nic, now);
            }
            None => {
                self.phase = Phase::Resolving;
                self.status = format!("recherche de {host}...");
                resolver.ask(stack, nic, host, now);
            }
        }
        true
    }

    fn connect(&mut self, stack: &mut Stack, nic: &mut Nic, now: u64) {
        self.phase = Phase::Connecting;
        self.status = format!("connexion à {}...", address_text(self.address));
        self.since = now;
        stack.send_tcp(nic, self.address, self.port, self.local_port, self.sequence, 0, SYN, &[], now);
    }

    /// Moves the conversation along. Called every time round the main loop.
    pub fn poll(&mut self, stack: &mut Stack, nic: &mut Nic, resolver: &mut Resolver, now: u64) {
        if !self.busy() {
            stack.segments.clear();
            return;
        }
        if self.phase == Phase::Resolving {
            if let Some(address) = resolver.answer {
                self.address = address;
                resolver.answer = None;
                resolver.asking = None;
                self.connect(stack, nic, now);
            } else if resolver.failed {
                self.give_up("le nom n'a pas pu être résolu");
            }
        }
        // The SYN may have gone out before the router's address was known.
        if self.phase == Phase::Connecting && now.saturating_sub(self.since) > 900 {
            self.since = now;
            stack.send_tcp(nic, self.address, self.port, self.local_port, self.sequence, 0, SYN, &[], now);
        }

        let mine: Vec<_> = core::mem::take(&mut stack.segments)
            .into_iter()
            .filter(|segment| {
                // Ours: our port, their address, their port. Anything else on
                // the wire belongs to somebody else's conversation.
                segment.to_port == self.local_port && segment.from == self.address && segment.from_port == self.port
            })
            .collect();
        for segment in mine {
            if segment.flags & RST != 0 {
                self.give_up("le serveur a refusé la connexion");
                return;
            }
            match self.phase {
                Phase::Connecting if segment.flags & SYN != 0 && segment.flags & ACK != 0 => {
                    self.sequence = segment.acknowledged;
                    self.acknowledged = segment.sequence.wrapping_add(1);
                    let request = format!(
                        "GET {} HTTP/1.0\r\nHost: {}\r\nUser-Agent: grenOS/0.6\r\nConnection: close\r\nAccept: text/html\r\n\r\n",
                        self.path, self.host
                    );
                    stack.send_tcp(
                        nic,
                        self.address,
                        self.port,
                        self.local_port,
                        self.sequence,
                        self.acknowledged,
                        ACK | PSH,
                        request.as_bytes(),
                        now,
                    );
                    self.sequence = self.sequence.wrapping_add(request.len() as u32);
                    self.phase = Phase::Reading;
                    self.status = format!("lecture de {}...", self.host);
                    self.since = now;
                }
                Phase::Reading => {
                    if !segment.data.is_empty() && segment.sequence == self.acknowledged {
                        self.acknowledged = self.acknowledged.wrapping_add(segment.data.len() as u32);
                        if self.raw.len() < BODY_CAP {
                            self.raw.extend_from_slice(&segment.data);
                        }
                        stack.send_tcp(
                            nic,
                            self.address,
                            self.port,
                            self.local_port,
                            self.sequence,
                            self.acknowledged,
                            ACK,
                            &[],
                            now,
                        );
                        self.since = now;
                    }
                    if segment.flags & FIN != 0 {
                        self.acknowledged = self.acknowledged.wrapping_add(1);
                        stack.send_tcp(
                            nic,
                            self.address,
                            self.port,
                            self.local_port,
                            self.sequence,
                            self.acknowledged,
                            ACK | FIN,
                            &[],
                            now,
                        );
                        self.finish();
                        return;
                    }
                }
                _ => {}
            }
        }
        if now.saturating_sub(self.since) > PATIENCE {
            if self.phase == Phase::Reading && !self.raw.is_empty() {
                self.finish();
            } else {
                self.give_up("pas de réponse");
            }
        }
    }

    fn finish(&mut self) {
        let answer = String::from_utf8_lossy(&self.raw).into_owned();
        let (head, body) = answer.split_once("\r\n\r\n").unwrap_or(("", answer.as_str()));
        let code = head.split_whitespace().nth(1).unwrap_or("?").to_string();
        self.text = to_text(body);
        self.status = format!("{} · {} · {} octets", self.host, code, self.raw.len());
        self.phase = Phase::Done;
    }

    fn give_up(&mut self, why: &str) {
        self.phase = Phase::Failed;
        self.status = format!("{} : {why}", self.host);
    }
}

/// HTML turned into something a text window can show: tags dropped, the head
/// left out, entities decoded, runs of space collapsed.
pub fn to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut inside_tag = false;
    let mut skipping: Option<&str> = None;
    let bytes: Vec<char> = html.chars().collect();
    let mut at = 0;
    while at < bytes.len() {
        let rest: String = bytes[at..(at + 16).min(bytes.len())].iter().collect();
        let lower = rest.to_ascii_lowercase();
        if let Some(tag) = skipping {
            if lower.starts_with(&format!("</{tag}")) {
                skipping = None;
            }
            at += 1;
            continue;
        }
        let c = bytes[at];
        if c == '<' {
            for tag in ["script", "style", "head"] {
                if lower.starts_with(&format!("<{tag}")) {
                    skipping = Some(tag);
                }
            }
            if lower.starts_with("<br") || lower.starts_with("</p") || lower.starts_with("</div") || lower.starts_with("</h") {
                out.push('\n');
            }
            inside_tag = true;
            at += 1;
            continue;
        }
        if c == '>' {
            inside_tag = false;
            at += 1;
            continue;
        }
        if !inside_tag {
            if c == '&' {
                let entity: String = bytes[at..(at + 8).min(bytes.len())].iter().collect();
                let (text, length) = entity_of(&entity);
                out.push_str(text);
                at += length;
                continue;
            }
            // Runs of space, tab and newline become one space: HTML is
            // written with line breaks that mean nothing on screen.
            if c.is_whitespace() {
                if !out.ends_with(' ') && !out.ends_with('\n') {
                    out.push(' ');
                }
            } else {
                out.push(c);
            }
        }
        at += 1;
    }
    out.trim().to_string()
}

/// The handful of HTML entities worth decoding, and how many characters each
/// one takes up.
fn entity_of(text: &str) -> (&'static str, usize) {
    for (entity, plain) in [
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&apos;", "'"),
        ("&#39;", "'"),
        ("&nbsp;", " "),
        ("&eacute;", "é"),
        ("&egrave;", "è"),
        ("&agrave;", "à"),
        ("&ccedil;", "ç"),
    ] {
        if text.starts_with(entity) {
            return (plain, entity.len());
        }
    }
    ("&", 1)
}
