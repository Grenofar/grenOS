//! TLS 1.3, client side, with one cipher suite and one key exchange: what it
//! takes to fetch a file from a server that only speaks https.
//!
//! It offers TLS_CHACHA20_POLY1305_SHA256 and x25519 and nothing else —
//! checked against the server grenOS updates from before a line was written.
//! It is a state machine that turns bytes into bytes and never touches a
//! socket, so the very code that runs in the kernel can be driven from the
//! host against the real server.
//!
//! **What it does not do: check who the server is.** The certificate is read
//! and hashed into the transcript, as the protocol requires, but not checked
//! against a chain of trust — there is no X.509 parser and no root store. The
//! channel is encrypted; it is not authenticated. Anything that installs what
//! it downloads must verify a signature of its own on top.

use alloc::string::String;
use alloc::vec::Vec;

use crate::chacha;
use crate::sha256::{self, Sha256};
use crate::x25519;

const SUITE: u16 = 0x1303;
const GROUP: u16 = 0x001d;
const VERSION: u16 = 0x0304;

/// A ServerHello with this random is a HelloRetryRequest in disguise.
const RETRY: [u8; 32] = [
    0xcf, 0x21, 0xad, 0x74, 0xe5, 0x9a, 0x61, 0x11, 0xbe, 0x1d, 0x8c, 0x02, 0x1e, 0x65, 0xb8, 0x91, 0xc2, 0xa2, 0x11,
    0x16, 0x7a, 0xbb, 0x8c, 0x5e, 0x07, 0x9e, 0x09, 0xe2, 0xc8, 0xa8, 0x33, 0x9c,
];

const RECORD_ALERT: u8 = 21;
const RECORD_HANDSHAKE: u8 = 22;
const RECORD_DATA: u8 = 23;
const RECORD_CHANGE_CIPHER: u8 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The server broke the protocol, or sent something this client cannot use.
    Protocol(&'static str),
    /// The server sent an alert, with its code.
    Alert(u8),
    /// A record failed to decrypt: wrong keys, or tampering.
    Record,
    /// The server's Finished did not match the transcript.
    Finished,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Hello,
    Encrypted,
    Connected,
    Closed,
}

/// One direction's key, its nonce base, and how many records it has sealed.
struct Keys {
    key: [u8; 32],
    iv: [u8; 12],
    sequence: u64,
}

impl Keys {
    fn from_secret(secret: &[u8; 32]) -> Keys {
        let mut key = [0u8; 32];
        sha256::expand_label(secret, "key", &[], &mut key);
        let mut iv = [0u8; 12];
        sha256::expand_label(secret, "iv", &[], &mut iv);
        Keys { key, iv, sequence: 0 }
    }

    /// The nonce for the next record: the base, with the record number
    /// folded into its last eight bytes.
    fn nonce(&mut self) -> [u8; 12] {
        let mut nonce = self.iv;
        for (slot, byte) in nonce[4..].iter_mut().zip(self.sequence.to_be_bytes()) {
            *slot ^= byte;
        }
        self.sequence += 1;
        nonce
    }
}

pub struct Client {
    stage: Stage,
    host: String,
    secret: [u8; 32],
    random: [u8; 32],
    session: [u8; 32],
    transcript: Sha256,
    /// Bytes from the wire not yet made into records.
    inbox: Vec<u8>,
    /// Handshake bytes not yet made into messages: a message may span records.
    pending: Vec<u8>,
    client_handshake: [u8; 32],
    server_handshake: [u8; 32],
    master: [u8; 32],
    read: Option<Keys>,
    write: Option<Keys>,
    /// Application data the server has sent.
    pub received: Vec<u8>,
    /// The size of the certificate the server presented — reported, not checked.
    pub certificate: usize,
}

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn extension(out: &mut Vec<u8>, kind: u16, body: &[u8]) {
    put_u16(out, kind);
    put_u16(out, body.len() as u16);
    out.extend_from_slice(body);
}

fn message(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 4);
    out.push(kind);
    out.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
    out.extend_from_slice(body);
    out
}

fn record(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 5);
    out.extend_from_slice(&[kind, 3, 3]);
    put_u16(&mut out, body.len() as u16);
    out.extend_from_slice(body);
    out
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]))
}

/// A record sealed under `keys`: the real type goes inside, and the outside
/// always says "application data", as TLS 1.3 hides everything.
fn seal_record(keys: &mut Keys, kind: u8, data: &[u8]) -> Vec<u8> {
    let mut inner = Vec::with_capacity(data.len() + 1 + 16);
    inner.extend_from_slice(data);
    inner.push(kind);
    let length = inner.len() + 16;
    let header = [RECORD_DATA, 3, 3, (length >> 8) as u8, length as u8];
    let nonce = keys.nonce();
    let tag = chacha::seal(&keys.key, &nonce, &header, &mut inner);
    let mut out = header.to_vec();
    out.extend_from_slice(&inner);
    out.extend_from_slice(&tag);
    out
}

/// A record opened under `keys`: its real type, and what it carried.
fn open_record(keys: &mut Keys, header: &[u8], body: &[u8]) -> Result<(u8, Vec<u8>), Error> {
    let nonce = keys.nonce();
    let mut plain = chacha::open(&keys.key, &nonce, header, body).ok_or(Error::Record)?;
    while plain.last() == Some(&0) {
        plain.pop();
    }
    let kind = plain.pop().ok_or(Error::Record)?;
    Ok((kind, plain))
}

impl Client {
    /// A client for `host`. The secret, the random and the session id must
    /// be fresh for every connection; where they come from is the caller's
    /// business, because the kernel and the host test have different sources.
    pub fn new(host: &str, secret: [u8; 32], random: [u8; 32], session: [u8; 32]) -> Self {
        Client {
            stage: Stage::Hello,
            host: String::from(host),
            secret,
            random,
            session,
            transcript: Sha256::default(),
            inbox: Vec::new(),
            pending: Vec::new(),
            client_handshake: [0; 32],
            server_handshake: [0; 32],
            master: [0; 32],
            read: None,
            write: None,
            received: Vec::new(),
            certificate: 0,
        }
    }

    pub fn connected(&self) -> bool {
        self.stage == Stage::Connected
    }

    pub fn closed(&self) -> bool {
        self.stage == Stage::Closed
    }

    /// The first flight: the ClientHello, ready for the wire.
    pub fn hello(&mut self) -> Vec<u8> {
        let mut body = Vec::with_capacity(256);
        put_u16(&mut body, 0x0303);
        body.extend_from_slice(&self.random);
        body.push(32);
        body.extend_from_slice(&self.session);
        put_u16(&mut body, 2);
        put_u16(&mut body, SUITE);
        body.extend_from_slice(&[1, 0]);

        let mut extensions = Vec::with_capacity(160);
        let name = self.host.as_bytes();
        let mut server_name = Vec::with_capacity(name.len() + 5);
        put_u16(&mut server_name, (name.len() + 3) as u16);
        server_name.push(0);
        put_u16(&mut server_name, name.len() as u16);
        server_name.extend_from_slice(name);
        extension(&mut extensions, 0x0000, &server_name);
        extension(&mut extensions, 0x000a, &[0, 2, 0x00, 0x1d]);
        // What signatures we say we accept. We do not verify any of them, but
        // a server with nothing it may use refuses the handshake.
        extension(&mut extensions, 0x000d, &[0, 8, 0x04, 0x03, 0x08, 0x04, 0x04, 0x01, 0x08, 0x07]);
        extension(&mut extensions, 0x002b, &[2, 0x03, 0x04]);
        let public = x25519::public(&self.secret);
        let mut share = Vec::with_capacity(38);
        put_u16(&mut share, 36);
        put_u16(&mut share, GROUP);
        put_u16(&mut share, 32);
        share.extend_from_slice(&public);
        extension(&mut extensions, 0x0033, &share);

        put_u16(&mut body, extensions.len() as u16);
        body.extend_from_slice(&extensions);
        let hello = message(1, &body);
        self.transcript.update(&hello);
        record(RECORD_HANDSHAKE, &hello)
    }

    /// Takes bytes from the server and returns bytes to send back — the
    /// client's Finished, once the server's has been checked.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Error> {
        self.inbox.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            if self.inbox.len() < 5 {
                break;
            }
            let length = usize::from(u16_at(&self.inbox, 3).unwrap_or(0));
            if self.inbox.len() < 5 + length {
                break;
            }
            let rest = self.inbox.split_off(5 + length);
            let whole = core::mem::replace(&mut self.inbox, rest);
            let (header, body) = whole.split_at(5);
            let kind = header[0];
            match kind {
                RECORD_CHANGE_CIPHER => {}
                RECORD_ALERT => return Err(Error::Alert(body.get(1).copied().unwrap_or(0))),
                RECORD_HANDSHAKE if self.stage == Stage::Hello => {
                    self.pending.extend_from_slice(body);
                    self.messages(&mut out)?;
                }
                RECORD_DATA => {
                    let keys = self.read.as_mut().ok_or(Error::Protocol("encrypted record before keys"))?;
                    let (inner, plain) = open_record(keys, header, body)?;
                    match inner {
                        RECORD_HANDSHAKE => {
                            self.pending.extend_from_slice(&plain);
                            self.messages(&mut out)?;
                        }
                        RECORD_DATA if self.stage == Stage::Connected => self.received.extend_from_slice(&plain),
                        RECORD_ALERT => {
                            // close_notify is how a server says it has finished.
                            if plain.get(1) == Some(&0) {
                                self.stage = Stage::Closed;
                                return Ok(out);
                            }
                            return Err(Error::Alert(plain.get(1).copied().unwrap_or(0)));
                        }
                        _ => return Err(Error::Protocol("unexpected inner record")),
                    }
                }
                _ => return Err(Error::Protocol("unexpected record")),
            }
        }
        Ok(out)
    }

    /// Application data, sealed for the wire.
    pub fn seal(&mut self, data: &[u8]) -> Result<Vec<u8>, Error> {
        if self.stage != Stage::Connected {
            return Err(Error::Protocol("not connected"));
        }
        let keys = self.write.as_mut().ok_or(Error::Protocol("no keys"))?;
        let mut out = Vec::with_capacity(data.len() + 32);
        for chunk in data.chunks(16_000) {
            out.extend_from_slice(&seal_record(keys, RECORD_DATA, chunk));
        }
        Ok(out)
    }

    /// Every whole handshake message waiting, in order.
    fn messages(&mut self, out: &mut Vec<u8>) -> Result<(), Error> {
        loop {
            if self.pending.len() < 4 {
                return Ok(());
            }
            let length = (usize::from(self.pending[1]) << 16) | (usize::from(self.pending[2]) << 8) | usize::from(self.pending[3]);
            if self.pending.len() < 4 + length {
                return Ok(());
            }
            let rest = self.pending.split_off(4 + length);
            let whole = core::mem::replace(&mut self.pending, rest);
            let kind = whole[0];
            let body = &whole[4..];
            match (self.stage, kind) {
                (Stage::Hello, 2) => {
                    self.server_hello(body)?;
                    self.transcript.update(&whole);
                    self.handshake_keys();
                    self.stage = Stage::Encrypted;
                }
                (Stage::Encrypted, 8 | 15) => self.transcript.update(&whole),
                (Stage::Encrypted, 11) => {
                    self.certificate = body.len();
                    self.transcript.update(&whole);
                }
                (Stage::Encrypted, 20) => {
                    self.server_finished(body)?;
                    self.transcript.update(&whole);
                    out.extend_from_slice(&self.client_finished());
                    self.stage = Stage::Connected;
                }
                // A session ticket, or anything else after the handshake: not needed.
                (Stage::Connected, _) => {}
                _ => return Err(Error::Protocol("handshake message out of order")),
            }
        }
    }

    fn server_hello(&mut self, body: &[u8]) -> Result<(), Error> {
        let random = body.get(2..34).ok_or(Error::Protocol("short ServerHello"))?;
        if random == RETRY {
            return Err(Error::Protocol("the server asked for another key exchange"));
        }
        let session = usize::from(*body.get(34).ok_or(Error::Protocol("short ServerHello"))?);
        let mut at = 35 + session;
        if u16_at(body, at) != Some(SUITE) {
            return Err(Error::Protocol("the server chose another cipher"));
        }
        at += 3; // the suite, then the compression method
        let end = at + 2 + usize::from(u16_at(body, at).ok_or(Error::Protocol("no extensions"))?);
        at += 2;
        let (mut version, mut key) = (None, None);
        while at + 4 <= end.min(body.len()) {
            let kind = u16_at(body, at).unwrap_or(0);
            let length = usize::from(u16_at(body, at + 2).unwrap_or(0));
            let data = body.get(at + 4..at + 4 + length).ok_or(Error::Protocol("truncated extension"))?;
            match kind {
                0x002b => version = u16_at(data, 0),
                0x0033 if u16_at(data, 0) == Some(GROUP) && u16_at(data, 2) == Some(32) => {
                    let mut share = [0u8; 32];
                    share.copy_from_slice(data.get(4..36).ok_or(Error::Protocol("short key share"))?);
                    key = Some(share);
                }
                _ => {}
            }
            at += 4 + length;
        }
        if version != Some(VERSION) {
            return Err(Error::Protocol("the server did not choose TLS 1.3"));
        }
        let peer = key.ok_or(Error::Protocol("no x25519 key share"))?;
        let shared = x25519::shared(&self.secret, &peer);
        // The schedule, RFC 8446 §7.1: early secret, then handshake secret.
        let zero = [0u8; 32];
        let empty = sha256::sha256(&[]);
        let early = sha256::extract(&zero, &zero);
        let salt = sha256::derive(&early, "derived", &empty);
        let handshake = sha256::extract(&salt, &shared);
        self.master = sha256::extract(&sha256::derive(&handshake, "derived", &empty), &zero);
        self.client_handshake = handshake;
        Ok(())
    }

    /// The handshake traffic keys, from the transcript through the ServerHello.
    fn handshake_keys(&mut self) {
        let hash = self.transcript.clone().finish();
        let handshake = self.client_handshake;
        self.client_handshake = sha256::derive(&handshake, "c hs traffic", &hash);
        self.server_handshake = sha256::derive(&handshake, "s hs traffic", &hash);
        self.read = Some(Keys::from_secret(&self.server_handshake));
        self.write = Some(Keys::from_secret(&self.client_handshake));
    }

    /// Checks the server's Finished against everything said so far. This is
    /// what proves both sides derived the same keys; it is not what proves
    /// who the server is.
    fn server_finished(&mut self, verify: &[u8]) -> Result<(), Error> {
        let mut key = [0u8; 32];
        sha256::expand_label(&self.server_handshake, "finished", &[], &mut key);
        let expected = sha256::hmac(&key, &self.transcript.clone().finish());
        let mut differ = u8::from(verify.len() != 32);
        for (a, b) in expected.iter().zip(verify) {
            differ |= a ^ b;
        }
        if differ != 0 {
            return Err(Error::Finished);
        }
        Ok(())
    }

    /// Our Finished, sealed under the handshake keys, then the switch to the
    /// application keys for both directions.
    fn client_finished(&mut self) -> Vec<u8> {
        let hash = self.transcript.clone().finish();
        let client_app = sha256::derive(&self.master, "c ap traffic", &hash);
        let server_app = sha256::derive(&self.master, "s ap traffic", &hash);
        let mut key = [0u8; 32];
        sha256::expand_label(&self.client_handshake, "finished", &[], &mut key);
        let verify = sha256::hmac(&key, &hash);
        let finished = message(20, &verify);
        let out = match self.write.as_mut() {
            Some(keys) => seal_record(keys, RECORD_HANDSHAKE, &finished),
            None => Vec::new(),
        };
        self.transcript.update(&finished);
        self.read = Some(Keys::from_secret(&server_app));
        self.write = Some(Keys::from_secret(&client_app));
        out
    }
}
