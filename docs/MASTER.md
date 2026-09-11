# Instructions du Maître

## Consignes en vigueur
- Le build doit passer avec cargo build --release et zéro avertissement clippy.
- Produire une image bootable avec Limine qui affiche 'grenOS' sur le port série COM1 et s'arrête proprement.

## Standing instructions
- Merge remaining phases into one Coder task delivering the whole minimal boot, continuing from f58a45fa; ensure asm! calls are inside unsafe blocks.
- Fix docs/PLAN.md to note that asm! must be inside an unsafe block.

## Exchange log

- 2026-09-10 19:54 UTC · Humain : « Le build passe. Continue : corrige clippy avec un hlt dans la boucle, puis produis l'image bootable avec Limine. » → Maître : « Manifest fix accepted; now fixing clippy and adding serial output. »
- 2026-09-11 05:05 UTC · Human: "docs/PLAN.md est faux : identifiants Limine inventés, core::arch::x86_64::hlt/outb/inb n'existent pas, limine.cfg, cible JSON. Fais-le réécrire par l'Architecte d'après le limine-rust-template (crate limine 0.5, son linker script, son limine.conf, son GNUmakefile) et le contrat CI. Ensuite le Codeur repart de 4b7d4930 (continue_from). / docs/PLAN.md is wrong: invented Limine IDs, core::arch::x86_64::hlt/outb/inb do not exist, limine.cfg, custom target JSON. Have the Architect rewrite it from the limine-rust-template and the CI contract first. Then the Coder continues from 4b7d4930." → Master: "I have asked the Architect to rewrite docs/PLAN.md based on the limine-rust-template and CI contract. Once that is done, the Coder will continue from task 4b7d4930."
- 2026-09-11 13:54 UTC · Human: ""CI is green only when the kernel boots and prints grenOS, so a task that stops at project setup can never pass. Merge the remaining phases into one Coder task that delivers the whole minimal boot, continuing from f58a45fa (it builds with limine 0.5; only the two asm! calls need an unsafe block). Also fix docs/PLAN.md: asm! must be inside unsafe."" → Master: "Updated MASTER.md with the human's instruction and created two Coder tasks: one to deliver the minimal boot from f58a45fa, another to fix docs/PLAN.md regarding unsafe asm!."
- 2026-09-11 14:20 UTC · Human: "From Claude, on Grenofar's behalf. I checked the Coder's work against the real CI logs. 1. a5780922 is the branch that got furthest: build and clippy passed, and only the image failed. Its make-iso.sh runs `cargo build` from the repository root ("could not find Cargo.toml"): CI runs the script from the root, and until today its output never reached you. Everything else on a5780922 is right: main.…" → Master: "The task was to deliver the whole minimal boot, but the resulting image still does not boot due to issues in limine.conf, make-iso.sh, and serial.rs (identity_op)."
