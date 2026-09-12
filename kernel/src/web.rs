//! What the browser can reach today: pages carried inside the kernel, and the
//! files of the machine.
//!
//! There is no network driver yet, so nothing outside this machine is
//! reachable, and the browser says so instead of hanging on a blank page. The
//! address bar already speaks two schemes — `grenos:` for the pages below,
//! `fichier:` for the file system — and a third, `http:`, will make sense the
//! day a card answers.

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
               ## Où aller\n\
               [Aide et raccourcis](grenos:aide)\n\
               [Pourquoi le web n'est pas encore là](grenos:reseau)\n\
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
               ## Dans VirtualBox\n\
               - Cliquez dans la fenêtre pour que la machine reçoive la souris. La touche Ctrl de droite vous la rend.\n\
               - Si le pointeur ne bouge pas, ouvrez Paramètres, Souris et clavier : les compteurs disent si les octets arrivent.\n\
               \n\
               [Retour à l'accueil](grenos:accueil)",
    },
    Page {
        url: "grenos:reseau",
        title: "Le réseau",
        body: "# Pourquoi le web ne s'ouvre pas\n\
               Ce navigateur affiche les pages que le noyau porte en lui, et les fichiers de la machine. Le web, lui, demande quatre choses qui n'existent pas encore ici :\n\
               \n\
               - un pilote de carte réseau (e1000 dans QEMU, PCnet ou e1000 dans VirtualBox) ;\n\
               - une pile IPv4 : ARP, DHCP pour obtenir une adresse, UDP, puis TCP ;\n\
               - un résolveur DNS, pour traduire un nom en adresse ;\n\
               - TLS, sans quoi presque aucun site n'accepte de répondre.\n\
               \n\
               Chacun est une étape de la feuille de route, dans cet ordre. La carte est déjà visible : Paramètres, Matériel, la liste PCI la montre.\n\
               \n\
               [Retour à l'accueil](grenos:accueil)",
    },
    Page {
        url: "grenos:versions",
        title: "Versions",
        body: "# Ce qui a changé\n\
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
                        return Block::Link(text, url);
                    }
                }
            }
            Block::Text(line)
        })
        .collect()
}
