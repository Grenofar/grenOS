//! « Mes jeux Steam » : les jeux du compte, installés ou non.
//!
//! Le profil public ne se lit plus sans connexion (vérifié le 17 novembre :
//! la page renvoie vers /login), et l'API Web demande une clé. On lit donc ce
//! que le client Steam garde sur la machine :
//!
//! * `userdata/<compte>/config/localconfig.vdf` — la liste `apps` des jeux
//!   que ce compte a eus sur cette machine (texte VDF) ;
//! * `appcache/appinfo.vdf` — le cache binaire où Steam range le nom et le
//!   type de chaque application qu'il connaît.
//!
//! Le format binaire est lu prudemment : chaque entrée porte sa taille, donc
//! une entrée qu'on ne comprend pas est SAUTÉE, jamais fatale. Le fichier est
//! relu seulement quand il change : il pèse des dizaines de mégaoctets.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

#[derive(Clone, Debug, PartialEq)]
pub struct Appli { pub nom: String, pub genre: String }

static CACHE: Mutex<Option<(PathBuf, SystemTime, HashMap<u32, Appli>)>> = Mutex::new(None);

// ------------------------------------------------------------ appinfo.vdf

struct Lecteur<'a> { o: &'a [u8], p: usize }

impl<'a> Lecteur<'a> {
    fn u8(&mut self) -> Option<u8> { let v = *self.o.get(self.p)?; self.p += 1; Some(v) }
    fn u32(&mut self) -> Option<u32> {
        let b = self.o.get(self.p..self.p + 4)?; self.p += 4;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn sauter(&mut self, n: usize) -> Option<()> { if self.p + n > self.o.len() { return None; } self.p += n; Some(()) }
    fn chaine(&mut self) -> Option<String> {
        let fin = self.o[self.p..].iter().position(|b| *b == 0)? + self.p;
        let s = String::from_utf8_lossy(&self.o[self.p..fin]).into_owned();
        self.p = fin + 1;
        Some(s)
    }
}

/// Parcourt un objet binaire ; `chemin` est la pile des clés au-dessus.
/// Ne garde que appinfo/common/{name,type}.
fn objet(l: &mut Lecteur, table: Option<&[String]>, chemin: &mut Vec<String>, trouve: &mut Appli, fin: usize) -> Option<()> {
    loop {
        if l.p >= fin { return None; }
        let genre = l.u8()?;
        if genre == 0x08 { return Some(()); }
        let cle = match table {
            Some(t) => t.get(l.u32()? as usize)?.clone(),
            None => l.chaine()?,
        };
        match genre {
            0x00 => { chemin.push(cle); objet(l, table, chemin, trouve, fin)?; chemin.pop(); }
            0x01 => {
                let v = l.chaine()?;
                if chemin.len() == 2 && chemin[1].eq_ignore_ascii_case("common") {
                    if cle.eq_ignore_ascii_case("name") { trouve.nom = v; }
                    else if cle.eq_ignore_ascii_case("type") { trouve.genre = v; }
                }
            }
            0x02 | 0x03 | 0x04 | 0x06 => l.sauter(4)?,
            0x07 | 0x0A => l.sauter(8)?,
            0x05 => { // chaîne UTF-16 terminée par deux zéros
                loop { let a = l.u8()?; let b = l.u8()?; if a == 0 && b == 0 { break; } }
            }
            _ => return None,
        }
    }
}

pub fn lire_appinfo(octets: &[u8]) -> HashMap<u32, Appli> {
    let mut sortie = HashMap::new();
    let mut l = Lecteur { o: octets, p: 0 };
    let Some(magie) = l.u32() else { return sortie };
    let version = match magie { 0x0756_4429 => 29, 0x0756_4428 => 28, 0x0756_4427 => 27, _ => return sortie };
    if l.u32().is_none() { return sortie; } // univers
    let mut table: Option<Vec<String>> = None;
    if version >= 29 {
        let Some(b) = octets.get(8..16) else { return sortie };
        let decalage = i64::from_le_bytes(b.try_into().unwrap()) as usize;
        l.p = 16;
        let mut t = Lecteur { o: octets, p: decalage };
        if let Some(n) = t.u32() {
            let mut v = Vec::with_capacity(n.min(1_000_000) as usize);
            for _ in 0..n { match t.chaine() { Some(s) => v.push(s), None => break } }
            table = Some(v);
        }
    }
    let entete = if version >= 28 { 60 } else { 40 };
    loop {
        let Some(appid) = l.u32() else { break };
        if appid == 0 { break; }
        let Some(taille) = l.u32() else { break };
        let fin = l.p + taille as usize;
        if fin > octets.len() { break; }
        if l.sauter(entete).is_some() {
            let mut trouve = Appli { nom: String::new(), genre: String::new() };
            let mut chemin = vec![];
            // L'entrée commence par un objet racine « appinfo ».
            if l.u8() == Some(0x00) {
                let cle = match table.as_deref() { Some(t) => l.u32().and_then(|i| t.get(i as usize).cloned()), None => l.chaine() };
                if let Some(c) = cle {
                    chemin.push(c);
                    let _ = objet(&mut l, table.as_deref(), &mut chemin, &mut trouve, fin);
                }
            }
            if !trouve.nom.is_empty() { sortie.insert(appid, trouve); }
        }
        l.p = fin;
    }
    sortie
}

fn appinfo(racine: &Path) -> HashMap<u32, Appli> {
    let chemin = racine.join("appcache/appinfo.vdf");
    let Ok(quand) = fs::metadata(&chemin).and_then(|m| m.modified()) else { return HashMap::new() };
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((c, q, carte)) = cache.as_ref() {
        if *c == chemin && *q == quand { return carte.clone(); }
    }
    let carte = fs::read(&chemin).map(|o| lire_appinfo(&o)).unwrap_or_default();
    *cache = Some((chemin, quand, carte.clone()));
    carte
}

// --------------------------------------------------------- localconfig.vdf

fn jetons(texte: &str) -> Vec<String> {
    let mut v = vec![];
    let mut it = texte.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' | '}' => v.push(c.to_string()),
            '"' => {
                let mut s = String::new();
                while let Some(d) = it.next() {
                    match d { '\\' => { if let Some(e) = it.next() { s.push(e); } } '"' => break, _ => s.push(d) }
                }
                v.push(format!("\"{s}"));
            }
            _ => {}
        }
    }
    v
}

/// Les appids sous UserLocalConfigStore/Software/Valve/Steam/apps.
pub fn apps_du_compte(texte: &str) -> HashSet<u32> {
    let voulu = ["userlocalconfigstore", "software", "valve", "steam", "apps"];
    let mut pile: Vec<String> = vec![];
    let mut sortie = HashSet::new();
    let j = jetons(texte);
    let mut i = 0;
    while i < j.len() {
        let t = &j[i];
        if t == "}" { pile.pop(); i += 1; continue; }
        if let Some(cle) = t.strip_prefix('"') {
            if j.get(i + 1).map(|s| s.as_str()) == Some("{") {
                if pile.len() == 5 && pile.iter().zip(voulu).all(|(a, b)| a.eq_ignore_ascii_case(b)) {
                    if let Ok(id) = cle.parse() { sortie.insert(id); }
                }
                pile.push(cle.to_string());
                i += 2;
                continue;
            }
            i += 2; // une paire clé valeur
            continue;
        }
        i += 1;
    }
    sortie
}

/// Les jeux du compte connus sur cette machine, sauf ceux de `exclus`.
pub fn jeux_du_compte(racines: &[PathBuf], exclus: &HashSet<u32>, est_outil: impl Fn(u32, &str) -> bool) -> Vec<(u32, String)> {
    let mut sortie: HashMap<u32, String> = HashMap::new();
    for racine in racines {
        let mut ids = HashSet::new();
        if let Ok(comptes) = fs::read_dir(racine.join("userdata")) {
            for c in comptes.flatten() {
                if let Ok(t) = fs::read_to_string(c.path().join("config/localconfig.vdf")) {
                    ids.extend(apps_du_compte(&t));
                }
            }
        }
        if ids.is_empty() { continue; }
        let infos = appinfo(racine);
        for id in ids {
            if exclus.contains(&id) || sortie.contains_key(&id) { continue; }
            match infos.get(&id) {
                Some(a) if a.genre.eq_ignore_ascii_case("game") && !est_outil(id, &a.nom) => { sortie.insert(id, a.nom.clone()); }
                Some(_) => {}
                // Sans cache lisible, on ne sait pas si c'est un jeu : on le
                // montre sous son numéro, et sa jaquette dira ce que c'est.
                None if infos.is_empty() && !est_outil(id, "") => { sortie.insert(id, format!("Jeu Steam {id}")); }
                None => {}
            }
        }
    }
    let mut v: Vec<(u32, String)> = sortie.into_iter().collect();
    v.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    v
}

#[cfg(test)]
mod essais {
    use super::*;

    fn cstr(v: &mut Vec<u8>, s: &str) { v.extend_from_slice(s.as_bytes()); v.push(0); }

    /// Un appinfo.vdf v29 fabriqué : deux applications, une table de chaînes.
    #[test]
    fn appinfo_v29() {
        let table = ["appinfo", "appid", "common", "name", "type", "extended"];
        let idx = |s: &str| table.iter().position(|t| *t == s).unwrap() as u32;
        let corps = |nom: &str, genre: &str| {
            let mut kv = vec![0x00]; kv.extend(idx("appinfo").to_le_bytes());
            kv.push(0x02); kv.extend(idx("appid").to_le_bytes()); kv.extend(7u32.to_le_bytes());
            kv.push(0x00); kv.extend(idx("common").to_le_bytes());
            kv.push(0x01); kv.extend(idx("name").to_le_bytes()); cstr(&mut kv, nom);
            kv.push(0x01); kv.extend(idx("type").to_le_bytes()); cstr(&mut kv, genre);
            kv.push(0x08);
            kv.push(0x00); kv.extend(idx("extended").to_le_bytes()); kv.push(0x07); kv.extend(idx("appid").to_le_bytes()); kv.extend(9u64.to_le_bytes()); kv.push(0x08);
            kv.push(0x08); kv.push(0x08);
            kv
        };
        let mut o = vec![];
        o.extend(0x0756_4429u32.to_le_bytes()); o.extend(1u32.to_le_bytes()); o.extend(0i64.to_le_bytes());
        for (id, nom, genre) in [(730u32, "Counter-Strike 2", "game"), (228980, "Steamworks Common Redistributables", "Tool")] {
            let kv = corps(nom, genre);
            o.extend(id.to_le_bytes()); o.extend(((60 + kv.len()) as u32).to_le_bytes());
            o.extend(vec![0u8; 60]); o.extend(kv);
        }
        o.extend(0u32.to_le_bytes());
        let decalage = o.len() as i64;
        o[8..16].copy_from_slice(&decalage.to_le_bytes());
        o.extend((table.len() as u32).to_le_bytes());
        for t in table { cstr(&mut o, t); }

        let r = lire_appinfo(&o);
        assert_eq!(r.get(&730), Some(&Appli { nom: "Counter-Strike 2".into(), genre: "game".into() }));
        assert_eq!(r.get(&228980).unwrap().genre, "Tool");
        assert!(lire_appinfo(b"pas un vdf").is_empty());
        // Une entrée tronquée ne fait pas tomber la lecture.
        assert!(lire_appinfo(&o[..40]).len() <= 1);
    }

    #[test]
    fn localconfig() {
        let t = r#""UserLocalConfigStore" { "Software" { "Valve" { "Steam" { "apps" { "730" { "LastPlayed" "1" } "1245620" { } } "autre" "x" } } } "apps" { "999" { } } }"#;
        let ids = apps_du_compte(t);
        assert!(ids.contains(&730) && ids.contains(&1245620));
        assert!(!ids.contains(&999));
    }
}
