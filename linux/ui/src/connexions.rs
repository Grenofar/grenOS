//! Connexions : le Wi-Fi et le Bluetooth, dans une seule fenêtre.
//!
//! Repris de `grenos-connexions` et `reseau.py` le 18 novembre. Tout passe
//! par `nmcli` et `bluetoothctl`, avec un délai pour chacun (`timeout`), et
//! dans des commandes `async` : la fenêtre s'affiche d'abord, l'état arrive
//! ensuite. La leçon du 26 septembre tient toujours — cette fenêtre mettait
//! DEUX MINUTES à s'ouvrir parce que `bluetoothctl` attendait son délai
//! entier sur une machine sans Bluetooth, PENDANT la construction.
//!
//! Les erreurs sont rendues telles quelles, traduites quand on les reconnaît :
//! « mot de passe refusé » est une information utile.
//!
//! LE 27 NOVEMBRE, chez Grenofar (câble ET Wi-Fi « Famille Belgacem ») :
//! - « savoir dans quel réseau je suis » : l'en-tête disait « Connecté en
//!   câble à Wired connection 1 », le Wi-Fi n'apparaissait nulle part. Le
//!   réseau Wi-Fi utilisé est maintenant dit à part (`wifi_nom`) ;
//! - « se connecter si le mot de passe est déjà mis, et si erreur il
//!   redemande quand même » : un profil enregistré est ouvert tel quel ; s'il
//!   échoue, la page redemande le mot de passe, et un mot de passe donné
//!   REMPLACE l'ancien profil (supprimé d'abord : nmcli réutiliserait le faux) ;
//! - « oublier un réseau » : `connexions_wifi_oublier`, profils retrouvés par
//!   leur SSID (le nom d'un profil n'est pas toujours celui du réseau) ;
//! - Bluetooth : les noms (un appareil sans nom — l'adresse en guise de nom —
//!   n'est plus montré), le genre (enceinte, casque, clavier…), une enceinte
//!   associée PENDANT une recherche (BlueZ oublie un appareil découvert 30 s
//!   après la fin de la recherche), et qui devient la sortie du son ;
//!   « Oublier » un appareil associé.

use crate::premier::decouper;
use serde::Serialize;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

#[cfg(test)]
thread_local! {
    /// Les essais remplacent nmcli, bluetoothctl et pactl par des faux.
    static CHEMIN: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

fn commande(programme: &str) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(programme);
    #[cfg(test)]
    CHEMIN.with(|p| {
        if let Some(p) = &*p.borrow() {
            c.env("PATH", p);
        }
    });
    c
}

/// Un programme, avec une limite de temps : `timeout` le tue au-delà.
fn avec_delai(secondes: u32, programme: &str, arguments: &[&str]) -> Option<Output> {
    commande("timeout")
        .arg(secondes.to_string())
        .arg(programme)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .ok()
}

fn texte(o: &Option<Output>) -> String {
    o.as_ref().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

fn reussi(o: &Option<Output>) -> bool {
    o.as_ref().map(|o| o.status.success()).unwrap_or(false)
}

fn nmcli(secondes: u32, arguments: &[&str]) -> Option<Output> {
    avec_delai(secondes, "nmcli", arguments)
}

fn bluetoothctl(secondes: u32, arguments: &[&str]) -> Option<Output> {
    avec_delai(secondes, "bluetoothctl", arguments)
}

fn pactl(arguments: &[&str]) -> String {
    texte(&avec_delai(8, "pactl", arguments))
}

#[derive(Serialize, Default)]
pub struct Lien {
    connecte: bool,
    par: String,
    nom: String,
}

pub(crate) fn lien_de(appareils: &str) -> Lien {
    for ligne in appareils.lines() {
        let c = decouper(ligne);
        if c.len() >= 3 && c[1] == "connected" && (c[0] == "wifi" || c[0] == "ethernet") {
            return Lien { connecte: true, par: if c[0] == "wifi" { "Wi-Fi" } else { "câble" }.into(), nom: c[2].clone() };
        }
    }
    Lien::default()
}

/// Le réseau Wi-Fi utilisé, d'après `nmcli -t -f ACTIVE,SSID device wifi`.
pub(crate) fn wifi_actif_de(sortie: &str) -> String {
    sortie
        .lines()
        .map(decouper)
        .find(|c| c.len() >= 2 && c[0] == "yes" && !c[1].is_empty())
        .map(|c| c[1].clone())
        .unwrap_or_default()
}

#[derive(Serialize)]
pub struct Etat {
    lien: Lien,
    /// Le réseau Wi-Fi utilisé, même quand le câble passe avant lui.
    wifi_nom: String,
    wifi_la: bool,
    wifi_on: bool,
    bt_la: bool,
    bt_on: bool,
}

#[tauri::command]
pub async fn connexions_etat() -> Etat {
    let appareils = texte(&nmcli(20, &["-t", "-f", "TYPE,STATE,CONNECTION", "device"]));
    let types = texte(&nmcli(20, &["-t", "-f", "TYPE", "device"]));
    let wifi_la = types.lines().any(|l| l.trim() == "wifi");
    let wifi_nom = if wifi_la {
        wifi_actif_de(&texte(&nmcli(20, &["-t", "-f", "ACTIVE,SSID", "device", "wifi", "list", "--rescan", "no"])))
    } else {
        String::new()
    };
    let liste_bt = bluetoothctl(8, &["list"]);
    let bt_la = reussi(&liste_bt) && !texte(&liste_bt).trim().is_empty();
    Etat {
        lien: lien_de(&appareils),
        wifi_nom,
        wifi_la,
        wifi_on: texte(&nmcli(20, &["radio", "wifi"])).contains("enabled"),
        bt_la,
        // Sans matériel, on ne demande même pas : `show` attendrait son délai.
        bt_on: bt_la && texte(&bluetoothctl(8, &["show"])).contains("Powered: yes"),
    }
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Reseau {
    nom: String,
    force: u32,
    protege: bool,
    connecte: bool,
    /// Un mot de passe est enregistré : on peut l'oublier.
    connu: bool,
}

pub(crate) fn reseaux_de(sortie: &str, connus: &[String]) -> Vec<Reseau> {
    let mut vus = std::collections::HashSet::new();
    let mut trouves = Vec::new();
    for ligne in sortie.lines() {
        let c = decouper(ligne);
        if c.len() < 4 || c[1].is_empty() || !vus.insert(c[1].clone()) {
            continue;
        }
        let securite = c[3].trim();
        trouves.push(Reseau {
            nom: c[1].clone(),
            force: c[2].parse().unwrap_or(0),
            protege: !securite.is_empty() && securite != "--",
            connecte: c[0].trim() == "*",
            connu: connus.contains(&c[1]),
        });
    }
    trouves.sort_by(|a, b| b.connecte.cmp(&a.connecte).then(b.connu.cmp(&a.connu)).then(b.force.cmp(&a.force)));
    trouves
}

/// Les profils Wi-Fi d'une liste `nmcli -t -f NAME,TYPE connection show`.
pub(crate) fn noms_wifi_de(liste: &str) -> Vec<String> {
    liste
        .lines()
        .map(decouper)
        .filter(|c| c.len() >= 2 && c[1] == "802-11-wireless" && !c[0].is_empty())
        .map(|c| c[0].clone())
        .collect()
}

/// Les profils Wi-Fi enregistrés : (nom du profil, SSID du réseau).
fn profils_wifi() -> Vec<(String, String)> {
    let liste = texte(&nmcli(20, &["-t", "-f", "NAME,TYPE", "connection", "show"]));
    noms_wifi_de(&liste)
        .into_iter()
        .map(|nom| {
            // `-g` échappe « : » comme `-t` : on recolle les morceaux.
            let ssid = texte(&nmcli(10, &["-g", "802-11-wireless.ssid", "connection", "show", "id", &nom]));
            let ssid = ssid.lines().next().map(|l| decouper(l).join(":")).unwrap_or_default();
            let ssid = if ssid.trim().is_empty() { nom.clone() } else { ssid };
            (nom, ssid)
        })
        .collect()
}

fn profils_de(ssid: &str) -> Vec<String> {
    profils_wifi().into_iter().filter(|(_, s)| s == ssid).map(|(p, _)| p).collect()
}

#[tauri::command]
pub async fn connexions_wifi_liste() -> Vec<Reseau> {
    let o = nmcli(25, &["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list"]);
    if !reussi(&o) {
        return Vec::new();
    }
    let connus: Vec<String> = profils_wifi().into_iter().map(|(_, s)| s).collect();
    reseaux_de(&texte(&o), &connus)
}

/// Les réseaux enregistrés qui ne sont pas à portée : pour pouvoir les oublier.
#[tauri::command]
pub async fn connexions_wifi_enregistres() -> Vec<String> {
    let mut ssids: Vec<String> = profils_wifi().into_iter().map(|(_, s)| s).collect();
    ssids.sort_by_key(|s| s.to_lowercase());
    ssids.dedup();
    ssids
}

#[tauri::command]
pub async fn connexions_wifi_radio(actif: bool) {
    nmcli(20, &["radio", "wifi", if actif { "on" } else { "off" }]);
}

/// Faut-il demander un mot de passe ? Seulement si le réseau est protégé ET
/// qu'aucun n'est déjà enregistré pour lui.
#[tauri::command]
pub async fn connexions_wifi_connu(nom: String) -> bool {
    !profils_de(&nom).is_empty()
}

pub(crate) fn refus(message: &str) -> String {
    if message.contains("Secrets were required") || message.contains("no secrets") {
        "Ce réseau demande un mot de passe.".into()
    } else if message.to_lowercase().contains("invalid") || message.contains("802-11-wireless-security") {
        "Mot de passe refusé.".into()
    } else if message.contains("No network with SSID") {
        "Ce réseau n'est plus à portée.".into()
    } else if message.trim().is_empty() {
        "La connexion a échoué.".into()
    } else {
        message.trim().to_string()
    }
}

/// Se connecter. Sans mot de passe : le profil enregistré, s'il y en a un.
/// Avec : l'ancien profil part d'abord — s'il est faux, nmcli le
/// réutiliserait —, puis un profil neuf. En cas d'échec, la page redemande
/// le mot de passe d'un réseau protégé, qu'il ait été enregistré ou non.
#[tauri::command]
pub async fn connexions_wifi_connecter(nom: String, mot_de_passe: String) -> Result<String, String> {
    let profils = profils_de(&nom);
    let o = if !mot_de_passe.is_empty() {
        for p in &profils {
            nmcli(20, &["connection", "delete", "id", p]);
        }
        nmcli(60, &["device", "wifi", "connect", &nom, "password", &mot_de_passe])
    } else if let Some(p) = profils.first() {
        nmcli(60, &["--wait", "45", "connection", "up", "id", p])
    } else {
        nmcli(60, &["device", "wifi", "connect", &nom])
    };
    match o {
        None => Err("Le gestionnaire de réseau n'a pas répondu.".into()),
        Some(o) if !o.status.success() => Err(refus(&String::from_utf8_lossy(&o.stderr))),
        Some(_) => Ok(format!("Connecté à {nom}.")),
    }
}

#[tauri::command]
pub async fn connexions_wifi_deconnecter(nom: String) -> Result<String, String> {
    let profil = profils_de(&nom).into_iter().next().unwrap_or_else(|| nom.clone());
    if reussi(&nmcli(30, &["connection", "down", "id", &profil])) {
        Ok(format!("Déconnecté de {nom}."))
    } else {
        Err("La déconnexion a échoué.".into())
    }
}

/// Oublier un réseau : chacun de ses profils, et donc son mot de passe.
#[tauri::command]
pub async fn connexions_wifi_oublier(nom: String) -> Result<String, String> {
    let profils = profils_de(&nom);
    if profils.is_empty() {
        return Err("Ce réseau n'est pas enregistré.".into());
    }
    let tous = profils.iter().fold(true, |ok, p| reussi(&nmcli(20, &["connection", "delete", "id", p])) && ok);
    if tous {
        Ok(format!("{nom} est oublié : son mot de passe est effacé."))
    } else {
        Err("Ce réseau n'a pas pu être oublié.".into())
    }
}

// ---- Bluetooth ------------------------------------------------------------------

#[derive(Serialize, Debug, PartialEq, Default, Clone)]
pub struct Appareil {
    adresse: String,
    nom: String,
    connecte: bool,
    apparie: bool,
    /// « audio », « clavier », « souris », « manette », « telephone »… ou vide.
    genre: String,
}

/// Une adresse Bluetooth : six paires hexadécimales. On ne passe rien d'autre
/// à bluetoothctl.
pub(crate) fn adresse_valide(a: &str) -> bool {
    let p: Vec<&str> = a.split(':').collect();
    p.len() == 6 && p.iter().all(|x| x.len() == 2 && x.chars().all(|c| c.is_ascii_hexdigit()))
}

/// BlueZ nomme un appareil sans nom par son adresse (« AA-BB-CC-DD-EE-FF ») :
/// pas un nom à montrer.
pub(crate) fn sans_nom(nom: &str, adresse: &str) -> bool {
    let n = nom.trim();
    n.is_empty() || n.replace('-', ":").eq_ignore_ascii_case(adresse)
}

pub(crate) fn genre_de(icone: &str) -> &'static str {
    match icone.trim() {
        i if i.starts_with("audio") => "audio",
        "input-keyboard" => "clavier",
        "input-mouse" | "input-tablet" => "souris",
        "input-gaming" => "manette",
        "phone" => "telephone",
        "computer" => "ordinateur",
        _ => "",
    }
}

/// Les adresses et noms de `bluetoothctl devices`.
pub(crate) fn adresses_de(sortie: &str) -> Vec<(String, String)> {
    sortie
        .lines()
        .filter_map(|l| {
            let mut c = l.trim().splitn(3, char::is_whitespace);
            if c.next()? != "Device" {
                return None;
            }
            let adresse = c.next()?.to_string();
            adresse_valide(&adresse).then(|| (adresse, c.next().unwrap_or("").trim().to_string()))
        })
        .collect()
}

/// Un appareil d'après `bluetoothctl info <adresse>`.
pub(crate) fn appareil_de(adresse: &str, nom_liste: &str, info: &str) -> Appareil {
    let mut a = Appareil { adresse: adresse.to_string(), nom: nom_liste.to_string(), ..Default::default() };
    let mut alias = String::new();
    let mut audio = false;
    for ligne in info.lines().map(str::trim) {
        if let Some(v) = ligne.strip_prefix("Name:") {
            a.nom = v.trim().to_string();
        } else if let Some(v) = ligne.strip_prefix("Alias:") {
            alias = v.trim().to_string();
        } else if let Some(v) = ligne.strip_prefix("Icon:") {
            a.genre = genre_de(v).to_string();
        } else if let Some(v) = ligne.strip_prefix("Paired:") {
            a.apparie = v.trim() == "yes";
        } else if let Some(v) = ligne.strip_prefix("Connected:") {
            a.connecte = v.trim() == "yes";
        } else if ligne.starts_with("UUID: Audio Sink") || ligne.starts_with("UUID: Headset") || ligne.starts_with("UUID: Handsfree") {
            audio = true;
        }
    }
    // Le nom que la personne a donné (Alias) passe avant celui de l'appareil.
    if !sans_nom(&alias, adresse) {
        a.nom = alias;
    }
    if a.genre.is_empty() && audio {
        a.genre = "audio".into();
    }
    a
}

/// Ce que la page montre : les appareils qui ont un nom, ou déjà associés ;
/// connectés, puis associés, puis le son, puis l'ordre alphabétique.
pub(crate) fn a_montrer(mut appareils: Vec<Appareil>) -> Vec<Appareil> {
    appareils.retain(|a| a.connecte || a.apparie || !sans_nom(&a.nom, &a.adresse));
    for a in &mut appareils {
        if sans_nom(&a.nom, &a.adresse) {
            a.nom = "Appareil sans nom".into();
        }
    }
    appareils.sort_by(|a, b| {
        b.connecte
            .cmp(&a.connecte)
            .then(b.apparie.cmp(&a.apparie))
            .then((b.genre == "audio").cmp(&(a.genre == "audio")))
            .then(a.nom.to_lowercase().cmp(&b.nom.to_lowercase()))
    });
    appareils
}

fn info(adresse: &str, nom: &str) -> Appareil {
    appareil_de(adresse, nom, &texte(&bluetoothctl(5, &["info", adresse])))
}

#[tauri::command]
pub async fn connexions_bt_liste() -> Vec<Appareil> {
    let tous = bluetoothctl(20, &["devices"]);
    if !reussi(&tous) {
        return Vec::new();
    }
    let appareils = adresses_de(&texte(&tous)).into_iter().take(40).map(|(adresse, nom)| info(&adresse, &nom)).collect();
    a_montrer(appareils)
}

#[tauri::command]
pub async fn connexions_bt_radio(actif: bool) {
    bluetoothctl(20, &["power", if actif { "on" } else { "off" }]);
}

/// Dix secondes de recherche : six ne suffisaient pas pour que les noms
/// arrivent (un appareil se montre d'abord par son adresse).
#[tauri::command]
pub async fn connexions_bt_chercher() {
    avec_delai(20, "bluetoothctl", &["--timeout", "10", "scan", "on"]);
}

/// La sortie du son d'un appareil Bluetooth : `bluez_output.AA_BB_….N`.
pub(crate) fn sortie_bt_de(sorties: &str, adresse: &str) -> Option<String> {
    let cle = format!("bluez_output.{}", adresse.replace(':', "_")).to_lowercase();
    sorties
        .lines()
        .filter_map(|l| l.split('\t').nth(1).or_else(|| l.split_whitespace().nth(1)))
        .find(|nom| nom.to_lowercase().starts_with(&cle))
        .map(str::to_string)
}

/// Le son sort par là, et ce qui joue déjà y passe.
fn choisir_sortie(sortie: &str) {
    pactl(&["set-default-sink", sortie]);
    for ligne in pactl(&["list", "short", "sink-inputs"]).lines() {
        if let Some(index) = ligne.split_whitespace().next() {
            pactl(&["move-sink-input", index, sortie]);
        }
    }
}

#[tauri::command]
pub async fn connexions_bt_lier(adresse: String, nom: String) -> Result<String, String> {
    if !adresse_valide(&adresse) {
        return Err("Adresse Bluetooth invalide.".into());
    }
    let avant = info(&adresse, &nom);
    if !avant.apparie {
        // BlueZ oublie un appareil découvert 30 s après la fin de la recherche
        // (TemporaryTimeout) : on cherche PENDANT l'association.
        let mut recherche = commande("bluetoothctl")
            .args(["--timeout", "40", "scan", "on"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok();
        std::thread::sleep(Duration::from_secs(3));
        bluetoothctl(20, &["trust", &adresse]);
        let appairage = bluetoothctl(40, &["pair", &adresse]);
        if let Some(r) = recherche.as_mut() {
            let _ = r.kill();
            let _ = r.wait();
        }
        if !texte(&appairage).contains("Pairing successful") && !info(&adresse, &nom).apparie {
            return Err("L'association a échoué. Mets l'appareil en mode association (souvent : maintenir son bouton Bluetooth), puis réessaie.".into());
        }
    } else {
        bluetoothctl(20, &["trust", &adresse]);
    }
    let connexion = bluetoothctl(40, &["connect", &adresse]);
    let apres = info(&adresse, &nom);
    if !texte(&connexion).contains("Connection successful") && !apres.connecte {
        return Err("La connexion a échoué. Vérifie que l'appareil est allumé et proche.".into());
    }
    let nom_lisible = if sans_nom(&apres.nom, &adresse) { nom } else { apres.nom.clone() };
    if apres.genre == "audio" || avant.genre == "audio" {
        // PipeWire crée la sortie quelques secondes après la connexion.
        for _ in 0..10 {
            if let Some(sortie) = sortie_bt_de(&pactl(&["list", "short", "sinks"]), &adresse) {
                choisir_sortie(&sortie);
                return Ok(format!("Connecté à {nom_lisible} : le son sort maintenant par là."));
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    Ok(format!("Connecté à {nom_lisible}."))
}

#[tauri::command]
pub async fn connexions_bt_delier(adresse: String, nom: String) -> Result<String, String> {
    if !adresse_valide(&adresse) {
        return Err("Adresse Bluetooth invalide.".into());
    }
    match bluetoothctl(30, &["disconnect", &adresse]) {
        None => Err("Le Bluetooth n'a pas répondu.".into()),
        Some(_) => Ok(format!("Déconnecté de {nom}.")),
    }
}

/// Oublier un appareil : il faudra l'associer de nouveau.
#[tauri::command]
pub async fn connexions_bt_oublier(adresse: String, nom: String) -> Result<String, String> {
    if !adresse_valide(&adresse) {
        return Err("Adresse Bluetooth invalide.".into());
    }
    let o = bluetoothctl(20, &["remove", &adresse]);
    if reussi(&o) || texte(&o).contains("Device has been removed") {
        Ok(format!("{nom} est oublié."))
    } else {
        Err("Cet appareil n'a pas pu être oublié.".into())
    }
}

#[cfg(test)]
mod essais {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn reseaux_tries_sans_doublon() {
        let connus = vec!["Voisin".to_string()];
        let r = reseaux_de(" :Voisin:40:WPA2\n*:Box\\:5G:72:WPA2\n :Voisin:30:WPA2\n :Cafe:90:\n :Libre:20:--\n ::10:\n", &connus);
        let noms: Vec<_> = r.iter().map(|r| r.nom.as_str()).collect();
        // Le réseau utilisé, puis ceux qu'on connaît, puis la force.
        assert_eq!(noms, ["Box:5G", "Voisin", "Cafe", "Libre"]);
        assert!(r[0].connecte && r[0].protege && !r[0].connu);
        assert!(r[1].connu);
        assert!(!r[2].protege);
        assert!(!r.iter().find(|x| x.nom == "Libre").unwrap().protege);
    }

    #[test]
    fn lien() {
        let l = lien_de("loopback:connected (externally):lo\nethernet:connected:Câble 1\n");
        assert!(l.connecte && l.par == "câble" && l.nom == "Câble 1");
        assert!(!lien_de("wifi:disconnected:\n").connecte);
    }

    #[test]
    fn le_wifi_utilise_meme_avec_le_cable() {
        assert_eq!(wifi_actif_de("no:Voisin\nyes:Famille Belgacem\nno:\n"), "Famille Belgacem");
        assert_eq!(wifi_actif_de("yes:Box\\:5G\n"), "Box:5G");
        assert_eq!(wifi_actif_de("no:Voisin\n"), "");
    }

    #[test]
    fn profils_wifi_seulement() {
        let n = noms_wifi_de("Wired connection 1:802-3-ethernet\nFamille Belgacem:802-11-wireless\nlo:loopback\nBox\\:5G:802-11-wireless\n");
        assert_eq!(n, ["Famille Belgacem", "Box:5G"]);
    }

    #[test]
    fn appareils_nommes_et_tries() {
        let liste = adresses_de("Device AA:BB:CC:DD:EE:FF JBL Flip 5\nDevice 11:22:33:44:55:66 11-22-33-44-55-66\nController 00:00:00:00:00:00 x\nDevice zz pas une adresse\n");
        assert_eq!(liste.len(), 2);
        let enceinte = appareil_de("AA:BB:CC:DD:EE:FF", "JBL Flip 5", "Device AA:BB:CC:DD:EE:FF (public)\n\tName: JBL Flip 5\n\tAlias: JBL Flip 5\n\tIcon: audio-card\n\tPaired: no\n\tConnected: no\n");
        assert_eq!(enceinte.genre, "audio");
        let anonyme = appareil_de("11:22:33:44:55:66", "11-22-33-44-55-66", "\tAlias: 11-22-33-44-55-66\n\tPaired: no\n");
        let clavier = appareil_de("12:34:56:78:9A:BC", "", "\tName: K380\n\tAlias: Clavier salon\n\tIcon: input-keyboard\n\tPaired: yes\n\tConnected: yes\n");
        assert_eq!((clavier.nom.as_str(), clavier.genre.as_str()), ("Clavier salon", "clavier"));
        let casque = appareil_de("98:76:54:32:10:FE", "Casque", "\tName: Casque\n\tPaired: yes\n\tUUID: Audio Sink (0000110b-0000-1000-8000-00805f9b34fb)\n");
        assert_eq!(casque.genre, "audio");
        let vus = a_montrer(vec![anonyme.clone(), enceinte, clavier, casque]);
        let noms: Vec<_> = vus.iter().map(|a| a.nom.as_str()).collect();
        // L'appareil sans nom disparaît ; connecté, associé (le son d'abord), puis le reste.
        assert_eq!(noms, ["Clavier salon", "Casque", "JBL Flip 5"]);
        // Associé mais sans nom : il reste, sous un nom lisible.
        let associe = Appareil { apparie: true, ..anonyme };
        assert_eq!(a_montrer(vec![associe])[0].nom, "Appareil sans nom");
    }

    #[test]
    fn sans_nom_reconnu() {
        assert!(sans_nom("AA-BB-CC-DD-EE-FF", "aa:bb:cc:dd:ee:ff"));
        assert!(sans_nom("  ", "AA:BB:CC:DD:EE:FF"));
        assert!(!sans_nom("JBL", "AA:BB:CC:DD:EE:FF"));
    }

    #[test]
    fn sortie_d_une_enceinte() {
        let s = "36\talsa_output.usb-casque.analog-stereo\tPipeWire\ts16le 2ch 48000Hz\tRUNNING\n61\tbluez_output.AA_BB_CC_DD_EE_FF.1\tPipeWire\ts16le 2ch 48000Hz\tSUSPENDED\n";
        assert_eq!(sortie_bt_de(s, "AA:BB:CC:DD:EE:FF").as_deref(), Some("bluez_output.AA_BB_CC_DD_EE_FF.1"));
        assert_eq!(sortie_bt_de(s, "11:22:33:44:55:66"), None);
    }

    #[test]
    fn adresses() {
        assert!(adresse_valide("AA:bb:01:23:45:67"));
        assert!(!adresse_valide("AA:bb:01:23:45"));
        assert!(!adresse_valide("--help"));
    }

    #[test]
    fn refus_traduits() {
        assert_eq!(refus("Error: Secrets were required, but not provided."), "Ce réseau demande un mot de passe.");
        assert_eq!(refus("802-11-wireless-security.psk: property is invalid"), "Mot de passe refusé.");
        assert_eq!(refus("Error: No network with SSID 'X' found."), "Ce réseau n'est plus à portée.");
        assert_eq!(refus(""), "La connexion a échoué.");
    }

    // ---- Avec de faux nmcli, bluetoothctl et pactl ----------------------------
    //
    // Pas de Wi-Fi ni de Bluetooth dans la CI : les faux programmes répondent
    // comme les vrais (sorties relevées chez Grenofar le 27 novembre) et
    // notent chaque appel. On vérifie CE QUI EST DEMANDÉ au système.

    const FAUX_NMCLI: &str = r#"#!/bin/sh
echo "nmcli $*" >> "$JOURNAL"
case "$*" in
  "-t -f NAME,TYPE connection show") printf 'Wired connection 1:802-3-ethernet\nFamille Belgacem:802-11-wireless\nVieux réseau:802-11-wireless\n' ;;
  "-g 802-11-wireless.ssid connection show id Famille Belgacem") echo "Famille Belgacem" ;;
  "-g 802-11-wireless.ssid connection show id Vieux réseau") echo "Ancienne Box" ;;
  "--wait 45 connection up id Famille Belgacem")
    [ -f "$ICI/bon" ] && exit 0
    echo "Error: Connection activation failed: Secrets were required, but not provided." >&2; exit 4 ;;
  "device wifi connect Famille Belgacem password bon") touch "$ICI/bon"; echo "Device 'wlan0' successfully activated." ;;
  "device wifi connect Famille Belgacem password faux") echo "Error: Connection activation failed: 802-11-wireless-security.psk: property is invalid" >&2; exit 4 ;;
  "connection delete id "*) echo "Connection successfully deleted." ;;
  *) exit 0 ;;
esac
"#;

    const FAUX_BLUETOOTHCTL: &str = r#"#!/bin/sh
echo "bluetoothctl $*" >> "$JOURNAL"
case "$*" in
  "devices") printf 'Device AA:BB:CC:DD:EE:FF JBL Flip 5\nDevice 11:22:33:44:55:66 11-22-33-44-55-66\n' ;;
  "info AA:BB:CC:DD:EE:FF")
    printf '\tName: JBL Flip 5\n\tAlias: JBL Flip 5\n\tIcon: audio-card\n'
    if [ -f "$ICI/associe" ]; then echo "	Paired: yes"; else echo "	Paired: no"; fi
    if [ -f "$ICI/connecte" ]; then echo "	Connected: yes"; else echo "	Connected: no"; fi ;;
  "info 11:22:33:44:55:66") printf '\tAlias: 11-22-33-44-55-66\n\tPaired: no\n\tConnected: no\n' ;;
  "--timeout 40 scan on") echo recherche >> "$ICI/recherches"; sleep 30 ;;
  "pair AA:BB:CC:DD:EE:FF") [ -f "$ICI/recherches" ] || { echo "Device AA:BB:CC:DD:EE:FF not available"; exit 1; }; touch "$ICI/associe"; echo "Pairing successful" ;;
  "connect AA:BB:CC:DD:EE:FF") touch "$ICI/connecte"; echo "Connection successful" ;;
  "remove AA:BB:CC:DD:EE:FF") rm -f "$ICI/associe" "$ICI/connecte"; echo "Device has been removed" ;;
  *) exit 0 ;;
esac
"#;

    const FAUX_PACTL: &str = r#"#!/bin/sh
echo "pactl $*" >> "$JOURNAL"
case "$*" in
  "list short sinks")
    printf '36\talsa_output.usb-casque.analog-stereo\tPipeWire\ts16le 2ch 48000Hz\tRUNNING\n'
    [ -f "$ICI/connecte" ] && printf '61\tbluez_output.AA_BB_CC_DD_EE_FF.1\tPipeWire\ts16le 2ch 48000Hz\tSUSPENDED\n' ;;
  "list short sink-inputs") printf '85\t36\t84\tPipeWire\tfloat32le 2ch 48000Hz\n' ;;
  *) exit 0 ;;
esac
"#;

    fn bac_a_sable() -> (std::path::PathBuf, std::path::PathBuf) {
        let ici = std::env::temp_dir().join(format!("grenos-connexions-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&ici);
        std::fs::create_dir_all(&ici).unwrap();
        for (nom, contenu) in [("nmcli", FAUX_NMCLI), ("bluetoothctl", FAUX_BLUETOOTHCTL), ("pactl", FAUX_PACTL)] {
            let f = ici.join(nom);
            std::fs::write(&f, contenu.replace("$ICI", &ici.to_string_lossy()).replace("$JOURNAL", &ici.join("journal").to_string_lossy())).unwrap();
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let chemin = format!("{}:{}", ici.display(), std::env::var("PATH").unwrap_or_default());
        CHEMIN.with(|c| *c.borrow_mut() = Some(chemin));
        (ici.clone(), ici.join("journal"))
    }

    fn journal(j: &std::path::Path) -> Vec<String> {
        std::fs::read_to_string(j).unwrap_or_default().lines().map(str::to_string).collect()
    }

    fn attendre<F: std::future::Future>(f: F) -> F::Output {
        tauri::async_runtime::block_on(f)
    }

    #[test]
    fn wifi_mot_de_passe_enregistre_puis_redemande() {
        let (ici, j) = bac_a_sable();
        // Le mot de passe enregistré : pas de question, le profil est ouvert.
        assert!(attendre(connexions_wifi_connu("Famille Belgacem".into())));
        assert!(!attendre(connexions_wifi_connu("Voisin".into())));
        // … et il est faux : l'erreur revient, la page redemandera.
        let e = attendre(connexions_wifi_connecter("Famille Belgacem".into(), String::new())).unwrap_err();
        assert_eq!(e, "Ce réseau demande un mot de passe.");
        assert!(journal(&j).contains(&"nmcli --wait 45 connection up id Famille Belgacem".to_string()));
        // Un mauvais mot de passe : refusé, dit en français.
        let e = attendre(connexions_wifi_connecter("Famille Belgacem".into(), "faux".into())).unwrap_err();
        assert_eq!(e, "Mot de passe refusé.");
        // Le bon : l'ancien profil est supprimé AVANT, puis la connexion.
        std::fs::remove_file(&j).unwrap();
        let ok = attendre(connexions_wifi_connecter("Famille Belgacem".into(), "bon".into())).unwrap();
        assert_eq!(ok, "Connecté à Famille Belgacem.");
        let appels = journal(&j);
        let supprime = appels.iter().position(|l| l == "nmcli connection delete id Famille Belgacem").expect("ancien profil supprimé");
        let connecte = appels.iter().position(|l| l == "nmcli device wifi connect Famille Belgacem password bon").expect("connexion");
        assert!(supprime < connecte, "{appels:?}");
        // Et ensuite, sans rien redemander.
        assert!(attendre(connexions_wifi_connecter("Famille Belgacem".into(), String::new())).is_ok());
        let _ = std::fs::remove_dir_all(ici);
    }

    #[test]
    fn wifi_oublier_par_le_nom_du_reseau() {
        let (ici, j) = bac_a_sable();
        // Le profil « Vieux réseau » porte le SSID « Ancienne Box ».
        assert_eq!(attendre(connexions_wifi_enregistres()), ["Ancienne Box", "Famille Belgacem"]);
        let ok = attendre(connexions_wifi_oublier("Ancienne Box".into())).unwrap();
        assert_eq!(ok, "Ancienne Box est oublié : son mot de passe est effacé.");
        assert!(journal(&j).contains(&"nmcli connection delete id Vieux réseau".to_string()));
        assert!(attendre(connexions_wifi_oublier("Inconnu".into())).is_err());
        let _ = std::fs::remove_dir_all(ici);
    }

    #[test]
    fn bluetooth_enceinte_associee_et_choisie() {
        let (ici, j) = bac_a_sable();
        let liste = attendre(connexions_bt_liste());
        assert_eq!(liste.len(), 1, "l'appareil sans nom n'est pas montré : {liste:?}");
        assert_eq!((liste[0].nom.as_str(), liste[0].genre.as_str(), liste[0].apparie), ("JBL Flip 5", "audio", false));
        // Associer : la recherche tourne PENDANT l'association.
        let ok = attendre(connexions_bt_lier("AA:BB:CC:DD:EE:FF".into(), "JBL Flip 5".into())).unwrap();
        assert_eq!(ok, "Connecté à JBL Flip 5 : le son sort maintenant par là.");
        let appels = journal(&j);
        let ordre = |l: &str| appels.iter().position(|x| x == l).unwrap_or_else(|| panic!("{l} absent : {appels:?}"));
        assert!(ordre("bluetoothctl --timeout 40 scan on") < ordre("bluetoothctl pair AA:BB:CC:DD:EE:FF"));
        assert!(ordre("bluetoothctl pair AA:BB:CC:DD:EE:FF") < ordre("bluetoothctl connect AA:BB:CC:DD:EE:FF"));
        // Le son passe sur l'enceinte, et ce qui jouait la suit.
        assert!(appels.contains(&"pactl set-default-sink bluez_output.AA_BB_CC_DD_EE_FF.1".to_string()));
        assert!(appels.contains(&"pactl move-sink-input 85 bluez_output.AA_BB_CC_DD_EE_FF.1".to_string()));
        // Oublier, puis on n'accepte qu'une adresse.
        assert_eq!(attendre(connexions_bt_oublier("AA:BB:CC:DD:EE:FF".into(), "JBL Flip 5".into())).unwrap(), "JBL Flip 5 est oublié.");
        assert!(attendre(connexions_bt_oublier("--help".into(), "x".into())).is_err());
        let _ = std::fs::remove_dir_all(ici);
    }
}
