//! Fetching a page: a small TCP client, TLS 1.3 on top of it when the address
//! says https, and just enough HTTP to ask for one document and read the
//! answer.
//!
//! One conversation at a time, in order. It does not retransmit its own data:
//! what it sends — a ClientHello, a Finished, one request — fits in single
//! segments, and a lost one is retried by asking again. That is enough for a
//! browser that opens a page when someone clicks and an update check that
//! reads one small file, and it keeps the whole thing readable.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::dhcp::Resolver;
use crate::e1000::Nic;
use crate::net::{ANY, Ipv4, Stack, address_text, parse_address};
use crate::tls;

const FIN: u8 = 1 << 0;
const SYN: u8 = 1 << 1;
const RST: u8 = 1 << 2;
const PSH: u8 = 1 << 3;
const ACK: u8 = 1 << 4;

/// How long each step may take, how much of an answer is kept, and how much
/// data goes into one segment.
const PATIENCE: u64 = 8_000;
const BODY_CAP: usize = 120_000;
const SEGMENT: usize = 1_400;

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
    /// True when the conversation is wrapped in TLS.
    pub secure: bool,
    /// The body, headers taken off and tags stripped, for a text window.
    pub text: String,
    /// The body exactly as it came, for a program that reads it (the update
    /// check reads JSON).
    pub body: Vec<u8>,
    /// One line for the window to show while it works.
    pub status: String,
    raw: Vec<u8>,
    tls: Option<tls::Client>,
    asked: bool,
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
            secure: false,
            text: String::new(),
            body: Vec::new(),
            status: String::new(),
            raw: Vec::new(),
            tls: None,
            asked: false,
            local_port: 49152,
            sequence: 0,
            acknowledged: 0,
            since: 0,
        }
    }

    pub fn busy(&self) -> bool {
        matches!(self.phase, Phase::Resolving | Phase::Connecting | Phase::Reading)
    }

    /// Starts fetching `http://` or `https://host[:port]/path`. False when the
    /// address cannot be reached from here.
    pub fn start(&mut self, stack: &mut Stack, nic: &mut Nic, resolver: &mut Resolver, url: &str, now: u64) -> bool {
        let (secure, rest) = if let Some(rest) = url.strip_prefix("https://") {
            (true, rest)
        } else if let Some(rest) = url.strip_prefix("http://") {
            (false, rest)
        } else {
            (false, url.strip_prefix("http:").unwrap_or(url))
        };
        let rest = rest.trim_start_matches('/');
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let (host, port) = match authority.split_once(':') {
            Some((host, port)) => (host, port.parse().unwrap_or(if secure { 443 } else { 80 })),
            None => (authority, if secure { 443 } else { 80 }),
        };
        if host.is_empty() {
            self.phase = Phase::Failed;
            self.status = "adresse vide".to_string();
            return false;
        }
        self.host = host.to_string();
        self.path = format!("/{path}");
        self.port = port;
        self.secure = secure;
        self.text.clear();
        self.body.clear();
        self.raw.clear();
        self.asked = false;
        self.since = now;
        self.local_port = 49152 + (now % 8000) as u16;
        self.sequence = u32::from_le_bytes(crate::rand::bytes()[..4].try_into().unwrap_or([0; 4]));
        self.tls = secure.then(|| tls::Client::new(host, crate::rand::bytes(), crate::rand::bytes(), crate::rand::bytes()));
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

    /// Data on the established connection, in segments small enough for the
    /// wire, and the sequence number moved past it.
    fn send(&mut self, stack: &mut Stack, nic: &mut Nic, data: &[u8], now: u64) {
        for piece in data.chunks(SEGMENT) {
            stack.send_tcp(
                nic,
                self.address,
                self.port,
                self.local_port,
                self.sequence,
                self.acknowledged,
                ACK | PSH,
                piece,
                now,
            );
            self.sequence = self.sequence.wrapping_add(piece.len() as u32);
        }
    }

    fn request(&self) -> String {
        format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: grenOS/{}\r\nConnection: close\r\nAccept: */*\r\n\r\n",
            self.path,
            self.host,
            env!("CARGO_PKG_VERSION")
        )
    }

    /// Moves the conversation along. Called every time round the main loop.
    pub fn poll(&mut self, stack: &mut Stack, nic: &mut Nic, resolver: &mut Resolver, now: u64) {
        if !self.busy() {
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
                return;
            }
        }
        // The SYN may have left before the router's card was known.
        if self.phase == Phase::Connecting && now.saturating_sub(self.since) > 900 {
            self.since = now;
            stack.send_tcp(nic, self.address, self.port, self.local_port, self.sequence, 0, SYN, &[], now);
        }

        let (address, port, local) = (self.address, self.port, self.local_port);
        let mut mine = Vec::new();
        stack.segments.retain(|segment| {
            // Ours: our port, their address, their port. Anything else on the
            // wire belongs to somebody else's conversation, and stays put.
            if segment.to_port == local && segment.from == address && segment.from_port == port {
                mine.push((segment.sequence, segment.acknowledged, segment.flags, segment.data.clone()));
                return false;
            }
            true
        });

        for (sequence, acknowledged, flags, data) in mine {
            if flags & RST != 0 {
                self.give_up("le serveur a refusé la connexion");
                return;
            }
            if self.phase == Phase::Connecting && flags & SYN != 0 && flags & ACK != 0 {
                self.sequence = acknowledged;
                self.acknowledged = sequence.wrapping_add(1);
                stack.send_tcp(nic, address, port, local, self.sequence, self.acknowledged, ACK, &[], now);
                self.phase = Phase::Reading;
                self.since = now;
                if let Some(mut client) = self.tls.take() {
                    self.status = format!("chiffrement avec {}...", self.host);
                    let hello = client.hello();
                    self.tls = Some(client);
                    self.send(stack, nic, &hello, now);
                } else {
                    self.status = format!("lecture de {}...", self.host);
                    let request = self.request();
                    self.send(stack, nic, request.as_bytes(), now);
                    self.asked = true;
                }
                continue;
            }
            if self.phase != Phase::Reading {
                continue;
            }
            if !data.is_empty() && sequence == self.acknowledged {
                self.acknowledged = self.acknowledged.wrapping_add(data.len() as u32);
                stack.send_tcp(nic, address, port, local, self.sequence, self.acknowledged, ACK, &[], now);
                self.since = now;
                if let Err(why) = self.take_in(stack, nic, &data, now) {
                    self.give_up(&why);
                    return;
                }
            }
            if flags & FIN != 0 {
                self.acknowledged = self.acknowledged.wrapping_add(1);
                stack.send_tcp(nic, address, port, local, self.sequence, self.acknowledged, ACK | FIN, &[], now);
                self.finish();
                return;
            }
        }
        if let Some(client) = &self.tls {
            if client.closed() {
                self.finish();
                return;
            }
        }
        if now.saturating_sub(self.since) > PATIENCE {
            if self.phase == Phase::Reading && !self.answer().is_empty() {
                self.finish();
            } else {
                self.give_up("pas de réponse");
            }
        }
    }

    /// What arrived, through TLS when there is TLS; the request goes out as
    /// soon as the handshake allows it.
    fn take_in(&mut self, stack: &mut Stack, nic: &mut Nic, data: &[u8], now: u64) -> Result<(), String> {
        let Some(mut client) = self.tls.take() else {
            if self.raw.len() < BODY_CAP {
                self.raw.extend_from_slice(data);
            }
            return Ok(());
        };
        let reply = client.feed(data);
        let ready = client.connected();
        let request = (ready && !self.asked).then(|| client.seal(self.request().as_bytes()));
        self.tls = Some(client);
        let reply = reply.map_err(|why| format!("TLS : {why:?}"))?;
        if !reply.is_empty() {
            self.send(stack, nic, &reply, now);
        }
        if let Some(sealed) = request {
            let sealed = sealed.map_err(|why| format!("TLS : {why:?}"))?;
            self.status = format!("lecture de {}...", self.host);
            self.send(stack, nic, &sealed, now);
            self.asked = true;
        }
        Ok(())
    }

    /// The answer so far, whichever road it came by.
    fn answer(&self) -> &[u8] {
        match &self.tls {
            Some(client) => &client.received,
            None => &self.raw,
        }
    }

    fn finish(&mut self) {
        let raw = self.answer().to_vec();
        let answer = String::from_utf8_lossy(&raw).into_owned();
        let (head, body) = answer.split_once("\r\n\r\n").unwrap_or(("", answer.as_str()));
        let code = head.split_whitespace().nth(1).unwrap_or("?").to_string();
        let start = raw.len() - body.len();
        self.body = raw[start..].to_vec();
        self.text = to_text(body);
        let lock = if self.secure { "https, chiffré" } else { "http" };
        self.status = format!("{} · {} · {} · {} octets", self.host, lock, code, raw.len());
        self.phase = if raw.is_empty() { Phase::Failed } else { Phase::Done };
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
