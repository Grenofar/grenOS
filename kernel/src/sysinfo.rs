//! What the Poste de travail window says about the machine.

use alloc::string::String;
use core::arch::x86_64::{__cpuid, CpuidResult};

fn cpuid(leaf: u32) -> CpuidResult {
    // SAFETY: CPUID exists on every x86_64 processor and only reads.
    unsafe { __cpuid(leaf) }
}

/// The processor's name, from CPUID leaves 0x80000002 to 0x80000004, or its
/// vendor (leaf 0) when it has no name.
pub fn cpu_name() -> String {
    let mut bytes = [0u8; 48];
    if cpuid(0x8000_0000).eax >= 0x8000_0004 {
        for (chunk, leaf) in bytes.chunks_exact_mut(16).zip(0x8000_0002u32..) {
            let r = cpuid(leaf);
            for (word, value) in chunk.chunks_exact_mut(4).zip([r.eax, r.ebx, r.ecx, r.edx]) {
                word.copy_from_slice(&value.to_le_bytes());
            }
        }
    } else {
        let r = cpuid(0);
        for (word, value) in bytes.chunks_exact_mut(4).zip([r.ebx, r.edx, r.ecx]) {
            word.copy_from_slice(&value.to_le_bytes());
        }
    }
    let name = String::from_utf8_lossy(&bytes);
    String::from(name.trim_matches(|c: char| c == '\0' || c.is_whitespace()))
}
