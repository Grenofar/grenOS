# TASK.md — ce qui est planifié, dans l'ordre

> Tenu par Claude. Mis à jour le 2026-11-17 au soir. Une session qui reprend
> le travail lit d'abord ce fichier, puis `CLAUDE.md` (local).

## 0. En cours — la publication de la version grenos-ui

- Branche `claude/grenos-ui` (grenOS) : grenos-ui en Rust/Tauri — premier
  démarrage, Mises à jour, grenPerf, GrenLaunch (126 jeux, « Mes jeux Steam »),
  auto-update corrigé (rien à chaud, Flatpak, choix de l'OOBE appliqué).
- CI `linux` : passes déjà vertes → compilation Rust, clé du dépôt, paquet,
  essai de mise à jour. Trois pannes corrigées en route (garde des recherches,
  verif-commandes qui lisait le binaire, hook qui exigeait l'install à chaud).
- **Quand la CI est verte sur la branche** (run 44156326449, commit `efa6efa`) :
  fusionner dans `main` CE commit vérifié, pas la pointe de branche :
  `git checkout main && git merge --no-ff efa6efa && git push`. Le commit
  `64904e0` (logo + fond, local, non poussé) part ensuite sur la branche pour
  sa propre CI, puis une version suivante.
  La CI de `main` publie la Release (ISO + zip VirtualBox) et le dépôt APT.
- **Puis le site** : dépôt `grenos-web-download`, branche locale
  `claude/theme-grenos-ui` (thème grenos-ui + section « Dans cette version »),
  à fusionner dans `main` et pousser — Vercel déploie. Seulement APRÈS la
  Release, pour ne pas annoncer une version introuvable.
- Supabase (projet `tpqzhzuoyqpfairatdrw`, org « kaka », plan gratuit) se met
  en pause après ~7 jours sans activité : « Restore » dans le tableau de bord,
  données conservées. Plafond de dépense activé : jamais facturé.

## FAIT le 18 novembre vers 03:00 — version `linux-20261118-0233` PUBLIÉE

- Release GitHub (ISO 1,6 Go + zip VirtualBox), dépôt APT et catalogue publiés
  par la CI de `main` (run 44218497249, cinq preuves vertes).
- Site https://grenos.vercel.app en ligne avec le thème grenos-ui, la marque
  sans lettre et « Dans cette version ».
- PROCHAINE VERSION : la branche `claude/grenos-ui` (coins arrondis, ronds de
  couleur, fondus) — pousser, CI verte, fusionner dans `main`.

## (historique) 18 novembre — la publication

- Run 44191198865 : 4 preuves sur 5 vertes ; l'UEFI échouait parce que son
  essai coupait la machine à 90 s fixes, avant l'ouverture de la session.
  Corrigé (`8206a6b`) : il attend la preuve (240 s max) et écrit le temps mis.
- Branche `claude/grenos-ui-release` = la version vérifiée (`1d89c52`) + ce
  seul correctif : C'EST ELLE QU'ON FUSIONNE DANS `main` quand sa CI est
  verte (`git checkout main && git merge --no-ff claude/grenos-ui-release &&
  git push`), puis le site (`grenos-web-download`, branche
  `claude/theme-grenos-ui` → main).
- `claude/grenos-ui` porte en plus coins arrondis + fondus : sa CI tourne en
  parallèle ; elle fera la version suivante.

## 0bis. Résultat du run 44156326449 (fin vers 21:09) — À TRAITER EN PREMIER

- VERT : construction, démarrage BIOS (grenos-ui OOBE affiché en 1 s, 11 Entrée,
  « nom choisi, la machine est a ci@grenos », apparence obéie), mise à jour au
  démarrage, VirtualBox. grenos-ui marche sur la machine d'essai.
- ROUGE « sans matériel » : l'essai attendait l'écran du nom sans appuyer sur
  Entrée (le nom n'est plus le 1er écran). CORRIGÉ dans
  `scripts/ci-sans-materiel.sh` (commit local, non poussé).
- ROUGE « UEFI » : écran tout noir, rien sur la persistance. À diagnostiquer :
  `gh run download 44156326449 -n grenos-linux-captures`, lire
  `serial-uefi.log` et `uefi-juge.log`. Soupçon : grenos-ui (WebKit) sur le
  framebuffer UEFI de QEMU ; le lanceur rebascule sur Python après 120 s — voir
  si la ligne « premier : grenos-ui ne s est pas affiche » est présente.
- Ensuite : pousser la branche (fix sans-matériel + logo + fond), attendre la CI,
  puis fusionner et publier comme ci-dessous.

## 1. Demandé le 17 novembre au soir — le visuel

1. **Logo grenOS sans lettre** — [écrit], commit local `visuel` : une pousse
   (deux feuilles + un point de lumière) dans `icones.py` (`marque`, `feuille`),
   les icônes de grenos-ui, le SVG `marque` de `web/commun.js` (logos de
   l'OOBE) et `marque-pastille` du site. Reste : vérifier sur capture QEMU.
2. **Fond d'écran** — VALIDÉ PAR GRENOFAR (commit local `4824a10`, « garde
   ça ») : nuit = noir bleuté, indigo discret en haut à gauche, violet en bas
   à droite, feuille géante en filigrane ; jour = lavande pâle, feuille tracée
   en violet. NE PLUS LE CHANGER sans qu'il le demande.
3. **Barres de titre des fenêtres** — [écrit], commit local `cff5b47` (nuit
   du 17 au 18) : démon Rust `linux/coins` (`grenos-coins`, x11rb, sans
   WebKit, 4 essais `cargo test`) lancé par `grenos-session` sous
   `grenos-veilleur` ; masque SHAPE de 10 px sur chaque cadre d'openbox,
   reposé quand openbox le remet à zéro (ShapeNotify « sans forme ») ; ignore
   agrandies / plein écran / DOCK / DESKTOP / fenêtres déjà formées. Compilé
   par la CI dans la même étape que grenos-ui (`.github/workflows/linux.yml`),
   binaire ignoré par git. `themerc` : bordure 0, padding 10/9, barre
   `#131b2e`, ronds 12 px façon feux (fermer `#ef4d5e`, réduire `#fbbf24`,
   agrandir `#2bc48a`), glyphe creusé au survol (`boutons-fenetre.py`,
   `--apercu` pour les relire en texte). RESTE : le voir sur capture QEMU
   (la CI de la branche le montrera) ; si openbox ignorait les couleurs par
   bouton (`window.active.button.close.unpressed.image.color`), les ronds
   seraient tous gris clair.
4. **Animations** — [écrit], commit local `0cf0e1e` : les fenêtres de
   grenos-ui naissent du fond (180 ms) et s'y effacent (140 ms) ; Rust retient
   la fermeture d'openbox, ajoute `.fermeture`, détruit après le fondu ; rien
   en « performance » ; un essai vérifie que Rust et `style.css` disent la
   même durée. Openbox anime déjà la réduction (`animateIconify`). RESTE : les
   menus et fenêtres GTK (Python) — sans compositeur, seule piste : animer le
   contenu, pas la fenêtre ; à faire quand barre/bureau passeront en gtk-rs.
5. Ne pas émettre de son pendant les essais (demande explicite).

## 2. Ensuite — plus aucun processus Python au quotidien

- **Barre (`grenos-shell`) et bureau (`grenos-bureau`) en Rust NATIF (gtk-rs)**,
  pas en Tauri : ils tournent en permanence, WebKit coûterait plus que Python.
- Les autres fenêtres (Fichiers, Réglages, GrenPlace, Tâches, Son, Connexions,
  grenHub, Jeux, Bienvenue, Secours, Arrêt) deviennent des pages de grenos-ui.
- Les assistants Python de secours (premier démarrage, maj) restent livrés.

## 3. Reste de `todo.md`

Section I (OOBE) : I8 mode jeu Ctrl+Alt+Échap, I13 sauvegardes, I17 écrans
Dev. Section K3 constructions reproductibles. Voir `todo.md`.

## Outils à disposition

- MCP `powershell-local` : captures d'écran et contrôle du PC (navigateur,
  VirtualBox pour essayer l'ISO). **Aucun son.**
- Aperçu des pages grenos-ui : `.claude/launch.json` → `grenos-ui-apercu`
  (port 8765, données factices dans `linux/ui/web/apercu.js`).
- `linux/tools/catalogue-jeux.py` régénère le catalogue de GrenLaunch.
