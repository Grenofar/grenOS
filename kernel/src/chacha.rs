//! ChaCha20 and Poly1305, and the AEAD that TLS 1.3 makes of the two
//! (RFC 8439). Chosen over AES-GCM because it is arithmetic only: no tables,
//! no processor instructions, nothing that changes behaviour between the
//! machine that tests it and the machine that runs it.

use alloc::vec::Vec;

/// One ChaCha20 block: the state, twenty rounds, added back to itself.
fn block(key: &[u8; 32], counter: u32, nonce: &[u8; 12]) -> [u8; 64] {
    let mut state = [0u32; 16];
    state[0] = 0x6170_7865;
    state[1] = 0x3320_646e;
    state[2] = 0x7962_2d32;
    state[3] = 0x6b20_6574;
    for (slot, chunk) in state[4..12].iter_mut().zip(key.chunks_exact(4)) {
        *slot = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    state[12] = counter;
    for (slot, chunk) in state[13..16].iter_mut().zip(nonce.chunks_exact(4)) {
        *slot = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    let mut working = state;
    for _ in 0..10 {
        quarter(&mut working, 0, 4, 8, 12);
        quarter(&mut working, 1, 5, 9, 13);
        quarter(&mut working, 2, 6, 10, 14);
        quarter(&mut working, 3, 7, 11, 15);
        quarter(&mut working, 0, 5, 10, 15);
        quarter(&mut working, 1, 6, 11, 12);
        quarter(&mut working, 2, 7, 8, 13);
        quarter(&mut working, 3, 4, 9, 14);
    }
    let mut out = [0u8; 64];
    for (i, chunk) in out.chunks_exact_mut(4).enumerate() {
        chunk.copy_from_slice(&working[i].wrapping_add(state[i]).to_le_bytes());
    }
    out
}

fn quarter(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(16);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(12);
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(8);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(7);
}

/// Exclusive-ors `data` with the key stream, starting at `counter`.
pub fn apply(key: &[u8; 32], counter: u32, nonce: &[u8; 12], data: &mut [u8]) {
    for (index, piece) in data.chunks_mut(64).enumerate() {
        let stream = block(key, counter + index as u32, nonce);
        for (byte, key_byte) in piece.iter_mut().zip(stream) {
            *byte ^= key_byte;
        }
    }
}

/// Poly1305, RFC 8439: a one-time authenticator over 130-bit arithmetic,
/// carried here in five 26-bit limbs.
struct Poly1305 {
    r: [u32; 5],
    h: [u32; 5],
    pad: [u32; 4],
    buffer: [u8; 16],
    held: usize,
}

impl Poly1305 {
    fn new(key: &[u8; 32]) -> Self {
        let word = |at: usize| u32::from_le_bytes([key[at], key[at + 1], key[at + 2], key[at + 3]]);
        let r = [
            word(0) & 0x03ff_ffff,
            (word(3) >> 2) & 0x03ff_ff03,
            (word(6) >> 4) & 0x03ff_c0ff,
            (word(9) >> 6) & 0x03f0_3fff,
            (word(12) >> 8) & 0x000f_ffff,
        ];
        Poly1305 { r, h: [0; 5], pad: [word(16), word(20), word(24), word(28)], buffer: [0; 16], held: 0 }
    }

    fn block(&mut self, chunk: &[u8; 16], last: bool) {
        let word = |at: usize| u32::from_le_bytes([chunk[at], chunk[at + 1], chunk[at + 2], chunk[at + 3]]);
        let high = if last { 0 } else { 1 << 24 };
        let mut h = [
            self.h[0] + (word(0) & 0x03ff_ffff),
            self.h[1] + ((word(3) >> 2) & 0x03ff_ffff),
            self.h[2] + ((word(6) >> 4) & 0x03ff_ffff),
            self.h[3] + ((word(9) >> 6) & 0x03ff_ffff),
            self.h[4] + ((word(12) >> 8) | high),
        ];
        let r = self.r;
        let s = [0u32, r[1] * 5, r[2] * 5, r[3] * 5, r[4] * 5];
        let mul = |a: u32, b: u32| u64::from(a) * u64::from(b);
        let d0 = mul(h[0], r[0]) + mul(h[1], s[4]) + mul(h[2], s[3]) + mul(h[3], s[2]) + mul(h[4], s[1]);
        let d1 = mul(h[0], r[1]) + mul(h[1], r[0]) + mul(h[2], s[4]) + mul(h[3], s[3]) + mul(h[4], s[2]);
        let d2 = mul(h[0], r[2]) + mul(h[1], r[1]) + mul(h[2], r[0]) + mul(h[3], s[4]) + mul(h[4], s[3]);
        let d3 = mul(h[0], r[3]) + mul(h[1], r[2]) + mul(h[2], r[1]) + mul(h[3], r[0]) + mul(h[4], s[4]);
        let d4 = mul(h[0], r[4]) + mul(h[1], r[3]) + mul(h[2], r[2]) + mul(h[3], r[1]) + mul(h[4], r[0]);
        let mut carry = d0 >> 26;
        h[0] = (d0 & 0x03ff_ffff) as u32;
        let d1 = d1 + carry;
        carry = d1 >> 26;
        h[1] = (d1 & 0x03ff_ffff) as u32;
        let d2 = d2 + carry;
        carry = d2 >> 26;
        h[2] = (d2 & 0x03ff_ffff) as u32;
        let d3 = d3 + carry;
        carry = d3 >> 26;
        h[3] = (d3 & 0x03ff_ffff) as u32;
        let d4 = d4 + carry;
        carry = d4 >> 26;
        h[4] = (d4 & 0x03ff_ffff) as u32;
        h[0] += (carry as u32) * 5;
        carry = u64::from(h[0] >> 26);
        h[0] &= 0x03ff_ffff;
        h[1] += carry as u32;
        self.h = h;
    }

    fn update(&mut self, mut data: &[u8]) {
        if self.held > 0 {
            let take = (16 - self.held).min(data.len());
            self.buffer[self.held..self.held + take].copy_from_slice(&data[..take]);
            self.held += take;
            data = &data[take..];
            if self.held < 16 {
                // As in the hash: the tail below rewrites `held`, so a partial
                // block has to leave now or it is lost.
                return;
            }
            let chunk = self.buffer;
            self.block(&chunk, false);
            self.held = 0;
        }
        let mut chunks = data.chunks_exact(16);
        for chunk in &mut chunks {
            let mut block = [0u8; 16];
            block.copy_from_slice(chunk);
            self.block(&block, false);
        }
        let rest = chunks.remainder();
        self.buffer[..rest.len()].copy_from_slice(rest);
        self.held = rest.len();
    }

    fn finish(mut self) -> [u8; 16] {
        if self.held > 0 {
            let held = self.held;
            self.buffer[held] = 1;
            for byte in self.buffer.iter_mut().skip(held + 1) {
                *byte = 0;
            }
            let chunk = self.buffer;
            self.block(&chunk, true);
        }
        let mut h = self.h;
        let mut carry = h[1] >> 26;
        h[1] &= 0x03ff_ffff;
        for limb in h.iter_mut().skip(2) {
            *limb += carry;
            carry = *limb >> 26;
            *limb &= 0x03ff_ffff;
        }
        h[0] += carry * 5;
        carry = h[0] >> 26;
        h[0] &= 0x03ff_ffff;
        h[1] += carry;

        // h + 5, which tells whether h has reached p: five is what is left of
        // 2^130 once p is taken away.
        let mut g = [0u32; 5];
        let mut carry = 5u32;
        for index in 0..4 {
            let value = h[index] + carry;
            g[index] = value & 0x03ff_ffff;
            carry = value >> 26;
        }
        let g4 = h[4].wrapping_add(carry).wrapping_sub(1 << 26);
        g[4] = g4;
        // The top bit of that last limb is its sign. Set means h + 5 stayed
        // below 2^130, so h was already smaller than p and stands; clear means
        // h had passed p and g is the reduced value. Chosen by mask, never by
        // a branch: the time this takes must not depend on the secret.
        let mask = ((g4 >> 31) & 1).wrapping_sub(1);
        for index in 0..5 {
            h[index] = (h[index] & !mask) | (g[index] & mask);
        }

        let mut out = [0u32; 4];
        out[0] = h[0] | (h[1] << 26);
        out[1] = (h[1] >> 6) | (h[2] << 20);
        out[2] = (h[2] >> 12) | (h[3] << 14);
        out[3] = (h[3] >> 18) | (h[4] << 8);
        let mut tag = [0u8; 16];
        let mut carry = 0u64;
        for (index, chunk) in tag.chunks_exact_mut(4).enumerate() {
            let sum = u64::from(out[index]) + u64::from(self.pad[index]) + carry;
            carry = sum >> 32;
            chunk.copy_from_slice(&(sum as u32).to_le_bytes());
        }
        tag
    }
}

/// The tag over the additional data and the ciphertext, as RFC 8439 lays it out.
fn tag(key: &[u8; 32], aad: &[u8], ciphertext: &[u8]) -> [u8; 16] {
    let mut poly = Poly1305::new(key);
    poly.update(aad);
    poly.update(&[0u8; 16][..(16 - aad.len() % 16) % 16]);
    poly.update(ciphertext);
    poly.update(&[0u8; 16][..(16 - ciphertext.len() % 16) % 16]);
    let mut lengths = [0u8; 16];
    lengths[..8].copy_from_slice(&(aad.len() as u64).to_le_bytes());
    lengths[8..].copy_from_slice(&(ciphertext.len() as u64).to_le_bytes());
    poly.update(&lengths);
    poly.finish()
}

/// The one-time key Poly1305 uses, from the cipher's own key stream.
fn one_time(key: &[u8; 32], nonce: &[u8; 12]) -> [u8; 32] {
    let stream = block(key, 0, nonce);
    let mut out = [0u8; 32];
    out.copy_from_slice(&stream[..32]);
    out
}

/// Encrypts in place and returns the tag.
pub fn seal(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8]) -> [u8; 16] {
    let poly_key = one_time(key, nonce);
    apply(key, 1, nonce, data);
    tag(&poly_key, aad, data)
}

/// Checks the tag and decrypts. None when the tag does not match — and then
/// the plaintext is never handed back, which is the whole point.
pub fn open(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 16 {
        return None;
    }
    let (ciphertext, given) = data.split_at(data.len() - 16);
    let poly_key = one_time(key, nonce);
    let expected = tag(&poly_key, aad, ciphertext);
    // Compared without an early exit: a comparison that stops at the first
    // wrong byte tells an attacker how far they got.
    let mut same = 0u8;
    for (a, b) in expected.iter().zip(given) {
        same |= a ^ b;
    }
    if same != 0 {
        return None;
    }
    let mut plain = ciphertext.to_vec();
    apply(key, 1, nonce, &mut plain);
    Some(plain)
}
