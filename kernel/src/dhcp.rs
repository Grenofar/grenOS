//! Getting an address without being told one (DHCP), and turning a name into
//! an address (DNS).
//!
//! Both are driven from the main loop: they send, and they look at what the
//! stack has kept for them. Neither blocks, because a kernel that waits for a
//! server is a kernel with a frozen desktop.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::e1000::Nic;
use crate::net::{ANY, Ipv4, Stack, address_text};

const SERVER_PORT: u16 = 67;
const CLIENT_PORT: u16 = 68;
const DNS_PORT: u16 = 53;
const COOKIE: [u8; 4] = [99, 130, 83, 99];
/// How long to wait before asking again, and how many times.
const RETRY: u64 = 2_000;
const TRIES: u32 = 5;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Asking,
    Confirming,
    Bound,
    Failed,
}

pub struct Dhcp {
    pub state: State,
    xid: u32,
    offered: Ipv4,
    server: Ipv4,
    since: u64,
    tries: u32,
}

impl Dhcp {
    pub fn new() -> Self {
        Dhcp { state: State::Idle, xid: 0, offered: ANY, server: ANY, since: 0, tries: 0 }
    }

    /// Starts asking. The exchange identifier comes from the card's address
    /// and the clock, which is enough to tell our answers from anyone else's.
    pub fn start(&mut self, stack: &mut Stack, nic: &mut Nic, now: u64) {
        self.xid = u32::from_be_bytes([stack.mac[2], stack.mac[3], stack.mac[4], stack.mac[5]]) ^ (now as u32);
        self.state = State::Asking;
        self.tries = 0;
        self.since = now;
        self.discover(stack, nic);
        stack.note("net: dhcp, asking for an address".to_string());
    }

    fn discover(&mut self, stack: &mut Stack, nic: &mut Nic) {
        let mut options = alloc::vec![53, 1, 1];
        options.extend_from_slice(&[55, 4, 1, 3, 6, 15]); // mask, router, name servers, domain
        options.push(255);
        let message = self.message(stack, &options);
        stack.broadcast_udp(nic, SERVER_PORT, CLIENT_PORT, &message);
        self.tries += 1;
    }

    fn request(&mut self, stack: &mut Stack, nic: &mut Nic) {
        let mut options = alloc::vec![53, 1, 3];
        options.extend_from_slice(&[54, 4]);
        options.extend_from_slice(&self.server);
        options.extend_from_slice(&[50, 4]);
        options.extend_from_slice(&self.offered);
        options.extend_from_slice(&[55, 4, 1, 3, 6, 15]);
        options.push(255);
        let message = self.message(stack, &options);
        stack.broadcast_udp(nic, SERVER_PORT, CLIENT_PORT, &message);
        self.tries += 1;
    }

    /// The 236 bytes every DHCP message starts with, then the options.
    fn message(&self, stack: &Stack, options: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(300);
        out.extend_from_slice(&[1, 1, 6, 0]); // a request, over Ethernet, six-byte address
        out.extend_from_slice(&self.xid.to_be_bytes());
        out.extend_from_slice(&[0, 0]); // seconds
        out.extend_from_slice(&[0x80, 0x00]); // answer by broadcast: we have no address yet
        out.extend_from_slice(&[0; 16]); // client, your, server and gateway addresses
        out.extend_from_slice(&stack.mac);
        out.extend_from_slice(&[0; 10]); // the rest of the sixteen-byte address field
        out.extend_from_slice(&[0; 192]); // server name and boot file, both empty
        out.extend_from_slice(&COOKIE);
        out.extend_from_slice(options);
        while out.len() < 300 {
            out.push(0);
        }
        out
    }

    /// Looks at what came back, and asks again when nothing did.
    pub fn poll(&mut self, stack: &mut Stack, nic: &mut Nic, now: u64) {
        if matches!(self.state, State::Idle | State::Bound | State::Failed) {
            stack.datagrams.retain(|datagram| datagram.to_port != CLIENT_PORT);
            return;
        }
        let mut answers = Vec::new();
        stack.datagrams.retain(|datagram| {
            if datagram.to_port == CLIENT_PORT {
                answers.push(datagram.data.clone());
                return false;
            }
            true
        });
        for answer in answers {
            self.on_answer(stack, nic, &answer, now);
        }
        if matches!(self.state, State::Asking | State::Confirming) && now.saturating_sub(self.since) > RETRY {
            self.since = now;
            if self.tries >= TRIES {
                self.state = State::Failed;
                stack.note("net: dhcp, no answer".to_string());
                return;
            }
            match self.state {
                State::Asking => self.discover(stack, nic),
                _ => self.request(stack, nic),
            }
        }
    }

    fn on_answer(&mut self, stack: &mut Stack, nic: &mut Nic, message: &[u8], now: u64) {
        if message.len() < 240 || message[0] != 2 || message[236..240] != COOKIE {
            return;
        }
        if u32::from_be_bytes([message[4], message[5], message[6], message[7]]) != self.xid {
            return;
        }
        let yours: Ipv4 = message[16..20].try_into().unwrap_or(ANY);
        let mut kind = 0;
        let (mut mask, mut router, mut dns, mut server) = (None, None, None, None);
        let mut at = 240;
        while at < message.len() {
            let code = message[at];
            if code == 255 {
                break;
            }
            if code == 0 {
                at += 1;
                continue;
            }
            let Some(&length) = message.get(at + 1) else {
                break;
            };
            let from = at + 2;
            let to = from + usize::from(length);
            let Some(value) = message.get(from..to) else {
                break;
            };
            match code {
                53 => kind = value.first().copied().unwrap_or(0),
                1 => mask = value.try_into().ok(),
                3 => router = value.get(..4).and_then(|v| v.try_into().ok()),
                6 => dns = value.get(..4).and_then(|v| v.try_into().ok()),
                54 => server = value.try_into().ok(),
                _ => {}
            }
            at = to;
        }
        match kind {
            // An offer: say yes to it.
            2 if self.state == State::Asking => {
                self.offered = yours;
                self.server = server.unwrap_or(ANY);
                self.state = State::Confirming;
                self.tries = 0;
                self.since = now;
                self.request(stack, nic);
            }
            // An acknowledgement: the address is ours.
            5 => {
                stack.ip = yours;
                if let Some(mask) = mask {
                    stack.mask = mask;
                }
                if let Some(router) = router {
                    stack.gateway = router;
                }
                if let Some(dns) = dns {
                    stack.dns = dns;
                }
                self.state = State::Bound;
                let line = format!(
                    "net: address {} mask {} gateway {} dns {}",
                    address_text(stack.ip),
                    address_text(stack.mask),
                    address_text(stack.gateway),
                    address_text(stack.dns)
                );
                stack.note(line);
                // Ask the router for its card straight away: everything that
                // leaves this machine goes through it.
                let gateway = stack.gateway;
                if gateway != ANY {
                    stack.ask_who_has(nic, gateway);
                }
            }
            // A refusal: start again.
            6 => {
                self.state = State::Failed;
                stack.note("net: dhcp, the server said no".to_string());
            }
            _ => {}
        }
    }
}

/// Turning a name into an address.
pub struct Resolver {
    pub asking: Option<String>,
    pub answer: Option<Ipv4>,
    pub failed: bool,
    xid: u16,
    since: u64,
    tries: u32,
}

impl Resolver {
    pub fn new() -> Self {
        Resolver { asking: None, answer: None, failed: false, xid: 0x6772, since: 0, tries: 0 }
    }

    pub fn ask(&mut self, stack: &mut Stack, nic: &mut Nic, name: &str, now: u64) {
        self.xid = self.xid.wrapping_add(1);
        self.asking = Some(name.to_string());
        self.answer = None;
        self.failed = false;
        self.since = now;
        self.tries = 0;
        self.send(stack, nic, now);
    }

    fn send(&mut self, stack: &mut Stack, nic: &mut Nic, now: u64) {
        let Some(name) = self.asking.clone() else {
            return;
        };
        if stack.dns == ANY {
            self.failed = true;
            return;
        }
        let mut query = Vec::with_capacity(32 + name.len());
        query.extend_from_slice(&self.xid.to_be_bytes());
        query.extend_from_slice(&[0x01, 0x00]); // ask the server to do the work
        query.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 0]); // one question
        for label in name.split('.').filter(|part| !part.is_empty()) {
            query.push(label.len().min(63) as u8);
            query.extend_from_slice(&label.as_bytes()[..label.len().min(63)]);
        }
        query.push(0);
        query.extend_from_slice(&[0, 1, 0, 1]); // an address, on the internet
        let server = stack.dns;
        stack.send_udp(nic, server, DNS_PORT, 40000 + (self.xid % 1000), &query, now);
        self.tries += 1;
    }

    pub fn poll(&mut self, stack: &mut Stack, nic: &mut Nic, now: u64) {
        if self.asking.is_none() || self.answer.is_some() {
            return;
        }
        let server = stack.dns;
        let mut answers = Vec::new();
        stack.datagrams.retain(|datagram| {
            // Only what the name server we asked actually sent back.
            if datagram.from_port == DNS_PORT && datagram.from == server {
                answers.push(datagram.data.clone());
                return false;
            }
            true
        });
        for answer in answers {
            if let Some(address) = read_answer(&answer, self.xid) {
                self.answer = Some(address);
                let name = self.asking.clone().unwrap_or_default();
                stack.note(format!("net: {name} is {}", address_text(address)));
                return;
            }
        }
        if now.saturating_sub(self.since) > RETRY {
            self.since = now;
            if self.tries >= TRIES {
                self.failed = true;
                self.asking = None;
                stack.note("net: the name server did not answer".to_string());
                return;
            }
            self.send(stack, nic, now);
        }
    }
}

/// The first address in a DNS answer, if it is the one we asked for.
fn read_answer(message: &[u8], xid: u16) -> Option<Ipv4> {
    if message.len() < 12 || u16::from_be_bytes([message[0], message[1]]) != xid {
        return None;
    }
    let questions = u16::from_be_bytes([message[4], message[5]]);
    let answers = u16::from_be_bytes([message[6], message[7]]);
    let mut at = 12;
    for _ in 0..questions {
        at = skip_name(message, at)?;
        at += 4; // type and class
    }
    for _ in 0..answers {
        at = skip_name(message, at)?;
        let kind = u16::from_be_bytes([*message.get(at)?, *message.get(at + 1)?]);
        let length = usize::from(u16::from_be_bytes([*message.get(at + 8)?, *message.get(at + 9)?]));
        let data = at + 10;
        if kind == 1 && length == 4 {
            return message.get(data..data + 4)?.try_into().ok();
        }
        at = data + length;
    }
    None
}

/// Walks over a name, which may end in a pointer back into the message.
fn skip_name(message: &[u8], mut at: usize) -> Option<usize> {
    loop {
        let length = *message.get(at)?;
        if length & 0xC0 == 0xC0 {
            return Some(at + 2);
        }
        at += 1 + usize::from(length);
        if length == 0 {
            return Some(at);
        }
    }
}
