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

use crate::premier::decouper;
use serde::Serialize;
use std::process::{Command, Output, Stdio};

/// Un programme, avec une limite de temps : `timeout` le tue au-delà.
fn avec_delai(secondes: u32, programme: &str, arguments: &[&str]) -> Option<Output> {
    Command::new("timeout")
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

#[derive(Serialize)]
pub struct Etat {
    lien: Lien,
    wifi_la: bool,
    wifi_on: bool,
    bt_la: bool,
    bt_on: bool,
}

#[tauri::command]
pub async fn connexions_etat() -> Etat {
    let appareils = texte(&nmcli(20, &["-t", "-f", "TYPE,STATE,CONNECTION", "device"]));
    let types = texte(&nmcli(20, &["-t", "-f", "TYPE", "device"]));
    let liste_bt = bluetoothctl(8, &["list"]);
    let bt_la = reussi(&liste_bt) && !texte(&liste_bt).trim().is_empty();
    Etat {
        lien: lien_de(&appareils),
        wifi_la: types.lines().any(|l| l.trim() == "wifi"),
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
}

pub(crate) fn reseaux_de(sortie: &str) -> Vec<Reseau> {
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
        });
    }
    trouves.sort_by(|a, b| b.connecte.cmp(&a.connecte).then(b.force.cmp(&a.force)));
    trouves
}

#[tauri::command]
pub async fn connexions_wifi_liste() -> Vec<Reseau> {
    let o = nmcli(25, &["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list"]);
    if !reussi(&o) {
        return Vec::new();
    }
    reseaux_de(&texte(&o))
}

#[tauri::command]
pub async fn connexions_wifi_radio(actif: bool) {
    nmcli(20, &["radio", "wifi", if actif { "on" } else { "off" }]);
}

fn connu(nom: &str) -> bool {
    texte(&nmcli(20, &["-t", "-f", "NAME", "connection", "show"])).lines().any(|l| l == nom)
}

/// Faut-il demander un mot de passe ? Seulement si le réseau est protégé ET
/// qu'aucun n'est déjà enregistré pour lui.
#[tauri::command]
pub async fn connexions_wifi_connu(nom: String) -> bool {
    connu(&nom)
}

pub(crate) fn refus(message: &str) -> String {
    if message.contains("Secrets were required") || message.contains("no secrets") {
        "Ce réseau demande un mot de passe.".into()
    } else if message.to_lowercase().contains("invalid") || message.contains("802-11-wireless-security") {
        "Mot de passe refusé.".into()
    } else if message.trim().is_empty() {
        "La connexion a échoué.".into()
    } else {
        message.trim().to_string()
    }
}

#[tauri::command]
pub async fn connexions_wifi_connecter(nom: String, mot_de_passe: String) -> Result<String, String> {
    let o = if !mot_de_passe.is_empty() {
        nmcli(60, &["device", "wifi", "connect", &nom, "password", &mot_de_passe])
    } else if connu(&nom) {
        nmcli(60, &["connection", "up", &nom])
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
    if reussi(&nmcli(30, &["connection", "down", &nom])) {
        Ok(format!("Déconnecté de {nom}."))
    } else {
        Err("La déconnexion a échoué.".into())
    }
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Appareil {
    adresse: String,
    nom: String,
    connecte: bool,
}

pub(crate) fn appareils_de(tous: &str, connectes: &str) -> Vec<Appareil> {
    let lies: std::collections::HashSet<&str> = connectes
        .lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .collect();
    let mut trouves: Vec<Appareil> = tous
        .lines()
        .filter_map(|l| {
            let mut c = l.splitn(3, char::is_whitespace);
            if c.next()? != "Device" {
                return None;
            }
            let adresse = c.next()?.to_string();
            let nom = c.next()?.trim().to_string();
            Some(Appareil { connecte: lies.contains(adresse.as_str()), adresse, nom })
        })
        .collect();
    trouves.sort_by(|a, b| b.connecte.cmp(&a.connecte).then(a.nom.to_lowercase().cmp(&b.nom.to_lowercase())));
    trouves
}

#[tauri::command]
pub async fn connexions_bt_liste() -> Vec<Appareil> {
    let tous = bluetoothctl(20, &["devices"]);
    if !reussi(&tous) {
        return Vec::new();
    }
    let connectes = bluetoothctl(20, &["devices", "Connected"]);
    appareils_de(&texte(&tous), &texte(&connectes))
}

#[tauri::command]
pub async fn connexions_bt_radio(actif: bool) {
    bluetoothctl(20, &["power", if actif { "on" } else { "off" }]);
}

#[tauri::command]
pub async fn connexions_bt_chercher() {
    avec_delai(16, "bluetoothctl", &["--timeout", "6", "scan", "on"]);
}

/// Une adresse Bluetooth : six paires hexadécimales. On ne passe rien d'autre
/// à bluetoothctl.
pub(crate) fn adresse_valide(a: &str) -> bool {
    let p: Vec<&str> = a.split(':').collect();
    p.len() == 6 && p.iter().all(|x| x.len() == 2 && x.chars().all(|c| c.is_ascii_hexdigit()))
}

#[tauri::command]
pub async fn connexions_bt_lier(adresse: String, nom: String) -> Result<String, String> {
    if !adresse_valide(&adresse) {
        return Err("Adresse Bluetooth invalide.".into());
    }
    bluetoothctl(20, &["trust", &adresse]);
    let appairage = bluetoothctl(40, &["pair", &adresse]);
    let connexion = bluetoothctl(40, &["connect", &adresse]);
    if texte(&connexion).contains("Connection successful") {
        Ok(format!("Connecté à {nom}."))
    } else if texte(&appairage).contains("Failed to pair") {
        Err("L'association a échoué. Mets l'appareil en mode association.".into())
    } else {
        Err("La connexion a échoué.".into())
    }
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

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn reseaux_tries_sans_doublon() {
        let r = reseaux_de(" :Voisin:40:WPA2\n*:Box\\:5G:72:WPA2\n :Voisin:30:WPA2\n :Cafe:90:\n :Libre:20:--\n ::10:\n");
        let noms: Vec<_> = r.iter().map(|r| r.nom.as_str()).collect();
        assert_eq!(noms, ["Box:5G", "Cafe", "Voisin", "Libre"]);
        assert!(r[0].connecte && r[0].protege);
        assert!(!r[1].protege);
        assert!(!r.iter().find(|x| x.nom == "Libre").unwrap().protege);
    }

    #[test]
    fn lien() {
        let l = lien_de("loopback:connected (externally):lo\nethernet:connected:Câble 1\n");
        assert!(l.connecte && l.par == "câble" && l.nom == "Câble 1");
        assert!(!lien_de("wifi:disconnected:\n").connecte);
    }

    #[test]
    fn appareils() {
        let a = appareils_de(
            "Device AA:BB:CC:DD:EE:FF Casque Sony\nDevice 11:22:33:44:55:66 clavier\nController 00:00:00:00:00:00 x\n",
            "Device 11:22:33:44:55:66 clavier\n",
        );
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].nom, "clavier");
        assert!(a[0].connecte && !a[1].connecte);
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
        assert_eq!(refus(""), "La connexion a échoué.");
    }
}
