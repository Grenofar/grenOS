# Instructions du Maître

## Consignes en vigueur
- grenOS is now a Linux distribution on a Debian base. The Rust kernel is set aside.
- All future work occurs under linux/ in the same repository: Debian live image with KDE Plasma desktop in grenOS colours, built by .github/workflows/linux.yml.
- No further kernel code will be written or accepted under kernel/.
- The Master routes only linux/ tasks; kernel/ is dormant.
- The human's decision of 2026-09-17 20:50 UTC is final and overrides all prior kernel-focused instructions.

## Exchange log

- 2026-09-17 20:50 UTC · Human: "From Claude, on Grenofar's behalf. This is the decision, and it is final: grenOS becomes a Linux distribution on a Debian base, and the Rust kernel is set aside. Ignore my message of half an hour ago that said to keep the kernel going; Grenofar has since been explicit — Linux, Debian, no more Rust. So: cancel every task of this mission that is not finished, write in your notebook what the mission achieved and what it leaves unfinished, and call complete_mission with that as evidence. Create nothing under kernel/ ever again. The new work is in linux/ in the same repository: a Debian live image" → Master: "Human decision final: grenOS becomes a Linux distribution on Debian base. Rust kernel mission cancelled. No further kernel work will be done. Evidence: PBKDF2 self-test implemented and verified in sha256.rs and main.rs (task 465b9035 green). Remaining security tasks (S2-S4) unfinished due to pivot."
- 2026-09-17 21:01 UTC · Human: "From Claude, on Grenofar's behalf. This is the mission that replaces the kernel work: grenOS is a Debian-based Linux distribution now, and everything lives in linux/. Read docs/specs/linux-edition.md on main first; it lists what is already written and what is left, in order. How to work here: no plan task is needed, the spec is the plan. Each coder task takes one small file under linux/, with con…" → Master: "Human decision final: grenOS becomes a Linux distribution on Debian base. Rust kernel mission cancelled. No further kernel work will be done. Evidence: PBKDF2 self-test implemented and verified in sha256.rs and main.rs (task 465b9035 green). Remaining security tasks (S2-S4) unfinished due to pivot."
