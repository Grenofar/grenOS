# Spec — security hardening

Written by Claude for the agents, 2026-09-16, at the human's request: "above
all, security". Each item is small, checkable at boot, and has its proof line
on the serial port. Every value below was checked against the named source.
English, because models read it.

## 0. Tasks, in order

| # | Task | Files | Proof on serial |
|---|------|-------|-----------------|
| S1 | PBKDF2-HMAC-SHA256 | `kernel/src/sha256.rs` (add `pbkdf2`) , `main.rs` | `security: pbkdf2 verified against RFC 7914` |
| S2 | The session password stored as a salted hash, compared in constant time, wiped after use | `kernel/src/desktop.rs` (account and lock screen only), `kernel/src/security.rs` | `security: password stored as PBKDF2-SHA256, <n> iterations` |
| S3 | Login throttling | `kernel/src/desktop.rs` (lock screen only) | none at boot; see §3 |
| S4 | Physical memory map not executable | `kernel/src/security.rs`, `kernel/src/paging.rs` | `security: physical memory map non-executable` |

Each task ends with the six CI steps green (build, clippy, boot, screen,
input, net).

## 1. S1 — PBKDF2 (RFC 2898 §5.2, with HMAC-SHA-256)

`sha256.rs` already has `hmac(key, data) -> [u8; 32]`. Add:

```rust
/// PBKDF2 with HMAC-SHA-256 (RFC 2898 §5.2). Writes `out.len()` bytes.
pub fn pbkdf2(password: &[u8], salt: &[u8], iterations: u32, out: &mut [u8]);
```

`T_i = U_1 xor U_2 xor ... xor U_c`, `U_1 = HMAC(P, S || INT(i))` with `INT(i)`
the block index as a 4-byte big-endian integer starting at 1, `U_j =
HMAC(P, U_{j-1})`; the output is `T_1 || T_2 || ...` cut to `out.len()`.
Compute the HMAC key pads once per call, not once per iteration: 80 000
iterations must stay well under a second in QEMU.

Test vectors (RFC 7914 §11, recomputed with Python's `hashlib.pbkdf2_hmac`),
checked at boot in `main.rs`:

- P = `passwd`, S = `salt`, c = 1, dkLen = 64 →
  `55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc49ca9cccf179b645991664b39d77ef317c71b845b1e30bd509112041d3a19783`
- P = `Password`, S = `NaCl`, c = 80000, dkLen = 64 →
  `4ddcd8f60b98be21830cee5ef22701f9641a4418d04c0414aeff08876b34ab56a1d425a1225833549adb841b51c9b3176a272bdebba1d078478f62b397f33c8d`

## 2. S2 — the session password

Today `Desktop` keeps `password: Option<String>` in clear and compares it with
`==` (desktop.rs, the account section of Paramètres and the lock screen).

- Store instead a `Credential { salt: [u8; 16], iterations: u32, hash: [u8; 32] }`:
  salt from `rand::bytes()`, iterations `ITERATIONS = 100_000`, hash =
  `pbkdf2(password, salt, iterations)`.
- Compare with a loop that ORs the XOR of every byte and looks at the result
  once at the end — never an early exit.
- After hashing or checking, overwrite the typed characters before clearing
  the string (write zero bytes over its buffer, e.g. through
  `unsafe { s.as_bytes_mut() }.fill(0)` with a SAFETY comment: zeros are valid
  UTF-8), so the password does not linger in the heap.
- The Sécurité window lists the storage method.

## 3. S3 — throttling

On the lock screen, after 3 wrong passwords in a row, refuse any attempt for
5 s, then 10 s after the next failure, doubling up to 300 s; a right password
resets the count. The screen says "Trop d'essais : réessayez dans <n> s" and
counts down with the clock (`advance(ms)`), and typing is ignored while it
counts. The delay is measured with the desktop's own milliseconds, which
already drive the animations.

## 4. S4 — no execution in the physical memory map

Source: Intel SDM Vol. 3A §4.5 (4-level paging): bit 63 of a paging-structure
entry is XD when EFER.NXE = 1 (already set by `security::arm`), and an address
is not executable if XD is set in **any** entry on its walk.

Limine's higher-half direct map (HHDM, offset `frames.hhdm()`) maps all of RAM,
the heap and the back buffer included; code never runs from there — the kernel
image is mapped separately at 0xFFFF_FFFF_8000_0000. So:

1. For every PML4 entry whose range lies inside the HHDM (from `hhdm` to
   `hhdm + highest usable physical address`) and is present, set bit 63.
   Never touch the PML4 entry that maps the kernel image (index 511) or the
   device windows (`0xFFFF_B000_...`, `0xFFFF_B100_...`) or the probe page
   (`0xFFFF_9000_...`).
2. Reload CR3 (write back its own value) to flush the TLB.
3. Check with `paging::flags_of` on an address inside the heap that bit 63 is
   now reported, and print the proof line — or which PML4 index failed.

`flags_of` currently returns the flags of the last entry only: XD can be in a
higher level. Make it OR the XD bits of every level it walks, and document it.
