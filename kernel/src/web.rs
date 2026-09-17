//! The pages carried inside the kernel. The address bar speaks four schemes:
//! `grenos:` for the pages below, `fichier:` for the file system, and `http:`
//! and `https:`, which go out through the network card (`http.rs`).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub struct Page {
    pub url: &'static str,
    pub title: &'static str,
    pub body: &'static str,
}

/// A line of a page, once read.
pub enum Block<'a> {
    Title(&'a str),
    Head(&'a str),
    Text(&'a str),
    Item(&'a str),
    /// The words shown, and where they lead.
    Link(&'a str, &'a str),
    Rule,
    Space,
}

pub const HOME: &str = "grenos:accueil";

pub const PAGES: [Page; 5] = [
    Page {
        url: "grenos:accueil",
        title: "Accueil",
        body: "# grenOS\n\
               Un système d'exploitation x86_64 écrit en Rust, sans rien dessous : pas de Linux, pas de Windows, pas de bibliothèque C. Il démarre avec Limine, parle au matériel lui-même, et dessine ce bureau dans le framebuffer.\n\
               \n\
               ## Chercher sur le web\n\
               Tapez une adresse ou des mots dans la barre du haut, puis Entrée : une adresse s'ouvre, des mots sont cherchés.\n\
               [Google](https://www.google.com/webhp?hl=fr)\n\
               [Wikipédia en français](https://fr.wikipedia.org/)\n\
               \n\
               ## Où aller\n\
               [Aide et raccourcis](grenos:aide)\n\
               [Le réseau et le web](grenos:reseau)\n\
               [Ce qui a changé](grenos:versions)\n\
               [Les fichiers de la machine](fichier:/)\n\
               \n\
               ---\n\
               Écrit par une équipe d'agents autonomes, et rien n'arrive ici sans une CI verte.",
    },
    Page {
        url: "grenos:aide",
        title: "Aide",
        body: "# Aide\n\
               ## À la souris\n\
               - Le bouton en haut à gauche ouvre le menu des applications ; tapez pour filtrer.\n\
               - Une fenêtre se déplace par sa barre de titre, se réduit, s'agrandit et se ferme par les trois boutons à droite.\n\
               - Une fenêtre réduite reste dans le panneau : un clic la ramène.\n\
               ## Au clavier\n\
               - F1 ou la touche Windows ouvre le menu.\n\
               - Tab passe d'une fenêtre à l'autre, Échap ferme un menu.\n\
               - Le clavier est en français (AZERTY).\n\
               ## Dans le navigateur\n\
               - La molette, les flèches, Page précédente et Page suivante font défiler la page ; Début et Fin vont aux deux bouts.\n\
               - La barre à droite de la page se tire à la souris.\n\
               - Au survol d'un lien, son adresse s'affiche en bas.\n\
               ## Dans VirtualBox\n\
               - Cliquez dans la fenêtre pour que la machine reçoive la souris. La touche Ctrl de droite vous la rend.\n\
               - Si le pointeur ne bouge pas, ouvrez Paramètres, Souris et clavier : les compteurs disent si les octets arrivent.\n\
               \n\
               [Retour à l'accueil](grenos:accueil)",
    },
    Page {
        url: "grenos:reseau",
        title: "Le réseau",
        body: "# Le réseau\n\
               grenOS parle au réseau : pilote de carte Intel 8254x, ARP, IPv4, ICMP, UDP, DHCP, DNS, un client TCP, et TLS 1.3 écrit dans le noyau.\n\
               \n\
               ## Ce qui marche\n\
               - La machine obtient son adresse toute seule, par DHCP. Paramètres, Réseau, la montre.\n\
               - La passerelle répond au ping, et grenOS répond aux pings qu'on lui envoie.\n\
               - Les noms sont résolus par le serveur que le réseau a indiqué.\n\
               - Ce navigateur ouvre les pages http et https, suit les redirections et lit le français quel que soit l'encodage.\n\
               - La barre d'adresse cherche sur le web : Google refuse les navigateurs sans JavaScript, la recherche passe donc par DuckDuckGo.\n\
               - grenOS vérifie tout seul, au démarrage, s'il existe une version plus récente.\n\
               \n\
               ## Ce qui manque\n\
               - L'identité du serveur : la connexion est chiffrée, mais le certificat n'est pas vérifié. Rien ne s'installe donc tout seul.\n\
               - Les images, les styles et le JavaScript : cette fenêtre montre le texte et les liens, sans les menus des sites.\n\
               \n\
               [Notre site, en https](https://grenos-dev.vercel.app/download)\n\
               [Essayer example.com](http://example.com)\n\
               [Retour à l'accueil](grenos:accueil)",
    },
    Page {
        url: "grenos:versions",
        title: "Versions",
        body: "# Ce qui a changé\n\
               ## 0.9.0\n\
               - Les mises à jour s'installent depuis Paramètres : manifeste signé, noyau vérifié, écrit dans l'autre emplacement\n\
               - Pilote de disque SATA (AHCI) et système de fichiers FAT32\n\
               - Signatures Ed25519 et SHA-512, vérifiées à chaque démarrage\n\
               ## 0.8.0\n\
               - Mise à jour automatique : vérifiée au démarrage, et d'un clic dans Paramètres\n\
               - TLS 1.3 dans le noyau, et les pages https dans le navigateur\n\
               ## 0.7.0\n\
               - L'explorateur de fichiers prend la forme de celui de Windows\n\
               - Le navigateur prend la forme de Chrome : onglet, omnibox, favoris\n\
               - Icônes redessinées, nettes à toutes les tailles\n\
               ## 0.6.0\n\
               - Le réseau : carte 8254x, ARP, IPv4, ICMP, UDP, DHCP, DNS, TCP\n\
               - Le navigateur ouvre les vraies pages en http\n\
               - Paramètres, Réseau : adresse, passerelle, trames, ping\n\
               ## 0.5.0\n\
               - Explorateur de fichiers, navigateur, écran de connexion, animations\n\
               - Réduire ne ferme plus la fenêtre, et de vraies icônes remplacent les lettres\n\
               - Le terminal ne s'ouvre plus tout seul, et la bienvenue ne s'affiche qu'une fois\n\
               - Un système de fichiers en mémoire, où le bloc-notes enregistre\n\
               ## 0.4.0\n\
               - Bureau sombre, panneau, menu avec recherche, fenêtres à ombre portée\n\
               - Souris PS/2 réparée, et la CI injecte une vraie souris pour le prouver\n\
               - Mémoire, pagination, tas, PCI, extinction ACPI\n\
               ## 0.3.0\n\
               - Premier bureau graphique, horloge, bloc-notes\n\
               \n\
               [Retour à l'accueil](grenos:accueil)",
    },
    Page {
        url: "grenos:projet",
        title: "Le projet",
        body: "# La fabrique\n\
               grenOS est écrit par une équipe d'agents : un Maître qui décide, un Architecte qui conçoit, un Codeur, un Testeur, et des spécialistes. Ils travaillent sur des branches, et seule une CI verte fait avancer une tâche.\n\
               \n\
               Le tableau de bord est sur grenos-dev.vercel.app, et les images s'y téléchargent sans compte.\n\
               \n\
               [Retour à l'accueil](grenos:accueil)",
    },
];

pub fn find(url: &str) -> Option<&'static Page> {
    PAGES.iter().find(|page| page.url == url)
}

/// Reads a page into the blocks the browser draws.
pub fn parse(body: &str) -> Vec<Block<'_>> {
    body.lines()
        .map(|line| {
            let line = line.trim_start();
            if line.is_empty() {
                return Block::Space;
            }
            if line == "---" {
                return Block::Rule;
            }
            if let Some(rest) = line.strip_prefix("## ") {
                return Block::Head(rest);
            }
            if let Some(rest) = line.strip_prefix("# ") {
                return Block::Title(rest);
            }
            if let Some(rest) = line.strip_prefix("- ") {
                return Block::Item(rest);
            }
            if let Some(rest) = line.strip_prefix('[') {
                if let Some((text, target)) = rest.split_once("](") {
                    if let Some(url) = target.strip_suffix(')') {
                        if !url.contains([')', ' ']) {
                            return Block::Link(text, url);
                        }
                    }
                }
            }
            Block::Text(line)
        })
        .collect()
}

/// How a row of a laid-out page is drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Title,
    Head,
    Body,
    Item,
    Rule,
    Space,
}

/// Words on a row, starting at a character column; a link when they lead
/// somewhere.
pub struct Span {
    pub column: usize,
    pub text: String,
    pub link: Option<String>,
}

/// One row of a page on screen. `bullet` marks the first row of an item.
pub struct Row {
    pub style: Style,
    pub bullet: bool,
    pub spans: Vec<Span>,
}

/// How many characters fit on a row in each font.
pub struct Columns {
    pub title: usize,
    pub head: usize,
    pub body: usize,
}

/// A page laid out in rows: every paragraph wrapped at spaces, and links —
/// on a line of their own or inside a sentence, written `[words](address)` —
/// kept as spans that a click can find.
pub fn layout(body: &str, columns: &Columns) -> Vec<Row> {
    let mut rows = Vec::new();
    for block in parse(body) {
        match block {
            Block::Title(text) => wrap_into(&mut rows, Style::Title, &segments(text), columns.title),
            Block::Head(text) => wrap_into(&mut rows, Style::Head, &segments(text), columns.head),
            Block::Text(text) => wrap_into(&mut rows, Style::Body, &segments(text), columns.body),
            Block::Item(text) => {
                let first = rows.len();
                wrap_into(&mut rows, Style::Item, &segments(text), columns.body.saturating_sub(2));
                if let Some(row) = rows.get_mut(first) {
                    row.bullet = true;
                }
            }
            Block::Link(text, url) => wrap_into(&mut rows, Style::Body, &[(text, Some(url))], columns.body),
            Block::Rule => rows.push(Row { style: Style::Rule, bullet: false, spans: Vec::new() }),
            Block::Space => rows.push(Row { style: Style::Space, bullet: false, spans: Vec::new() }),
        }
    }
    rows
}

/// The link under a character column of a row, if any.
pub fn link_at(row: &Row, column: usize) -> Option<&str> {
    row.spans
        .iter()
        .find(|span| column >= span.column && column < span.column + span.text.chars().count())
        .and_then(|span| span.link.as_deref())
}

/// A line split into plain runs and `[words](address)` links.
fn segments(text: &str) -> Vec<(&str, Option<&str>)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let Some(middle) = after.find("](") else {
            break;
        };
        let words = &after[..middle];
        let target = &after[middle + 2..];
        let Some(end) = target.find(')') else {
            break;
        };
        let url = &target[..end];
        let address = ["http://", "https://", "grenos:", "fichier:"].iter().any(|scheme| url.starts_with(scheme));
        if words.contains('[') || url.contains(' ') || !address {
            // Not a link: the bracket is text, and the search goes on after it.
            out.push((&rest[..=open], None));
            rest = after;
            continue;
        }
        if open > 0 {
            out.push((&rest[..open], None));
        }
        out.push((words, Some(url)));
        rest = &target[end + 1..];
    }
    if !rest.is_empty() {
        out.push((rest, None));
    }
    out
}

/// Lays runs of words out in rows of at most `width` characters, cutting at
/// spaces, and a word longer than a row wherever it must.
fn wrap_into(rows: &mut Vec<Row>, style: Style, segments: &[(&str, Option<&str>)], width: usize) {
    let width = width.max(8);
    let mut row = Row { style, bullet: false, spans: Vec::new() };
    let mut used = 0;
    let mut space = false;
    for &(text, link) in segments {
        for (index, piece) in text.split(' ').enumerate() {
            space |= index > 0;
            let mut word = piece;
            while !word.is_empty() {
                let length = word.chars().count();
                let gap = usize::from(space && used > 0);
                if used > 0 && used + gap + length > width {
                    rows.push(core::mem::replace(&mut row, Row { style, bullet: false, spans: Vec::new() }));
                    used = 0;
                    continue;
                }
                let cut = word.char_indices().nth(width).map_or(word.len(), |(at, _)| at);
                let (head, tail) = word.split_at(cut);
                match row.spans.last_mut() {
                    Some(last) if last.link.as_deref() == link => {
                        if gap > 0 {
                            last.text.push(' ');
                        }
                        last.text.push_str(head);
                    }
                    _ => row.spans.push(Span {
                        column: used + gap,
                        text: head.to_string(),
                        link: link.map(ToString::to_string),
                    }),
                }
                used += gap + head.chars().count();
                space = false;
                word = tail;
            }
        }
    }
    rows.push(row);
}

/// Turns what the human typed into a URL to open.
///
/// - `http://`, `https://`, `grenos:` and `fichier:` are opened as they are.
/// - Text with no space that contains a dot is opened as `https://` + text.
/// - Anything else is a search on DuckDuckGo's HTML page, which answers text
///   browsers, with the query percent-encoded (space as `+`, every byte
///   outside `A-Za-z0-9-._~` as `%XX` of its UTF-8 bytes).
pub fn parse_address(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("grenos:")
        || trimmed.starts_with("fichier:")
    {
        return Some(trimmed.to_string());
    }
    if !trimmed.contains(' ') && trimmed.contains('.') {
        return Some(format!("https://{trimmed}"));
    }
    let mut encoded = String::new();
    for byte in trimmed.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else if byte == b' ' {
            encoded.push('+');
        } else {
            encoded.push('%');
            encoded.push(hex(byte >> 4));
            encoded.push(hex(byte & 0x0F));
        }
    }
    Some(format!("https://html.duckduckgo.com/html/?q={encoded}"))
}

fn hex(nibble: u8) -> char {
    if nibble < 10 {
        (b'0' + nibble) as char
    } else {
        (b'A' + (nibble - 10)) as char
    }
}
