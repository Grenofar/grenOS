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
- **Quand la CI est verte sur la branche** : fusionner dans `main`
  (`git checkout main && git merge --no-ff claude/grenos-ui && git push`).
  La CI de `main` publie la Release (ISO + zip VirtualBox) et le dépôt APT.
- **Puis le site** : dépôt `grenos-web-download`, branche locale
  `claude/theme-grenos-ui` (thème grenos-ui + section « Dans cette version »),
  à fusionner dans `main` et pousser — Vercel déploie. Seulement APRÈS la
  Release, pour ne pas annoncer une version introuvable.
- Supabase (projet `tpqzhzuoyqpfairatdrw`, org « kaka », plan gratuit) se met
  en pause après ~7 jours sans activité : « Restore » dans le tableau de bord,
  données conservées. Plafond de dépense activé : jamais facturé.

## 1. Demandé le 17 novembre au soir — le visuel

1. **Logo grenOS sans lettre** — [écrit], commit local `visuel` : une pousse
   (deux feuilles + un point de lumière) dans `icones.py` (`marque`, `feuille`),
   les icônes de grenos-ui, le SVG `marque` de `web/commun.js` (logos de
   l'OOBE) et `marque-pastille` du site. Reste : vérifier sur capture QEMU.
2. **Fond d'écran** — [écrit], même commit : l'aurore de grenos-ui (bleu en
   haut à gauche, violet en bas à droite) + une feuille géante en filigrane.
3. **Barres de titre des fenêtres** (déplacer, fermer) : arrondies et propres. [à faire]
   PLAN PRÉCIS : (a) un petit démon Rust `grenos-coins` (crate x11rb, sans
   WebKit) lancé par `grenos-session`, qui écoute la création/redimension des
   fenêtres CADRE d'openbox (enfants de la racine, `_NET_CLIENT_LIST`) et leur
   applique un masque XShape aux coins arrondis de 10 px — même algorithme que
   `grenosui.region_arrondie` ; ignorer les fenêtres maximisées/plein écran
   (`_NET_WM_STATE`) ; (b) `themerc` : barre plus haute (padding.height 9),
   bordure 0, couleur `#131b2e` (= --surface-pleine), boutons en ronds pleins
   (XBM disque 10x10 via `boutons-fenetre.py`) façon feux : fermer rouge
   `#ef4d5e`, agrandir vert `#2bc48a`, réduire ambre `#fbbf24` (openbox 3.6
   accepte `window.active.button.close.unpressed.image.color`, etc.) ;
   (c) vérifier sur capture QEMU AVANT de pousser.
   Thème openbox `linux/config/includes.chroot/usr/share/themes/grenOS/openbox-3/themerc`
   + boutons `linux/tools/boutons-fenetre.py`. Sans compositeur : l'arrondi se
   fait par masque de forme (voir `grenosui.region_arrondie`) ou openbox
   `window.*.border` ; à vérifier sur capture QEMU.
4. **Animations** : ouverture/fermeture de fenêtres et menus, sans coût CPU
   (pas de compositeur : fondus courts côté GTK/WebKit, respect du réglage
   « effets »).
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
