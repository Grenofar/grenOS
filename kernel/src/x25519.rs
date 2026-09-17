//! X25519, the key agreement of RFC 7748: the two sides each keep a secret,
//! exchange a public value, and end up with the same shared bytes without
//! ever sending them.
//!
//! The arithmetic follows TweetNaCl's shape — sixteen limbs of sixteen bits
//! in signed 64-bit words — because it is short enough to read in one sitting
//! and its behaviour does not depend on the values it handles. Ed25519
//! (ed25519.rs) uses the same field, so its arithmetic is shared.

pub(crate) type Field = [i64; 16];

pub(crate) const ZERO: Field = [0; 16];
pub(crate) const ONE: Field = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
/// 121665, the constant of the curve's ladder.
const A24: Field = [0xDB41, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

pub(crate) fn add(out: &mut Field, a: &Field, b: &Field) {
    for i in 0..16 {
        out[i] = a[i] + b[i];
    }
}

pub(crate) fn subtract(out: &mut Field, a: &Field, b: &Field) {
    for i in 0..16 {
        out[i] = a[i] - b[i];
    }
}

/// Carries each limb back into sixteen bits, folding the overflow back in
/// through 2^255 = 19.
fn carry(field: &mut Field) {
    for i in 0..16 {
        let value = field[i] + (1 << 16);
        let carried = value >> 16;
        field[(i + 1) * usize::from(i < 15)] += carried - 1 + 37 * (carried - 1) * i64::from(i == 15);
        field[i] = value - carried * (1 << 16);
    }
}

pub(crate) fn multiply(out: &mut Field, a: &Field, b: &Field) {
    let mut product = [0i64; 31];
    for i in 0..16 {
        for j in 0..16 {
            product[i + j] += a[i] * b[j];
        }
    }
    for i in 0..15 {
        product[i] += 38 * product[i + 16];
    }
    out[..16].copy_from_slice(&product[..16]);
    carry(out);
    carry(out);
}

pub(crate) fn square(out: &mut Field, a: &Field) {
    let copy = *a;
    multiply(out, &copy, &copy);
}

/// Swaps `p` and `q` when `swap` is one, without a branch on the secret.
pub(crate) fn conditional_swap(p: &mut Field, q: &mut Field, swap: i64) {
    let mask = !(swap - 1);
    for i in 0..16 {
        let difference = mask & (p[i] ^ q[i]);
        p[i] ^= difference;
        q[i] ^= difference;
    }
}

/// The inverse, by raising to p - 2.
pub(crate) fn invert(out: &mut Field, a: &Field) {
    let mut c = *a;
    for i in (0..=253).rev() {
        let copy = c;
        square(&mut c, &copy);
        if i != 2 && i != 4 {
            let copy = c;
            multiply(&mut c, &copy, a);
        }
    }
    *out = c;
}

pub(crate) fn unpack(out: &mut Field, bytes: &[u8; 32]) {
    for i in 0..16 {
        out[i] = i64::from(bytes[2 * i]) + (i64::from(bytes[2 * i + 1]) << 8);
    }
    out[15] &= 0x7fff;
}

pub(crate) fn pack(out: &mut [u8; 32], field: &Field) {
    let mut value = *field;
    carry(&mut value);
    carry(&mut value);
    carry(&mut value);
    // Two conditional subtractions of p bring it into range.
    for _ in 0..2 {
        let mut minus = [0i64; 16];
        minus[0] = value[0] - 0xffed;
        for i in 1..15 {
            minus[i] = value[i] - 0xffff - ((minus[i - 1] >> 16) & 1);
            minus[i - 1] &= 0xffff;
        }
        minus[15] = value[15] - 0x7fff - ((minus[14] >> 16) & 1);
        let borrowed = (minus[15] >> 16) & 1;
        minus[14] &= 0xffff;
        conditional_swap(&mut value, &mut minus, 1 - borrowed);
    }
    for i in 0..16 {
        out[2 * i] = (value[i] & 0xff) as u8;
        out[2 * i + 1] = (value[i] >> 8) as u8;
    }
}

/// The Montgomery ladder: `secret` times the point `point`.
pub fn scalar_multiply(secret: &[u8; 32], point: &[u8; 32]) -> [u8; 32] {
    let mut clamped = *secret;
    clamped[0] &= 248;
    clamped[31] &= 127;
    clamped[31] |= 64;

    let mut x = ZERO;
    unpack(&mut x, point);
    let mut a = ONE;
    let mut b = x;
    let mut c = ZERO;
    let mut d = ONE;
    let (mut e, mut f) = (ZERO, ZERO);

    // The ladder, one bit of the secret at a time. Each line below is one
    // operation of the reference implementation, in its order and with its
    // destination: getting a single destination wrong gives a result that
    // looks like a key and agrees with nobody.
    let mut t = ZERO;
    for i in (0..255).rev() {
        let bit = i64::from((clamped[i >> 3] >> (i & 7)) & 1);
        conditional_swap(&mut a, &mut b, bit);
        conditional_swap(&mut c, &mut d, bit);
        add(&mut e, &a, &c); //            e = a + c
        subtract(&mut t, &a, &c); //       a = a - c
        a = t;
        add(&mut t, &b, &d); //            c = b + d, from the old b and d
        let sum = t;
        subtract(&mut t, &b, &d); //       b = b - d
        b = t;
        c = sum;
        square(&mut d, &e); //             d = e²
        square(&mut f, &a); //             f = a²
        multiply(&mut t, &c, &a); //       a = c * a
        a = t;
        multiply(&mut t, &b, &e); //       c = b * e
        c = t;
        add(&mut t, &a, &c); //            e = a + c
        e = t;
        subtract(&mut t, &a, &c); //       a = a - c
        a = t;
        square(&mut b, &a); //             b = a²
        subtract(&mut t, &d, &f); //       c = d - f
        c = t;
        multiply(&mut t, &c, &A24); //     a = c * 121665
        a = t;
        add(&mut t, &a, &d); //            a = a + d
        a = t;
        multiply(&mut t, &c, &a); //       c = c * a
        c = t;
        multiply(&mut t, &d, &f); //       a = d * f
        a = t;
        multiply(&mut t, &b, &x); //       d = b * x
        d = t;
        square(&mut b, &e); //             b = e²
        conditional_swap(&mut a, &mut b, bit);
        conditional_swap(&mut c, &mut d, bit);
    }

    let mut inverse = ZERO;
    invert(&mut inverse, &c);
    let mut result = ZERO;
    multiply(&mut result, &a, &inverse);
    let mut out = [0u8; 32];
    pack(&mut out, &result);
    out
}

/// The public value that goes on the wire, for a secret of our own.
pub fn public(secret: &[u8; 32]) -> [u8; 32] {
    let mut base = [0u8; 32];
    base[0] = 9;
    scalar_multiply(secret, &base)
}

/// What both sides end up with.
pub fn shared(secret: &[u8; 32], peer: &[u8; 32]) -> [u8; 32] {
    scalar_multiply(secret, peer)
}
