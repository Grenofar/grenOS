//! Ed25519 signature verification (RFC 8032 §5.1.7): the check that a kernel
//! offered as an update was signed by the release workflow's key
//! (docs/specs/disk-and-updates.md §5).
//!
//! It follows TweetNaCl's `crypto_sign_open`, over the field of `x25519.rs`:
//! decode the public key as a point and negate it, hash R, A and the message,
//! compute [S]B - [h]A, and compare its encoding with R. Verification only:
//! no secret is ever handled here. The curve constants were derived exactly
//! (d = -121665/121666, sqrt(-1) = 2^((p-1)/4), the base point from y = 4/5),
//! and the RFC's test vectors are checked at boot.

use crate::sha512::Sha512;
use crate::x25519::{Field, ONE, ZERO, add, conditional_swap, invert, multiply, pack, square, subtract, unpack};

const D: Field = [0x78a3, 0x1359, 0x4dca, 0x75eb, 0xd8ab, 0x4141, 0x0a4d, 0x0070, 0xe898, 0x7779, 0x4079, 0x8cc7, 0xfe73, 0x2b6f, 0x6cee, 0x5203];
const D2: Field = [0xf159, 0x26b2, 0x9b94, 0xebd6, 0xb156, 0x8283, 0x149a, 0x00e0, 0xd130, 0xeef3, 0x80f2, 0x198e, 0xfce7, 0x56df, 0xd9dc, 0x2406];
const SQRT_M1: Field = [0xa0b0, 0x4a0e, 0x1b27, 0xc4ee, 0xe478, 0xad2f, 0x1806, 0x2f43, 0xd7a7, 0x3dfb, 0x0099, 0x2b4d, 0xdf0b, 0x4fc1, 0x2480, 0x2b83];
const BASE_X: Field = [0xd51a, 0x8f25, 0x2d60, 0xc956, 0xa7b2, 0x9525, 0xc760, 0x692c, 0xdc5c, 0xfdd6, 0xe231, 0xc0a4, 0x53fe, 0xcd6e, 0x36d3, 0x2169];
const BASE_Y: Field = [0x6658, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666];
const L: [i64; 32] = [0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10];

/// A point in extended coordinates (X, Y, Z, T).
type Point = [Field; 4];

fn plus(a: &Field, b: &Field) -> Field {
    let mut out = ZERO;
    add(&mut out, a, b);
    out
}

fn minus(a: &Field, b: &Field) -> Field {
    let mut out = ZERO;
    subtract(&mut out, a, b);
    out
}

fn times(a: &Field, b: &Field) -> Field {
    let mut out = ZERO;
    multiply(&mut out, a, b);
    out
}

fn squared(a: &Field) -> Field {
    let mut out = ZERO;
    square(&mut out, a);
    out
}

fn packed(a: &Field) -> [u8; 32] {
    let mut out = [0u8; 32];
    pack(&mut out, a);
    out
}

fn parity(a: &Field) -> u8 {
    packed(a)[0] & 1
}

fn differ(a: &Field, b: &Field) -> bool {
    packed(a) != packed(b)
}

/// a^((p - 5) / 8), for the square root in point decoding.
fn pow2523(a: &Field) -> Field {
    let mut c = *a;
    for bit in (0..=250).rev() {
        c = squared(&c);
        if bit != 1 {
            c = times(&c, a);
        }
    }
    c
}

/// p = p + q, in place.
fn point_add(p: &mut Point, q: &Point) {
    let a = times(&minus(&p[1], &p[0]), &minus(&q[1], &q[0]));
    let b = times(&plus(&p[0], &p[1]), &plus(&q[0], &q[1]));
    let c = times(&times(&p[3], &q[3]), &D2);
    let zz = times(&p[2], &q[2]);
    let d = plus(&zz, &zz);
    let e = minus(&b, &a);
    let f = minus(&d, &c);
    let g = plus(&d, &c);
    let h = plus(&b, &a);
    p[0] = times(&e, &f);
    p[1] = times(&h, &g);
    p[2] = times(&g, &f);
    p[3] = times(&e, &h);
}

fn point_swap(p: &mut Point, q: &mut Point, bit: u8) {
    for (a, b) in p.iter_mut().zip(q.iter_mut()) {
        conditional_swap(a, b, i64::from(bit));
    }
}

/// [s]q, with q consumed as working space.
fn scalar_multiply(q: &mut Point, s: &[u8; 32]) -> Point {
    let mut p: Point = [ZERO, ONE, ONE, ZERO];
    for index in (0..256).rev() {
        let bit = (s[index / 8] >> (index & 7)) & 1;
        point_swap(&mut p, q, bit);
        point_add(q, &p);
        let copy = p;
        point_add(&mut p, &copy);
        point_swap(&mut p, q, bit);
    }
    p
}

fn scalar_base(s: &[u8; 32]) -> Point {
    let mut base: Point = [BASE_X, BASE_Y, ONE, times(&BASE_X, &BASE_Y)];
    scalar_multiply(&mut base, s)
}

fn encode(p: &Point) -> [u8; 32] {
    let mut inverse = ZERO;
    invert(&mut inverse, &p[2]);
    let x = times(&p[0], &inverse);
    let y = times(&p[1], &inverse);
    let mut out = packed(&y);
    out[31] ^= parity(&x) << 7;
    out
}

/// The point -A for the encoded public key A, or None when the bytes are not
/// a point of the curve.
fn decode_negated(bytes: &[u8; 32]) -> Option<Point> {
    let mut y = ZERO;
    unpack(&mut y, bytes);
    let z = ONE;
    let y2 = squared(&y);
    let den = plus(&z, &times(&y2, &D));
    let num = minus(&y2, &z);
    let den2 = squared(&den);
    let den4 = squared(&den2);
    let den6 = times(&den4, &den2);
    let mut t = times(&times(&den6, &num), &den);
    t = pow2523(&t);
    t = times(&times(&times(&t, &num), &den), &den);
    let mut x = times(&t, &den);
    if differ(&times(&squared(&x), &den), &num) {
        x = times(&x, &SQRT_M1);
    }
    if differ(&times(&squared(&x), &den), &num) {
        return None;
    }
    if parity(&x) == bytes[31] >> 7 {
        x = minus(&ZERO, &x);
    }
    Some([x, y, z, times(&x, &y)])
}

/// Reduces a 512-bit little-endian number modulo L.
fn reduce(hash: &[u8; 64]) -> [u8; 32] {
    let mut x = [0i64; 64];
    for (value, byte) in x.iter_mut().zip(hash) {
        *value = i64::from(*byte);
    }
    for i in (32..64).rev() {
        let mut carry = 0i64;
        let mut j = i - 32;
        while j < i - 12 {
            x[j] += carry - 16 * x[i] * L[j - (i - 32)];
            carry = (x[j] + 128) >> 8;
            x[j] -= carry << 8;
            j += 1;
        }
        x[j] += carry;
        x[i] = 0;
    }
    let mut carry = 0i64;
    let top = x[31] >> 4;
    for (value, l) in x.iter_mut().zip(L) {
        *value += carry - top * l;
        carry = *value >> 8;
        *value &= 255;
    }
    for (value, l) in x.iter_mut().zip(L) {
        *value -= carry * l;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        x[i + 1] += x[i] >> 8;
        out[i] = (x[i] & 255) as u8;
    }
    out
}

/// Whether the 32 little-endian bytes of `s` are below L, as RFC 8032 requires
/// of a signature's S: accepting S + L would make signatures malleable.
fn canonical(s: &[u8]) -> bool {
    for (byte, l) in s.iter().zip(L).rev() {
        let l = l as u8;
        if *byte != l {
            return *byte < l;
        }
    }
    false
}

/// Whether `signature` is a valid Ed25519 signature of `message` by the key
/// `public_key`. Never panics: bytes that are not a key or not a signature
/// are simply not valid.
pub fn verify(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
    let (r, s) = signature.split_at(32);
    if !canonical(s) {
        return false;
    }
    let Some(mut minus_a) = decode_negated(public_key) else {
        return false;
    };
    let mut hash = Sha512::new();
    hash.update(r);
    hash.update(public_key);
    hash.update(message);
    let h = reduce(&hash.finish());
    let mut s_bytes = [0u8; 32];
    s_bytes.copy_from_slice(s);

    let mut point = scalar_multiply(&mut minus_a, &h);
    point_add(&mut point, &scalar_base(&s_bytes));
    let mut same = 0u8;
    for (a, b) in encode(&point).iter().zip(r) {
        same |= a ^ b;
    }
    same == 0
}

/// Hex text to bytes, for the fixed test values below.
fn hex<const N: usize>(text: &str) -> [u8; N] {
    let mut out = [0u8; N];
    for (byte, pair) in out.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        let digit = |c: u8| match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            _ => 0,
        };
        *byte = digit(pair[0]) << 4 | digit(pair[1]);
    }
    out
}

/// The FIPS 180-4 SHA-512 example and RFC 8032's first three Ed25519 test
/// vectors (§7.1), plus a forged one that must fail. Run at boot: the check
/// every update goes through is itself checked on every machine.
pub fn self_test() -> bool {
    let abc: [u8; 64] = hex(
        "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
    );
    if crate::sha512::sha512(b"abc") != abc {
        return false;
    }
    let vectors: [(&str, &[u8], &str); 3] = [
        (
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
            b"",
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
        ),
        (
            "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
            &[0x72],
            "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
        ),
        (
            "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
            &[0xaf, 0x82],
            "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
        ),
    ];
    for (key, message, signature) in vectors {
        if !verify(&hex(key), message, &hex(signature)) {
            return false;
        }
    }
    let (key, message, signature) = vectors[1];
    let mut forged: [u8; 64] = hex(signature);
    forged[0] ^= 1;
    !verify(&hex(key), message, &forged)
}
