//! Fermer une application pour de bon (24 novembre).
//!
//! Grenofar : « fais que dans la flèche ça ferme complètement le process ».
//! Fermer ses fenêtres ne suffit pas : Steam, Discord, Heroic… se rangent
//! dans un coin et continuent de tourner. « Fermer » dans la flèche quitte
//! donc l'application entière : ses fenêtres d'abord (elle peut enregistrer),
//! puis SIGTERM à tout son arbre de processus, puis SIGKILL à ce qui reste.
//!
//! L'arbre : on part du processus de chaque fenêtre (`_NET_WM_PID`), on
//! remonte jusqu'au plus haut parent qui fait encore partie de l'application
//! — pas init, pas systemd, pas un shell ni un terminal, rien de la session
//! grenOS (les ancêtres de la barre) — puis on prend tous ses descendants.
//! Steam : la fenêtre est à steamwebhelper, on remonte à steam, et tout
//! part ; Firefox : le processus principal et ses onglets.

use std::collections::HashSet;
use std::process::Command;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct Processus {
    pub pid: u32,
    pub parent: u32,
    pub nom: String,
    /// Date de naissance (champ 22 de /proc/pid/stat) : un pid réutilisé
    /// entre deux signaux n'est pas le même processus.
    pub debut: u64,
}

/// Lit /proc/<pid>/stat. Le nom est entre parenthèses et peut en contenir.
fn lire(pid: u32) -> Option<Processus> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    analyser_stat(&stat)
}

pub fn analyser_stat(stat: &str) -> Option<Processus> {
    let ouvre = stat.find('(')?;
    let ferme = stat.rfind(')')?;
    let pid = stat[..ouvre].trim().parse().ok()?;
    let nom = stat[ouvre + 1..ferme].to_string();
    let reste: Vec<&str> = stat[ferme + 1..].split_whitespace().collect();
    // reste[0] = état, reste[1] = parent, … reste[19] = début.
    let parent = reste.get(1)?.parse().ok()?;
    let debut = reste.get(19)?.parse().ok()?;
    Some(Processus { pid, parent, nom, debut })
}

/// Le processus `pid` est-il bien celui de l'application ? `_NET_WM_PID` est
/// écrit par l'application elle-même : une Flatpak y met son pid du bac à
/// sable, qui sur la machine est un tout autre programme. On ne signale donc
/// que si son nom ou son chemin ressemble à la classe de la fenêtre ou au
/// programme lancé (soffice.bin vit dans /usr/lib/libreoffice).
pub fn est_de(pid: u32, indices: &[&str]) -> bool {
    let Some(p) = lire(pid) else { return false };
    let chemin = std::fs::read(format!("/proc/{pid}/cmdline"))
        .ok()
        .and_then(|c| c.split(|&b| b == 0).next().map(|a| String::from_utf8_lossy(a).into_owned()))
        .unwrap_or_default();
    ressemble(&p.nom, &chemin, indices)
}

pub fn ressemble(nom: &str, chemin: &str, indices: &[&str]) -> bool {
    let botte = format!("{} {}", nom, chemin).to_lowercase();
    let nom = nom.to_lowercase();
    indices.iter().any(|i| {
        let i = std::path::Path::new(i).file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        // « google-chrome » → « google », « chrome » ; « Firefox-esr » → « firefox ».
        i.split(|c: char| !c.is_alphanumeric())
            .filter(|m| m.len() >= 4)
            .any(|m| botte.contains(m) || (nom.len() >= 4 && m.contains(nom.as_str())))
    })
}

/// Tous les processus de la machine.
pub fn tous() -> Vec<Processus> {
    let Ok(dossier) = std::fs::read_dir("/proc") else { return Vec::new() };
    dossier
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse::<u32>().ok())
        .filter_map(lire)
        .collect()
}

/// Ce qu'on ne remonte jamais : au-dessus, ce n'est plus l'application.
fn barriere(nom: &str) -> bool {
    const NOMS: &[&str] = &[
        "systemd", "init", "openbox", "Xorg", "Xwayland", "lightdm", "gdm", "sddm", "xinit",
        "dbus-daemon", "dbus-broker", "sh", "bash", "dash", "zsh", "fish", "setsid", "sudo",
        "x-terminal-emul", "xterm", "gnome-terminal-", "xfce4-terminal", "konsole", "alacritty",
        "kitty", "foot", "tilix", "flatpak-session", "flatpak-portal", "flatpak", "bwrap",
        // Les programmes Python de grenOS s'appellent « python3 » dans /proc.
        "python3", "python", "xdg-desktop-por",
    ];
    nom.starts_with("grenos") || NOMS.contains(&nom)
}

/// Le plus haut processus qui appartient encore à l'application de `pid`.
/// `session` : la barre et ses ancêtres, qu'on ne touche jamais.
fn sommet(pid: u32, liste: &[Processus], session: &HashSet<u32>) -> u32 {
    let mut courant = pid;
    for _ in 0..64 {
        let Some(p) = liste.iter().find(|p| p.pid == courant) else { break };
        let Some(parent) = liste.iter().find(|q| q.pid == p.parent) else { break };
        if parent.pid <= 1 || session.contains(&parent.pid) || barriere(&parent.nom) {
            break;
        }
        courant = parent.pid;
    }
    courant
}

/// L'arbre de l'application : les sommets et tous leurs descendants, sans
/// rien de la session ni de grenOS.
pub fn arbre(pids: &[u32], liste: &[Processus], session: &HashSet<u32>) -> Vec<Processus> {
    let mut garde: HashSet<u32> = pids
        .iter()
        .filter(|&&p| p > 1 && !session.contains(&p))
        // Une fenêtre de grenOS lui-même : on ne prend ni elle ni ses enfants.
        .filter(|&&p| liste.iter().find(|q| q.pid == p).is_some_and(|q| !q.nom.starts_with("grenos")))
        .map(|&p| sommet(p, liste, session))
        .collect();
    loop {
        let avant = garde.len();
        for p in liste {
            if garde.contains(&p.parent) {
                garde.insert(p.pid);
            }
        }
        if garde.len() == avant {
            break;
        }
    }
    liste
        .iter()
        .filter(|p| garde.contains(&p.pid) && !session.contains(&p.pid) && !p.nom.starts_with("grenos"))
        .cloned()
        .collect()
}

/// La barre et tous ses ancêtres.
pub fn session(liste: &[Processus]) -> HashSet<u32> {
    let mut s = HashSet::new();
    let mut courant = std::process::id();
    while courant > 1 && s.insert(courant) {
        match liste.iter().find(|p| p.pid == courant) {
            Some(p) => courant = p.parent,
            None => break,
        }
    }
    s.insert(1);
    s
}

fn signaler(signal: &str, cibles: &[Processus]) {
    // Seulement ceux qui vivent encore et sont bien les mêmes.
    let vivants: Vec<String> = cibles
        .iter()
        .filter(|c| lire(c.pid).is_some_and(|p| p.debut == c.debut))
        .map(|c| c.pid.to_string())
        .collect();
    if vivants.is_empty() {
        return;
    }
    let _ = Command::new("kill").arg(format!("-{signal}")).args(&vivants).status();
}

/// Quitte l'application pour de bon, sans bloquer la barre : SIGTERM (elle
/// range ses affaires), puis SIGKILL trois secondes plus tard si elle
/// s'accroche. `flatpak` non vide : `flatpak kill` fait tomber tout le bac
/// à sable. `delai` laisse d'abord aux fenêtres le temps de se fermer.
pub fn quitter(pids: Vec<u32>, flatpak: String, delai: Duration, force: bool) {
    let liste = tous();
    let session = session(&liste);
    let cibles = arbre(&pids, &liste, &session);
    std::thread::spawn(move || {
        std::thread::sleep(delai);
        if !force {
            signaler("TERM", &cibles);
            std::thread::sleep(Duration::from_secs(3));
        }
        signaler("KILL", &cibles);
        if !flatpak.is_empty() {
            let _ = Command::new("flatpak").args(["kill", flatpak.as_str()]).status();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, parent: u32, nom: &str) -> Processus {
        Processus { pid, parent, nom: nom.into(), debut: 0 }
    }

    #[test]
    fn stat_se_lit_meme_avec_des_parentheses() {
        let s = "1234 (Web Content (x)) S 1200 1234 1234 0 -1 4194560 0 0 0 0 0 0 0 0 20 0 1 0 98765 0 0";
        let pr = analyser_stat(s).unwrap();
        assert_eq!((pr.pid, pr.parent, pr.nom.as_str(), pr.debut), (1234, 1200, "Web Content (x)", 98765));
    }

    #[test]
    fn le_pid_de_la_fenetre_est_verifie() {
        assert!(ressemble("chrome", "/opt/google/chrome/chrome", &["Google-chrome"]));
        assert!(ressemble("steamwebhelper", "./steamwebhelper", &["Steam", "steam"]));
        assert!(ressemble("soffice.bin", "/usr/lib/libreoffice/program/soffice.bin", &["libreoffice-writer"]));
        assert!(ressemble("firefox-esr", "/usr/lib/firefox-esr/firefox-esr", &["Firefox-esr", "/usr/bin/firefox-esr"]));
        // Un pid du bac à sable qui tombe sur autre chose : on ne touche pas.
        assert!(!ressemble("pipewire", "/usr/bin/pipewire", &["discord", "com.discordapp.Discord"]));
    }

    #[test]
    fn steam_part_en_entier_et_rien_d_autre() {
        let liste = vec![
            p(1, 0, "systemd"),
            p(50, 1, "grenos-session"),
            p(60, 50, "grenos-shell-na"),
            p(100, 1, "steam"),
            p(101, 100, "steamwebhelper"),
            p(102, 101, "steamwebhelper"),
            p(200, 1, "firefox-esr"),
            p(300, 1, "bash"),
            p(301, 300, "vim"),
        ];
        let session: HashSet<u32> = [1, 50, 60].into();
        let mut a: Vec<u32> = arbre(&[102], &liste, &session).iter().map(|x| x.pid).collect();
        a.sort();
        assert_eq!(a, vec![100, 101, 102]);
        // Lancé d'un terminal : on s'arrête au shell, le shell reste.
        let a: Vec<u32> = arbre(&[301], &liste, &session).iter().map(|x| x.pid).collect();
        assert_eq!(a, vec![301]);
        // Jamais la session.
        assert!(arbre(&[60, 50, 1], &liste, &session).is_empty());
        // Une application de grenOS (et ses processus WebKit) : rien.
        let mut l2 = liste.clone();
        l2.push(p(400, 1, "grenos-parametr"));
        l2.push(p(401, 400, "WebKitWebProces"));
        assert!(arbre(&[400], &l2, &session).is_empty());
    }
}
