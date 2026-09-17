//! SHA-256, and the two things TLS builds on it: HMAC, and the key
//! derivation of RFC 5869 in the shape RFC 8446 asks for.
//!
//! Every piece here is checked against the test vectors of its own standard
//! before it is trusted — see `scratchpad/crypto_test`.

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98,
    0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
    0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8,
    0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819,
    0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
    0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
    0xc67178f2,
];

/// A hash being built: TLS needs the running hash of everything said so far.
#[derive(Clone)]
pub struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    held: usize,
    total: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Sha256 {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            buffer: [0; 64],
            held: 0,
            total: 0,
        }
    }
}

impl Sha256 {
    pub fn update(&mut self, mut data: &[u8]) {
        self.total += data.len() as u64;
        if self.held > 0 {
            let room = 64 - self.held;
            let take = room.min(data.len());
            self.buffer[self.held..self.held + take].copy_from_slice(&data[..take]);
            self.held += take;
            data = &data[take..];
            if self.held < 64 {
                // Still short of a block. Returning here matters: the tail of
                // this function sets `held` from what is left of `data`, and
                // falling through would throw away what was just buffered —
                // which made `finish` wait for a length it could never reach.
                return;
            }
            let block = self.buffer;
            self.compress(&block);
            self.held = 0;
        }
        let mut chunks = data.chunks_exact(64);
        for chunk in &mut chunks {
            let mut block = [0u8; 64];
            block.copy_from_slice(chunk);
            self.compress(&block);
        }
        let rest = chunks.remainder();
        self.buffer[..rest.len()].copy_from_slice(rest);
        self.held = rest.len();
    }

    pub fn finish(mut self) -> [u8; 32] {
        let bits = self.total * 8;
        self.update_raw(&[0x80]);
        while self.held != 56 {
            self.update_raw(&[0]);
        }
        self.update_raw(&bits.to_be_bytes());
        let mut out = [0u8; 32];
        for (chunk, word) in out.chunks_exact_mut(4).zip(self.state) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    /// Padding must not count towards the length.
    fn update_raw(&mut self, data: &[u8]) {
        let total = self.total;
        self.update(data);
        self.total = total;
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for (slot, chunk) in w.iter_mut().zip(block.chunks_exact(4)) {
            *slot = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ (!e & g);
            let t1 = h.wrapping_add(s1).wrapping_add(choose).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let major = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(major);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, value) in self.state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::default();
    hash.update(data);
    hash.finish()
}

/// HMAC-SHA256, RFC 2104.
pub fn hmac(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut block = [0u8; 64];
    if key.len() > 64 {
        block[..32].copy_from_slice(&sha256(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::default();
    let mut outer = Sha256::default();
    let mut pad = [0u8; 64];
    for (slot, byte) in pad.iter_mut().zip(block) {
        *slot = byte ^ 0x36;
    }
    inner.update(&pad);
    inner.update(data);
    let digest = inner.finish();
    for (slot, byte) in pad.iter_mut().zip(block) {
        *slot = byte ^ 0x5c;
    }
    outer.update(&pad);
    outer.update(&digest);
    outer.finish()
}

/// HKDF-Extract, RFC 5869.
pub fn extract(salt: &[u8], key: &[u8]) -> [u8; 32] {
    hmac(salt, key)
}

/// HKDF-Expand, RFC 5869, for outputs of at most 32 bytes — which is all
/// TLS 1.3 asks of it here.
pub fn expand(key: &[u8], info: &[u8], out: &mut [u8]) {
    let mut previous = [0u8; 32];
    let mut done = 0;
    let mut counter = 1u8;
    while done < out.len() {
        let mut round = alloc::vec::Vec::with_capacity(previous.len() + info.len() + 1);
        if counter > 1 {
            round.extend_from_slice(&previous);
        }
        round.extend_from_slice(info);
        round.push(counter);
        previous = hmac(key, &round);
        let take = (out.len() - done).min(32);
        out[done..done + take].copy_from_slice(&previous[..take]);
        done += take;
        counter += 1;
    }
}

/// HKDF-Expand-Label, the shape RFC 8446 wraps every derivation in.
pub fn expand_label(key: &[u8], label: &str, context: &[u8], out: &mut [u8]) {
    let mut info = alloc::vec::Vec::with_capacity(64);
    info.extend_from_slice(&(out.len() as u16).to_be_bytes());
    info.push((6 + label.len()) as u8);
    info.extend_from_slice(b"tls13 ");
    info.extend_from_slice(label.as_bytes());
    info.push(context.len() as u8);
    info.extend_from_slice(context);
    expand(key, &info, out);
}

/// Derive-Secret, RFC 8446: a label applied to a transcript hash.
pub fn derive(secret: &[u8; 32], label: &str, transcript: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    expand_label(secret, label, transcript, &mut out);
    out
}

/// PBKDF2 with HMAC-SHA-256 (RFC 2898 §5.2). Writes `out.len()` bytes.
///
/// `T_i = U_1 xor U_2 xor ... xor U_c`, `U_1 = HMAC(P, S || INT(i))` with
/// `INT(i)` the block index as a 4-byte big-endian integer starting at 1,
/// `U_j = HMAC(P, U_{j-1})`; the output is `T_1 || T_2 || ...` cut to
/// `out.len()`. The HMAC key pads are computed once per call, not once per
/// iteration, so 80 000 iterations stay well under a second in QEMU.
pub fn pbkdf2(password: &[u8], salt: &[u8], iterations: u32, out: &mut [u8]) {
    let mut block = [0u8; 64];
    if password.len() > 64 {
        block[..32].copy_from_slice(&sha256(password));
    } else {
        block[..password.len()].copy_from_slice(password);
    }
    let mut inner_pad = [0u8; 64];
    let mut outer_pad = [0u8; 64];
    for (slot, byte) in inner_pad.iter_mut().zip(block) {
        *slot = byte ^ 0x36;
    }
    for (slot, byte) in outer_pad.iter_mut().zip(block) {
        *slot = byte ^ 0x5c;
    }

    let mut index = 1u32;
    let mut done = 0;
    while done < out.len() {
        let mut u;
        let mut first = alloc::vec::Vec::with_capacity(4 + salt.len());
        first.extend_from_slice(&index.to_be_bytes());
        first.extend_from_slice(salt);
        let mut inner = Sha256::default();
        inner.update(&inner_pad);
        inner.update(&first);
        u = inner.finish();
        let mut outer = Sha256::default();
        outer.update(&outer_pad);
        outer.update(&u);
        u = outer.finish();
        let mut t = u;
        for _ in 1..iterations {
            let mut inner = Sha256::default();
            inner.update(&inner_pad);
            inner.update(&u);
            u = inner.finish();
            let mut outer = Sha256::default();
            outer.update(&outer_pad);
            outer.update(&u);
            u = outer.finish();
            for (slot, byte) in t.iter_mut().zip(u) {
                *slot ^= byte;
            }
        }
        let take = (out.len() - done).min(32);
        out[done..done + take].copy_from_slice(&t[..take]);
        done += take;
        index += 1;
    }
}

/// Checks PBKDF2 against the two RFC 7914 §11 test vectors.
pub fn pbkdf2_self_test() -> bool {
    let mut out = [0u8; 64];
    pbkdf2(b"passwd", b"salt", 1, &mut out);
    let expected1: [u8; 64] = [
        0x55, 0xac, 0x04, 0x6e, 0x56, 0xe3, 0x08, 0x9f, 0xec, 0x16, 0x91, 0xc2, 0x25, 0x44, 0xb6, 0x05,
        0xf9, 0x41, 0x85, 0x21, 0x6d, 0xde, 0x04, 0x65, 0xe6, 0x8b, 0x9d, 0x57, 0xc2, 0x0d, 0xac, 0xbc,
        0x49, 0xca, 0x9c, 0xcf, 0xf1, 0x79, 0xb6, 0x45, 0x99, 0x16, 0x64, 0xb3, 0x9d, 0x77, 0xef, 0x31,
        0x7c, 0x71, 0xb8, 0x45, 0xb1, 0xe3, 0x0b, 0xd5, 0x09, 0x11, 0x20, 0x41, 0xd3, 0xa1, 0x97, 0x83,
    ];
    if out != expected1 {
        return false;
    }
    pbkdf2(b"Password", b"NaCl", 80_000, &mut out);
    let expected2: [u8; 64] = [
        0x4d, 0xdc, 0xd8, 0xf6, 0x0b, 0x98, 0xbe, 0x21, 0x83, 0x0c, 0xee, 0x5e, 0xf2, 0x27, 0x01, 0xf9,
        0x64, 0x1a, 0x44, 0x18, 0xd0, 0x4c, 0x04, 0x14, 0xae, 0xff, 0x08, 0x87, 0x6b, 0x34, 0xab, 0x56,
        0xa1, 0xd4, 0x25, 0xa1, 0x22, 0x58, 0x33, 0x54, 0x9a, 0xdb, 0x84, 0x1b, 0x51, 0xc9, 0xb3, 0x17,
        0x6a, 0x27, 0x2b, 0xde, 0xbb, 0xa1, 0xd0, 0x78, 0x47, 0x8f, 0x62, 0xb3, 0x97, 0xf3, 0x3c, 0x8d,
    ];
    out == expected2
}
