//! The network, one layer at a time: Ethernet frames, ARP, IPv4, ICMP and
//! UDP, over the 8254x card.
//!
//! Enough to get an address, answer a ping, ask a name server and carry DHCP.
//! TCP, and with it the web, sits on top of this and comes next. Everything
//! here is polled from the main loop: no allocation happens in an interrupt
//! handler, because none of this runs in one.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::e1000::Nic;

pub type Mac = [u8; 6];
pub type Ipv4 = [u8; 4];

pub const BROADCAST: Mac = [0xFF; 6];
pub const ANY: Ipv4 = [0, 0, 0, 0];

const ETHERTYPE_IPV4: u16 = 0x0800;
const ETHERTYPE_ARP: u16 = 0x0806;
const PROTOCOL_ICMP: u8 = 1;
const PROTOCOL_UDP: u8 = 17;
const PROTOCOL_TCP: u8 = 6;

/// How long a learned address stays good, in milliseconds.
const ARP_LIFE: u64 = 120_000;

/// A datagram the stack kept for whoever asked for that port.
pub struct Datagram {
    pub from: Ipv4,
    pub from_port: u16,
    pub to_port: u16,
    pub data: Vec<u8>,
}

/// A segment of a TCP conversation, handed to the client above.
pub struct Segment {
    pub from: Ipv4,
    pub from_port: u16,
    pub to_port: u16,
    pub sequence: u32,
    pub acknowledged: u32,
    pub flags: u8,
    pub data: Vec<u8>,
}

pub struct Stack {
    pub mac: Mac,
    pub ip: Ipv4,
    pub mask: Ipv4,
    pub gateway: Ipv4,
    pub dns: Ipv4,
    /// What the desktop shows: one line per thing that happened.
    pub log: Vec<String>,
    /// Addresses learned from ARP: address, card, when it was learned.
    arp: Vec<(Ipv4, Mac, u64)>,
    /// What came in and has not been read yet.
    pub datagrams: Vec<Datagram>,
    pub segments: Vec<Segment>,
    pub sent: u32,
    pub received: u32,
    pub pings: u32,
    pub pong: u32,
    pub answered: u32,
}

/// The one's complement sum every header here is checked with.
pub fn checksum(parts: &[&[u8]]) -> u16 {
    let mut sum: u32 = 0;
    let mut odd: Option<u8> = None;
    for part in parts {
        let mut bytes = *part;
        if let Some(first) = odd.take() {
            if let Some((&second, rest)) = bytes.split_first() {
                sum += u32::from(u16::from_be_bytes([first, second]));
                bytes = rest;
            } else {
                odd = Some(first);
                continue;
            }
        }
        let mut chunks = bytes.chunks_exact(2);
        for chunk in &mut chunks {
            sum += u32::from(u16::from_be_bytes([chunk[0], chunk[1]]));
        }
        if let Some(&last) = chunks.remainder().first() {
            odd = Some(last);
        }
    }
    if let Some(last) = odd {
        sum += u32::from(u16::from_be_bytes([last, 0]));
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

pub fn address_text(ip: Ipv4) -> String {
    format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3])
}

/// Reads an address written as 1.2.3.4.
pub fn parse_address(text: &str) -> Option<Ipv4> {
    let mut out = [0u8; 4];
    let mut parts = text.split('.');
    for slot in &mut out {
        *slot = parts.next()?.parse().ok()?;
    }
    parts.next().is_none().then_some(out)
}

impl Stack {
    pub fn new(mac: Mac) -> Self {
        Stack {
            mac,
            ip: ANY,
            mask: [255, 255, 255, 0],
            gateway: ANY,
            dns: ANY,
            log: Vec::new(),
            arp: Vec::new(),
            datagrams: Vec::new(),
            segments: Vec::new(),
            sent: 0,
            received: 0,
            pings: 0,
            pong: 0,
            answered: 0,
        }
    }

    pub fn note(&mut self, line: String) {
        crate::serial::write_str(&line);
        crate::serial::write_str("\n");
        self.log.push(line);
        if self.log.len() > 60 {
            self.log.remove(0);
        }
    }

    pub fn ready(&self) -> bool {
        self.ip != ANY
    }

    /// True when this address is on our own wire rather than beyond the router.
    fn local(&self, ip: Ipv4) -> bool {
        (0..4).all(|byte| ip[byte] & self.mask[byte] == self.ip[byte] & self.mask[byte])
    }

    fn known(&self, ip: Ipv4, now: u64) -> Option<Mac> {
        self.arp.iter().find(|(known, _, when)| *known == ip && now.saturating_sub(*when) < ARP_LIFE).map(|(_, mac, _)| *mac)
    }

    fn learn(&mut self, ip: Ipv4, mac: Mac, now: u64) {
        if let Some(entry) = self.arp.iter_mut().find(|(known, _, _)| *known == ip) {
            *entry = (ip, mac, now);
            return;
        }
        self.arp.push((ip, mac, now));
        if self.arp.len() > 16 {
            self.arp.remove(0);
        }
    }

    fn frame(&self, to: Mac, ethertype: u16, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(14 + payload.len());
        out.extend_from_slice(&to);
        out.extend_from_slice(&self.mac);
        out.extend_from_slice(&ethertype.to_be_bytes());
        out.extend_from_slice(payload);
        // The wire wants at least sixty bytes before the checksum.
        while out.len() < 60 {
            out.push(0);
        }
        out
    }

    /// Asks who holds `ip`.
    pub fn ask_who_has(&mut self, nic: &mut Nic, ip: Ipv4) {
        let mut arp = Vec::with_capacity(28);
        arp.extend_from_slice(&[0, 1]); // Ethernet
        arp.extend_from_slice(&ETHERTYPE_IPV4.to_be_bytes());
        arp.extend_from_slice(&[6, 4, 0, 1]); // lengths, and "request"
        arp.extend_from_slice(&self.mac);
        arp.extend_from_slice(&self.ip);
        arp.extend_from_slice(&[0; 6]);
        arp.extend_from_slice(&ip);
        let frame = self.frame(BROADCAST, ETHERTYPE_ARP, &arp);
        if nic.send(&frame) {
            self.sent += 1;
        }
    }

    /// An IPv4 packet, header and all, ready to go into a frame.
    fn packet(&self, to: Ipv4, protocol: u8, payload: &[u8]) -> Vec<u8> {
        let total = 20 + payload.len();
        let mut header = Vec::with_capacity(total);
        header.extend_from_slice(&[0x45, 0x00]);
        header.extend_from_slice(&(total as u16).to_be_bytes());
        header.extend_from_slice(&[0, 0, 0x40, 0x00]); // no fragments
        header.push(64); // time to live
        header.push(protocol);
        header.extend_from_slice(&[0, 0]); // the checksum, filled in below
        header.extend_from_slice(&self.ip);
        header.extend_from_slice(&to);
        let sum = checksum(&[&header]);
        header[10..12].copy_from_slice(&sum.to_be_bytes());
        header.extend_from_slice(payload);
        header
    }

    /// Sends an IPv4 packet, going through the router when the address is not
    /// on our wire. False when the card of the next hop is not known yet: an
    /// ARP request goes out, and the caller tries again in a moment.
    pub fn send_ipv4(&mut self, nic: &mut Nic, to: Ipv4, protocol: u8, payload: &[u8], now: u64) -> bool {
        let hop = if self.local(to) || self.gateway == ANY { to } else { self.gateway };
        let Some(card) = self.known(hop, now) else {
            self.ask_who_has(nic, hop);
            return false;
        };
        let packet = self.packet(to, protocol, payload);
        let frame = self.frame(card, ETHERTYPE_IPV4, &packet);
        let ok = nic.send(&frame);
        if ok {
            self.sent += 1;
        }
        ok
    }

    /// A UDP datagram to everyone on the wire. DHCP needs this: before it
    /// answers, we have no address, and no one to ask where to send.
    pub fn broadcast_udp(&mut self, nic: &mut Nic, port: u16, from_port: u16, data: &[u8]) -> bool {
        let to: Ipv4 = [255, 255, 255, 255];
        let length = 8 + data.len();
        let mut datagram = Vec::with_capacity(length);
        datagram.extend_from_slice(&from_port.to_be_bytes());
        datagram.extend_from_slice(&port.to_be_bytes());
        datagram.extend_from_slice(&(length as u16).to_be_bytes());
        datagram.extend_from_slice(&[0, 0]);
        datagram.extend_from_slice(data);
        let pseudo = pseudo_header(self.ip, to, PROTOCOL_UDP, length);
        let sum = checksum(&[&pseudo, &datagram]);
        datagram[6..8].copy_from_slice(&if sum == 0 { 0xFFFFu16 } else { sum }.to_be_bytes());
        let packet = self.packet(to, PROTOCOL_UDP, &datagram);
        let frame = self.frame(BROADCAST, ETHERTYPE_IPV4, &packet);
        let ok = nic.send(&frame);
        if ok {
            self.sent += 1;
        }
        ok
    }

    /// A UDP datagram. The pseudo-header makes the checksum cover the
    /// addresses as well as the data, as the protocol asks.
    pub fn send_udp(&mut self, nic: &mut Nic, to: Ipv4, port: u16, from_port: u16, data: &[u8], now: u64) -> bool {
        let length = 8 + data.len();
        let mut datagram = Vec::with_capacity(length);
        datagram.extend_from_slice(&from_port.to_be_bytes());
        datagram.extend_from_slice(&port.to_be_bytes());
        datagram.extend_from_slice(&(length as u16).to_be_bytes());
        datagram.extend_from_slice(&[0, 0]);
        datagram.extend_from_slice(data);
        let pseudo = pseudo_header(self.ip, to, PROTOCOL_UDP, length);
        let sum = checksum(&[&pseudo, &datagram]);
        // Zero means "not computed" in UDP, so it is written as all ones.
        let sum = if sum == 0 { 0xFFFF } else { sum };
        datagram[6..8].copy_from_slice(&sum.to_be_bytes());
        self.send_ipv4(nic, to, PROTOCOL_UDP, &datagram, now)
    }

    /// A TCP segment, for the client that lives above this.
    #[allow(clippy::too_many_arguments)]
    pub fn send_tcp(
        &mut self,
        nic: &mut Nic,
        to: Ipv4,
        port: u16,
        from_port: u16,
        sequence: u32,
        acknowledged: u32,
        flags: u8,
        data: &[u8],
        now: u64,
    ) -> bool {
        let length = 20 + data.len();
        let mut segment = Vec::with_capacity(length);
        segment.extend_from_slice(&from_port.to_be_bytes());
        segment.extend_from_slice(&port.to_be_bytes());
        segment.extend_from_slice(&sequence.to_be_bytes());
        segment.extend_from_slice(&acknowledged.to_be_bytes());
        segment.push(5 << 4); // five words of header, no options
        segment.push(flags);
        segment.extend_from_slice(&8192u16.to_be_bytes()); // the window we offer
        segment.extend_from_slice(&[0, 0, 0, 0]); // checksum, then urgent
        segment.extend_from_slice(data);
        let pseudo = pseudo_header(self.ip, to, PROTOCOL_TCP, length);
        let sum = checksum(&[&pseudo, &segment]);
        segment[16..18].copy_from_slice(&sum.to_be_bytes());
        self.send_ipv4(nic, to, PROTOCOL_TCP, &segment, now)
    }

    pub fn ping(&mut self, nic: &mut Nic, to: Ipv4, now: u64) -> bool {
        let mut echo = Vec::with_capacity(16);
        echo.extend_from_slice(&[8, 0, 0, 0]); // echo request, checksum later
        echo.extend_from_slice(&[0x67, 0x72, 0x00, 0x01]); // identifier, sequence
        echo.extend_from_slice(b"grenOS ping");
        let sum = checksum(&[&echo]);
        echo[2..4].copy_from_slice(&sum.to_be_bytes());
        let ok = self.send_ipv4(nic, to, PROTOCOL_ICMP, &echo, now);
        if ok {
            self.pings += 1;
        }
        ok
    }

    /// Reads everything the card has taken in. Answers ARP and pings itself,
    /// and keeps UDP and TCP for whoever asked.
    pub fn poll(&mut self, nic: &mut Nic, now: u64) {
        while let Some(frame) = nic.receive() {
            self.received += 1;
            if frame.len() < 14 {
                continue;
            }
            let ethertype = u16::from_be_bytes([frame[12], frame[13]]);
            let body = &frame[14..];
            match ethertype {
                ETHERTYPE_ARP => self.on_arp(nic, body, now),
                ETHERTYPE_IPV4 => self.on_ipv4(nic, body, now),
                _ => {}
            }
        }
    }

    fn on_arp(&mut self, nic: &mut Nic, arp: &[u8], now: u64) {
        if arp.len() < 28 {
            return;
        }
        let operation = u16::from_be_bytes([arp[6], arp[7]]);
        let sender_mac: Mac = arp[8..14].try_into().unwrap_or(BROADCAST);
        let sender_ip: Ipv4 = arp[14..18].try_into().unwrap_or(ANY);
        let target_ip: Ipv4 = arp[24..28].try_into().unwrap_or(ANY);
        self.learn(sender_ip, sender_mac, now);
        if operation == 1 && target_ip == self.ip && self.ip != ANY {
            let mut reply = Vec::with_capacity(28);
            reply.extend_from_slice(&[0, 1]);
            reply.extend_from_slice(&ETHERTYPE_IPV4.to_be_bytes());
            reply.extend_from_slice(&[6, 4, 0, 2]); // "reply"
            reply.extend_from_slice(&self.mac);
            reply.extend_from_slice(&self.ip);
            reply.extend_from_slice(&sender_mac);
            reply.extend_from_slice(&sender_ip);
            let frame = self.frame(sender_mac, ETHERTYPE_ARP, &reply);
            if nic.send(&frame) {
                self.sent += 1;
                self.answered += 1;
            }
        }
    }

    fn on_ipv4(&mut self, nic: &mut Nic, packet: &[u8], now: u64) {
        if packet.len() < 20 || packet[0] >> 4 != 4 {
            return;
        }
        let header = usize::from(packet[0] & 0x0F) * 4;
        let total = usize::from(u16::from_be_bytes([packet[2], packet[3]])).min(packet.len());
        if header < 20 || total < header {
            return;
        }
        let from: Ipv4 = packet[12..16].try_into().unwrap_or(ANY);
        let to: Ipv4 = packet[16..20].try_into().unwrap_or(ANY);
        let body = &packet[header..total];
        // Anything not addressed to us, and not a broadcast, is none of our
        // business: the card takes broadcasts as well as our own address.
        if self.ip != ANY && to != self.ip && to != [255, 255, 255, 255] {
            return;
        }
        match packet[9] {
            PROTOCOL_ICMP => self.on_icmp(nic, from, body, now),
            PROTOCOL_UDP => {
                if body.len() >= 8 {
                    let from_port = u16::from_be_bytes([body[0], body[1]]);
                    let to_port = u16::from_be_bytes([body[2], body[3]]);
                    let length = usize::from(u16::from_be_bytes([body[4], body[5]])).clamp(8, body.len());
                    self.datagrams.push(Datagram {
                        from,
                        from_port,
                        to_port,
                        data: body[8..length].to_vec(),
                    });
                    if self.datagrams.len() > 8 {
                        self.datagrams.remove(0);
                    }
                }
            }
            PROTOCOL_TCP => {
                if body.len() >= 20 {
                    let offset = usize::from(body[12] >> 4) * 4;
                    if offset <= body.len() {
                        self.segments.push(Segment {
                            from,
                            from_port: u16::from_be_bytes([body[0], body[1]]),
                            to_port: u16::from_be_bytes([body[2], body[3]]),
                            sequence: u32::from_be_bytes([body[4], body[5], body[6], body[7]]),
                            acknowledged: u32::from_be_bytes([body[8], body[9], body[10], body[11]]),
                            flags: body[13],
                            data: body[offset..].to_vec(),
                        });
                        if self.segments.len() > 16 {
                            self.segments.remove(0);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn on_icmp(&mut self, nic: &mut Nic, from: Ipv4, icmp: &[u8], now: u64) {
        if icmp.len() < 8 {
            return;
        }
        match icmp[0] {
            // Somebody is pinging us: send the same bytes back as a reply.
            8 => {
                let mut reply = icmp.to_vec();
                reply[0] = 0;
                reply[2..4].copy_from_slice(&[0, 0]);
                let sum = checksum(&[&reply]);
                reply[2..4].copy_from_slice(&sum.to_be_bytes());
                if self.send_ipv4(nic, from, PROTOCOL_ICMP, &reply, now) {
                    self.answered += 1;
                }
            }
            // Our own ping came back.
            0 => self.pong += 1,
            _ => {}
        }
    }
}

/// The twelve bytes UDP and TCP add to their checksum so that it covers the
/// addresses too.
fn pseudo_header(from: Ipv4, to: Ipv4, protocol: u8, length: usize) -> Vec<u8> {
    let mut pseudo = Vec::with_capacity(12);
    pseudo.extend_from_slice(&from);
    pseudo.extend_from_slice(&to);
    pseudo.push(0);
    pseudo.push(protocol);
    pseudo.extend_from_slice(&(length as u16).to_be_bytes());
    pseudo
}
