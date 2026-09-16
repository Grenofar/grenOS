//! Randomness, for the keys TLS makes up for every connection.
//!
//! The processor's own generator (RDRAND) when it has one; always mixed with
//! the time-stamp counter, the millisecond clock and a counter, through
//! SHA-256. QEMU's default processor has no RDRAND, and then the result is
//! only as unpredictable as the clock — good enough that two connections never
//! share a key, not good enough against someone who can watch the machine
//! boot. The Réseau section says which of the two this machine got.

use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::sha256::Sha256;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn rdtsc() -> u64 {
    let (high, low): (u32, u32);
    // SAFETY: RDTSC exists on every x86_64 processor and only reads a counter.
    unsafe { asm!("rdtsc", out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags)) };
    u64::from(high) << 32 | u64::from(low)
}

/// True when the processor offers RDRAND (CPUID leaf 1, ECX bit 30).
pub fn hardware() -> bool {
    // SAFETY: CPUID leaf 1 exists on every x86_64 processor and only reads.
    let leaf = unsafe { core::arch::x86_64::__cpuid(1) };
    leaf.ecx & (1 << 30) != 0
}

/// One draw from RDRAND, retried a few times as Intel recommends.
fn rdrand() -> Option<u64> {
    if !hardware() {
        return None;
    }
    for _ in 0..10 {
        let (value, ok): (u64, u8);
        // SAFETY: the processor said it has RDRAND; the instruction only
        // writes its result and the carry flag, which says whether it worked.
        unsafe {
            asm!("rdrand {v}", "setc {ok}", v = out(reg) value, ok = out(reg_byte) ok, options(nomem, nostack));
        }
        if ok == 1 {
            return Some(value);
        }
    }
    None
}

/// Thirty-two bytes nobody else should be able to predict.
pub fn bytes() -> [u8; 32] {
    let mut hash = Sha256::default();
    for _ in 0..4 {
        if let Some(draw) = rdrand() {
            hash.update(&draw.to_le_bytes());
        }
    }
    hash.update(&rdtsc().to_le_bytes());
    hash.update(&crate::events::millis().to_le_bytes());
    hash.update(&COUNTER.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    hash.update(&rdtsc().to_le_bytes());
    hash.finish()
}
