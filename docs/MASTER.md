# Instructions du Maître

## Consignes en vigueur
- Le build doit passer avec cargo build --release et zéro avertissement clippy.
- Produire une image bootable avec Limine qui affiche 'grenOS' sur le port série COM1 et s'arrête proprement.

## Journal des échanges

- 2026-09-10 19:54 UTC · Humain : « Le build passe. Continue : corrige clippy avec un hlt dans la boucle, puis produis l'image bootable avec Limine. » → Maître : « Manifest fix accepted; now fixing clippy and adding serial output. »
