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
use crate::html;
use crate::net::{ANY, Ipv4, Stack, address_text, parse_address};
use crate::tls;

const FIN: u8 = 1 << 0;
const SYN: u8 = 1 << 1;
const RST: u8 = 1 << 2;
const PSH: u8 = 1 << 3;
const ACK: u8 = 1 << 4;

/// How long each step may take, how much of a page is kept, and how much data
/// goes into one segment.
const PATIENCE: u64 = 8_000;
const BODY_CAP: usize = 1_000_000;
const SEGMENT: usize = 1_400;
/// Room for the status line and the headers, on top of a body's limit.
const HEAD_ROOM: usize = 16_000;
/// How many redirects are followed before giving up (docs/specs/browser-search.md §2.2).
const MAX_REDIRECTS: u8 = 5;

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
    /// Where the answer finally came from, redirects followed.
    pub url: String,
    /// The body as browser lines (`html::page`), and the page's title.
    pub text: String,
    pub title: String,
    /// The body exactly as it came, for a program that reads it (the update
    /// check reads JSON).
    pub body: Vec<u8>,
    /// One line for the window to show while it works.
    pub status: String,
    /// The HTTP status code of the answer, 0 until there is one.
    pub code: u16,
    /// The largest body kept. Pages stay small; the installer raises it for a
    /// kernel.
    pub limit: usize,
    /// Whether the body is turned into text for a window. A kernel is not.
    pub wants_text: bool,
    raw: Vec<u8>,
    tls: Option<tls::Client>,
    asked: bool,
    /// Redirects followed so far, and the next address to follow.
    redirects: u8,
    follow: Option<String>,
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
            url: String::new(),
            text: String::new(),
            title: String::new(),
            body: Vec::new(),
            status: String::new(),
            code: 0,
            limit: BODY_CAP,
            wants_text: true,
            raw: Vec::new(),
            tls: None,
            asked: false,
            redirects: 0,
            follow: None,
            local_port: 49152,
            sequence: 0,
            acknowledged: 0,
            since: 0,
        }
    }

    pub fn busy(&self) -> bool {
        matches!(self.phase, Phase::Resolving | Phase::Connecting | Phase::Reading)
    }

    /// Bytes of the answer received so far, headers included: progress for a
    /// long download.
    pub fn received(&self) -> usize {
        self.answer().len()
    }

    /// Starts fetching `http://` or `https://host[:port]/path`. False when the
    /// address cannot be reached from here.
    pub fn start(&mut self, stack: &mut Stack, nic: &mut Nic, resolver: &mut Resolver, url: &str, now: u64) -> bool {
        self.redirects = 0;
        self.follow = None;
        self.begin(stack, nic, resolver, url, now)
    }

    fn begin(&mut self, stack: &mut Stack, nic: &mut Nic, resolver: &mut Resolver, url: &str, now: u64) -> bool {
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
        let default_port = if secure { 443 } else { 80 };
        let scheme = if secure { "https" } else { "http" };
        self.url = if port == default_port {
            format!("{scheme}://{host}{}", self.path)
        } else {
            format!("{scheme}://{host}:{port}{}", self.path)
        };
        self.text.clear();
        self.title.clear();
        self.body.clear();
        self.raw.clear();
        self.code = 0;
        self.asked = false;
        self.since = now;
        // A new port for each redirect: the last connection may still be
        // closing on the server's side.
        self.local_port = 49152 + ((now + u64::from(self.redirects) * 997) % 8000) as u16;
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

    /// The request. No `Accept-Encoding`: nothing compressed can be read here.
    /// Google alone gets `SOCS=CAI`, the cookie its consent page sets for
    /// "Reject all" — this browser keeps no cookies, so that is its true state.
    fn request(&self) -> String {
        let host = self.host.to_ascii_lowercase();
        let google = host == "google.com" || host.ends_with(".google.com");
        format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: grenOS/{}\r\nConnection: close\r\nAccept: */*\r\n{}\r\n",
            self.path,
            self.host,
            env!("CARGO_PKG_VERSION"),
            if google { "Cookie: SOCS=CAI\r\n" } else { "" }
        )
    }

    /// Moves the conversation along. Called every time round the main loop.
    pub fn poll(&mut self, stack: &mut Stack, nic: &mut Nic, resolver: &mut Resolver, now: u64) {
        self.step(stack, nic, resolver, now);
        if let Some(next) = self.follow.take() {
            self.redirects += 1;
            self.begin(stack, nic, resolver, &next, now);
        }
    }

    fn step(&mut self, stack: &mut Stack, nic: &mut Nic, resolver: &mut Resolver, now: u64) {
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
            if self.raw.len() + data.len() > self.limit + HEAD_ROOM {
                return Err("la réponse dépasse la taille permise".to_string());
            }
            self.raw.extend_from_slice(data);
            return Ok(());
        };
        let reply = client.feed(data);
        if client.received.len() > self.limit + HEAD_ROOM {
            self.tls = Some(client);
            return Err("la réponse dépasse la taille permise".to_string());
        }
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
        if raw.is_empty() {
            self.give_up("réponse vide");
            return;
        }
        let Some(split) = raw.windows(4).position(|window| window == b"\r\n\r\n") else {
            self.give_up("réponse sans en-têtes");
            return;
        };
        let head = String::from_utf8_lossy(&raw[..split]).into_owned();
        let mut body = raw[split + 4..].to_vec();
        self.code = head.split_whitespace().nth(1).and_then(|code| code.parse().ok()).unwrap_or(0);
        if matches!(self.code, 301 | 302 | 303 | 307 | 308) {
            if let Some(location) = header(&head, "location") {
                if self.redirects >= MAX_REDIRECTS {
                    self.give_up("trop de redirections, arrêté après 5");
                    return;
                }
                if let Some(next) = html::resolve(&self.url, &location) {
                    self.status = format!("redirigé vers {}...", html::host_of(&next));
                    self.follow = Some(next);
                    self.phase = Phase::Done;
                    return;
                }
            }
        }
        if header(&head, "transfer-encoding").is_some_and(|value| value.to_ascii_lowercase().contains("chunked")) {
            match dechunk(&body) {
                Some(decoded) => body = decoded,
                None => {
                    self.give_up("corps en morceaux (chunked) illisible ou incomplet");
                    return;
                }
            }
        } else if let Some(length) = header(&head, "content-length").and_then(|value| value.parse::<usize>().ok()) {
            if body.len() < length {
                self.give_up("réponse incomplète");
                return;
            }
            body.truncate(length);
        }
        if body.len() > self.limit {
            self.give_up("la réponse dépasse la taille permise");
            return;
        }
        if self.wants_text {
            let page = html::page(&body, header(&head, "content-type").as_deref(), &self.url);
            self.text = page.lines;
            self.title = page.title;
        }
        let lock = if self.secure { "https, chiffré" } else { "http" };
        self.status = format!("{} · {} · {} · {} octets", self.host, lock, self.code, body.len());
        self.body = body;
        self.phase = Phase::Done;
    }

    fn give_up(&mut self, why: &str) {
        self.phase = Phase::Failed;
        self.status = format!("{} : {why}", self.host);
    }
}

/// Checks the body decoders on fixed inputs (docs/specs/browser-search.md
/// §2.1): `Ok`, or the name of the case that failed.
pub fn self_test() -> Result<(), &'static str> {
    // Input, what it decodes to (None: refused), and the case's name.
    type Case = (&'static [u8], Option<&'static [u8]>, &'static str);
    let cases: [Case; 5] = [
        (b"5\r\nhello\r\n0\r\n\r\n", Some(b"hello"), "chunked"),
        (b"5;ext=1\r\nhello\r\n6\r\n world\r\n0\r\nTrailer: x\r\n\r\n", Some(b"hello world"), "chunked trailer"),
        (b"0A\r\n0123456789\r\n0\r\n\r\n", Some(b"0123456789"), "chunked hex"),
        (b"5\r\nhel", None, "chunked truncated"),
        (b"FFFFFFFFFFFFFFFFFF\r\n", None, "chunked overflow"),
    ];
    for (input, expected, name) in cases {
        if dechunk(input).as_deref() != expected {
            return Err(name);
        }
    }
    let head = "HTTP/1.1 302 Found\r\nlocation: /ailleurs\r\nContent-Type: text/html";
    if header(head, "Location").as_deref() != Some("/ailleurs") {
        return Err("header case");
    }
    Ok(())
}

/// The value of header `name` in a response head, whatever its case (RFC 9110
/// §5.1: field names are case-insensitive). Pure.
pub fn header(head: &str, name: &str) -> Option<String> {
    head.lines()
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.trim().eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim().to_string())
}

/// Decodes a body sent with `Transfer-Encoding: chunked` (RFC 9112 §7.1):
/// `chunked-body = *chunk last-chunk trailer-section CRLF`. None when the
/// input is malformed or cut short. Never panics. Pure.
///
/// The CRLF right after the zero size ends the last-chunk line; the trailer
/// section, zero or more header lines, ends with its own CRLF. Two models out
/// of two got exactly that wrong when asked (docs/specs/browser-search.md).
pub fn dechunk(body: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut at = 0usize;
    loop {
        let line_end = find_crlf(body, at)?;
        let line = &body[at..line_end];
        let digits = line.split(|&b| b == b';').next()?;
        if digits.is_empty() {
            return None;
        }
        let mut size: usize = 0;
        for &digit in digits {
            let value = match digit {
                b'0'..=b'9' => digit - b'0',
                b'a'..=b'f' => digit - b'a' + 10,
                b'A'..=b'F' => digit - b'A' + 10,
                _ => return None,
            };
            size = size.checked_mul(16)?.checked_add(usize::from(value))?;
        }
        at = line_end + 2;
        if size == 0 {
            loop {
                let end = find_crlf(body, at)?;
                if end == at {
                    return Some(out);
                }
                at = end + 2;
            }
        }
        let data_end = at.checked_add(size)?;
        if data_end.checked_add(2)? > body.len() || &body[data_end..data_end + 2] != b"\r\n" {
            return None;
        }
        out.extend_from_slice(&body[at..data_end]);
        at = data_end + 2;
    }
}

/// The index of the next CRLF at or after `from`.
fn find_crlf(bytes: &[u8], from: usize) -> Option<usize> {
    bytes.get(from..)?.windows(2).position(|pair| pair == b"\r\n").map(|offset| from + offset)
}
