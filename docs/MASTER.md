# Instructions du Maître

## Consignes en vigueur
- Le build doit passer avec cargo build --release et zéro avertissement clippy.
- Produire une image bootable avec Limine qui affiche 'grenOS' sur le port série COM1 et s'arrête proprement.

## Exchange log

- 2026-09-10 19:54 UTC · Humain : « Le build passe. Continue : corrige clippy avec un hlt dans la boucle, puis produis l'image bootable avec Limine. » → Maître : « Manifest fix accepted; now fixing clippy and adding serial output. »
- 2026-09-11 05:05 UTC · Human: "docs/PLAN.md est faux : identifiants Limine inventés, core::arch::x86_64::hlt/outb/inb n'existent pas, limine.cfg, cible JSON. Fais-le réécrire par l'Architecte d'après le limine-rust-template (crate limine 0.5, son linker script, son limine.conf, son GNUmakefile) et le contrat CI. Ensuite le Codeur repart de 4b7d4930 (continue_from). / docs/PLAN.md is wrong: invented Limine IDs, core::arch::x86_64::hlt/outb/inb do not exist, limine.cfg, custom target JSON. Have the Architect rewrite it from the limine-rust-template and the CI contract first. Then the Coder continues from 4b7d4930." → Master: "I have asked the Architect to rewrite docs/PLAN.md based on the limine-rust-template and CI contract. Once that is done, the Coder will continue from task 4b7d4930."
