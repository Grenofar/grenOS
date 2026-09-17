# Spec — loading screen, wallpapers, and a Windows-like explorer

Written by Claude for the agents, 2026-09-17, at the human's request: "the
explorer, the wallpaper as one likes, and a loading screen at the start".
Every function named below exists on main today (read in `kernel/src/fb.rs`,
`desktop.rs`, `fs.rs`, `main.rs`). English, because models read it.

## 0. Rules that bind every task here

1. **The font only has Latin-1** (U+0000 to U+00FF). No `…`, `—`, `•`, `›`,
   `✓`, emoji or arrows in any displayed text: they draw as `?`. Write `...`,
   `-`, `>`. Accents like `é à è ç` are fine.
2. `desktop.rs` is also compiled on the host by Claude's preview harness: it must
   not call hardware or kernel services (`rand`, `sha256`, `ahci`, `events`)
   directly. It asks, and `main.rs` answers — the pattern of `wants_update` /
   `update_result`, `wants_install` / `install_progress`.
3. CI judges the screen at 25 s: at least 3 colours, none on more than 90% of
   it. The **default** wallpaper must keep passing that; a plain colour must
   never be the default.
4. Each task ends with the six CI steps green, and CI fails a boot whose serial
   log contains `FAILED its`.

## 1. Task L — the loading screen (`kernel/src/splash.rs`, `kernel/src/main.rs`)

Today the screen stays black from Limine until `desktop: drawn`: the kernel is
checking itself (memory, paging, SHA-512/Ed25519, PBKDF2 with 80 000
iterations, PCI, the network card, ACPI, the disks) and nothing says so. The
desktop's own fade-in splash (`Desktop::paint_splash`, `SPLASH_MS`) only starts
once the desktop exists.

- New module `splash.rs`:
  ```rust
  /// Draws the loading screen: the grenOS badge, the name, what the kernel is
  /// doing, and a bar `done` out of `total` long, then presents the screen.
  pub fn draw(screen: &mut fb::Screen, step: &str, done: usize, total: usize);
  ```
  Look: the same as `Desktop::paint_splash` so the hand-over is seamless — a
  `Rgb(0x05, 0x07, 0x0D)` background (`screen.fill`), a 64-pixel rounded badge
  in `Rgb(0x2F, 0x7D, 0xF6)`, the desktop's `ACCENT` (`screen.round`) with `icons::draw(..., Icon::Home,
  ...)` inside, "grenOS" centred below in `Font::Title`, the step in
  `Font::Small` under it, and a 180 x 4 bar (`screen.round`, filled part in the
  accent colour). End with `screen.present(screen.full())`.
- `main.rs`: the `Screen` is created today after the network start
  (`fb::Screen::new(frame.addr(), (start + heap_size + hhdm) as *mut u32,
  mode)`). Create it right after `heap::init` instead (the back buffer lives
  just after the heap, which is already reserved there), call `font::tune`
  first, and call `splash::draw` before each stage: "Mémoire", "Sécurité du
  processeur", "Vérification des signatures", "Périphériques", "Réseau",
  "Disques", "Bureau". Keep every existing serial line exactly as it is.
- Proof: one serial line `splash: shown` after the first draw.

## 2. Task W — wallpapers the person chooses (`kernel/src/desktop.rs` only)

Today `Desktop::paint_wallpaper` draws one gradient (`WALL_TOP` to
`WALL_BOTTOM`) and the grenOS mark.

- Add at least six wallpapers, drawn procedurally with `screen.gradient`,
  `screen.fill`, `screen.wash` and `screen.round` only (no image files):
  "Nuit" (today's, and the default), "Aurore" (dark blue to teal with two soft
  diagonal bands of green and violet washed over), "Océan" (deep blue gradient
  with lighter horizontal waves), "Crépuscule" (violet to orange), "Grille"
  (dark background with a faint grid every 48 pixels), "Graphite" (dark grey
  gradient with the mark in accent blue).
- A field `wallpaper: usize` on `Desktop`, 0 by default.
- Paramètres → Écran (section 2, today `paint_rows(... screen_lines())`): keep
  the lines, and below them a row of clickable thumbnails, 120 x 72 each with
  the wallpaper's name under it; the chosen one gets a 2-pixel accent border.
  Draw a thumbnail by painting the wallpaper into the thumbnail's rectangle
  (give the painting function an `area: Rect` parameter rather than assuming
  the whole screen).
- Clicking a thumbnail sets `wallpaper` and damages the whole screen
  (`self.damage_all()`), so it changes at once.
- Add `pub fn wallpaper_rect(&self, index: usize) -> Rect` (the thumbnail) so
  the preview harness can click it.
- Proof: CI's screen step stays green with the default; Claude renders every
  wallpaper on the host.

## 3. Task E — the explorer, closer to Windows' (`kernel/src/desktop.rs` only)

The explorer already has Windows' layout (toolbar, breadcrumb, quick access,
columns Name/Type/Size, status bar; `files_*` functions). What it lacks is what
people do in it. `fs.rs` already has what is needed: `make_dir(path)`,
`write(path, text) -> bool`, `read(path) -> Option<&str>`, `remove(path) ->
bool`, `exists(path)`, `list(dir)`, `free_name(dir, stem, extension)`,
`parent_of`, `name_of`, `join`. The mouse has one button (`mouse::Packet {
dx, dy, left }`), so there is no right-click menu: commands go in a command bar,
as in Windows 11.

Three small tasks, in this order, each one commit:

- **E1 — selection and the command bar.** One click selects a row (accent
  background, as Windows does), a second click on the same row within 500 ms
  opens it (a folder navigates, a text file opens in Bloc-notes, which already
  happens on a click today). A command bar under the title: "Nouveau dossier",
  "Renommer", "Supprimer", "Copier", "Coller", drawn with `screen.round` and
  greyed out (`TEXT_FAINT`) when they do not apply. The status bar says
  "N éléments" et, when one is selected, "1 élément sélectionné".
- **E2 — the commands.** "Nouveau dossier" makes `free_name(dir, "Nouveau
  dossier", "")` and selects it; "Supprimer" removes the selection (system files
  under `/Système` are refused, with a one-line message in the status bar);
  "Copier" remembers the path, "Coller" writes a copy under `free_name`.
- **E3 — rename and search.** "Renommer" turns the selected name into a text
  field (keyboard input goes there, Enter confirms, Escape cancels; the file is
  copied to the new name and the old one removed); a search box at the right of
  the address bar filters the listed names as one types
  (case-insensitive `contains`).

Proof for each: CI green; Claude drives the explorer in the host preview harness
and looks at the frames.
