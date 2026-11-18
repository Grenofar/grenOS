//! GrenAI (todo H — 18 novembre) : parler à un modèle, avec SES clés, ou à un
//! modèle qui tourne sur la machine (Ollama).
//!
//! TROIS RÈGLES, et le code les tient :
//! - **H11, jamais en arrière-plan.** Aucun service : GrenAI n'existe que
//!   pendant que sa fenêtre est ouverte, et chaque requête part d'un clic.
//! - **Les clés ne se voient pas.** Gardées dans ~/.config/grenos/grenai.json
//!   (droits 0600 : la personne seule), et données à curl par un fichier
//!   d'en-têtes temporaire (0600, effacé aussitôt) — jamais en argument, où
//!   n'importe quel programme les lirait dans la liste des processus.
//! - **On dit où partent les mots.** La page affiche le fournisseur choisi ;
//!   pour Ollama, rien ne quitte la machine.
//!
//! Les adresses sont celles des API publiques ; le modèle est un champ libre
//! (avec une proposition), parce que les noms changent plus vite que grenOS.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Reglages {
    #[serde(default)]
    pub fournisseur: String,
    #[serde(default)]
    pub modeles: HashMap<String, String>,
    #[serde(default)]
    pub cles: HashMap<String, String>,
}

pub const FOURNISSEURS: &[(&str, &str, &str)] = &[
    // clé, nom affiché, modèle proposé
    ("ollama", "Ollama (sur cette machine)", "llama3.2"),
    ("openai", "OpenAI (GPT)", "gpt-4o-mini"),
    ("gemini", "Google (Gemini)", "gemini-2.0-flash"),
    ("grok", "xAI (Grok)", "grok-2-latest"),
];

fn fichier() -> PathBuf {
    let maison = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(maison).join(".config/grenos/grenai.json")
}

fn lire() -> Reglages {
    std::fs::read_to_string(fichier()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn ecrire_prive(chemin: &std::path::Path, contenu: &str) -> Result<(), String> {
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(chemin).map_err(|e| e.to_string())?;
        f.write_all(contenu.as_bytes()).map_err(|e| e.to_string())?;
        // Un fichier qui existait avant avec d'autres droits les garde : on
        // les resserre explicitement.
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(chemin, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(chemin, contenu).map_err(|e| e.to_string())
    }
}

/// Ce que la page voit : les fournisseurs, le choix, les modèles, et
/// seulement SI une clé existe — jamais la clé elle-même.
#[tauri::command]
pub fn grenai_reglages() -> Value {
    let r = lire();
    json!({
        "fournisseur": if r.fournisseur.is_empty() { "ollama".into() } else { r.fournisseur.clone() },
        "fournisseurs": FOURNISSEURS.iter().map(|(c, n, m)| json!({
            "cle": c, "nom": n,
            "modele": r.modeles.get(*c).cloned().unwrap_or_else(|| m.to_string()),
            "a_une_cle": r.cles.get(*c).map(|k| !k.is_empty()).unwrap_or(false),
            "local": *c == "ollama",
        })).collect::<Vec<_>>(),
    })
}

#[tauri::command]
pub fn grenai_regler(fournisseur: String, modele: String, cle: Option<String>) -> Result<Value, String> {
    if !FOURNISSEURS.iter().any(|f| f.0 == fournisseur) {
        return Err("fournisseur inconnu".into());
    }
    let modele = modele.trim();
    if modele.is_empty() || modele.len() > 100 || !modele.chars().all(|c| c.is_ascii_alphanumeric() || "._:-/".contains(c)) {
        return Err("Nom de modèle illisible.".into());
    }
    let mut r = lire();
    r.fournisseur = fournisseur.clone();
    r.modeles.insert(fournisseur.clone(), modele.to_string());
    if let Some(k) = cle {
        let k = k.trim().to_string();
        if k.contains(['\n', '\r']) || k.len() > 400 {
            return Err("Clé illisible.".into());
        }
        if k.is_empty() {
            r.cles.remove(&fournisseur);
        } else {
            r.cles.insert(fournisseur.clone(), k);
        }
    }
    ecrire_prive(&fichier(), &serde_json::to_string_pretty(&r).map_err(|e| e.to_string())?)?;
    Ok(grenai_reglages())
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Message {
    pub role: String,
    pub contenu: String,
}

/// L'adresse, les en-têtes et le corps de la requête, pour chaque fournisseur.
pub fn requete(fournisseur: &str, modele: &str, cle: &str, messages: &[Message]) -> Result<(String, Vec<String>, Value), String> {
    let oai: Vec<Value> = messages.iter().map(|m| json!({ "role": m.role, "content": m.contenu })).collect();
    Ok(match fournisseur {
        "ollama" => ("http://127.0.0.1:11434/api/chat".into(), vec![], json!({ "model": modele, "messages": oai, "stream": false })),
        "openai" => ("https://api.openai.com/v1/chat/completions".into(), vec![format!("Authorization: Bearer {cle}")], json!({ "model": modele, "messages": oai })),
        "grok" => ("https://api.x.ai/v1/chat/completions".into(), vec![format!("Authorization: Bearer {cle}")], json!({ "model": modele, "messages": oai })),
        "gemini" => {
            let contenus: Vec<Value> = messages
                .iter()
                .filter(|m| m.role != "system")
                .map(|m| json!({ "role": if m.role == "assistant" { "model" } else { "user" }, "parts": [{ "text": m.contenu }] }))
                .collect();
            (format!("https://generativelanguage.googleapis.com/v1beta/models/{modele}:generateContent"), vec![format!("x-goog-api-key: {cle}")], json!({ "contents": contenus }))
        }
        _ => return Err("fournisseur inconnu".into()),
    })
}

/// La réponse, quel que soit le format du fournisseur, ou son message d'erreur.
pub fn lire_reponse(fournisseur: &str, v: &Value) -> Result<String, String> {
    if let Some(e) = v.get("error") {
        let m = e.get("message").and_then(Value::as_str).or_else(|| e.as_str()).unwrap_or("erreur inconnue");
        return Err(format!("Le fournisseur a refusé : {m}"));
    }
    let t = match fournisseur {
        "ollama" => v.pointer("/message/content"),
        "gemini" => v.pointer("/candidates/0/content/parts/0/text"),
        _ => v.pointer("/choices/0/message/content"),
    };
    t.and_then(Value::as_str).map(String::from).ok_or_else(|| "Réponse illisible.".into())
}

#[tauri::command]
pub async fn grenai_demander(messages: Vec<Message>) -> Result<String, String> {
    use std::io::Write;
    let r = lire();
    let f = if r.fournisseur.is_empty() { "ollama".to_string() } else { r.fournisseur.clone() };
    let modele = r.modeles.get(&f).cloned().unwrap_or_else(|| FOURNISSEURS.iter().find(|x| x.0 == f).map(|x| x.2.to_string()).unwrap_or_default());
    let cle = r.cles.get(&f).cloned().unwrap_or_default();
    if f != "ollama" && cle.is_empty() {
        return Err("Il faut d'abord la clé de ce fournisseur (Réglages de GrenAI).".into());
    }
    let messages: Vec<Message> = messages.into_iter().filter(|m| (m.role == "user" || m.role == "assistant" || m.role == "system") && !m.contenu.is_empty()).collect();
    let (adresse, entetes, corps) = requete(&f, &modele, &cle, &messages)?;

    // Les en-têtes (dont la clé) dans un fichier 0600, effacé à la sortie.
    let dossier = std::env::var("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    let fichier_entetes = dossier.join(format!("grenai-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
    let mut lignes = entetes.clone();
    lignes.push("Content-Type: application/json".into());
    ecrire_prive(&fichier_entetes, &lignes.join("\n"))?;
    let resultat = (|| {
        let mut enfant = std::process::Command::new("curl")
            .args(["--silent", "--show-error", "--max-time", "120", "--header"])
            .arg(format!("@{}", fichier_entetes.to_string_lossy()))
            .args(["--data-binary", "@-", &adresse])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|_| "curl n'est pas installé.".to_string())?;
        if let Some(mut e) = enfant.stdin.take() {
            let _ = e.write_all(corps.to_string().as_bytes());
        }
        enfant.wait_with_output().map_err(|e| e.to_string())
    })();
    let _ = std::fs::remove_file(&fichier_entetes);
    let sortie = resultat?;
    if !sortie.status.success() {
        let e = String::from_utf8_lossy(&sortie.stderr);
        return Err(if f == "ollama" && (e.contains("Connection refused") || e.contains("Failed to connect")) {
            "Ollama ne répond pas sur cette machine. Installe-le (GrenPlace propose Alpaca, qui l'embarque) et lance-le.".into()
        } else {
            format!("Pas de réponse : {}", e.lines().last().unwrap_or("le réseau ?"))
        });
    }
    let v: Value = serde_json::from_slice(&sortie.stdout).map_err(|_| "Réponse illisible.".to_string())?;
    lire_reponse(&f, &v)
}

/// H9 : le texte d'un fichier à résumer, choisi dans le vrai sélecteur. Les
/// PDF passent par pdftotext. Coupé à 12 000 caractères : au-delà, les
/// modèles gratuits refusent, et la page le dit.
#[cfg(target_os = "linux")]
#[tauri::command]
pub fn grenai_lire_document(fenetre: tauri::WebviewWindow) -> Result<Option<Value>, String> {
    use gtk::prelude::*;
    let parent = fenetre.gtk_window().ok();
    let boite = gtk::FileChooserNative::new(Some("Choisir un document à résumer"), parent.as_ref(), gtk::FileChooserAction::Open, Some("Choisir"), Some("Annuler"));
    let filtre = gtk::FileFilter::new();
    filtre.set_name(Some("Textes et PDF"));
    for m in ["*.txt", "*.md", "*.pdf", "*.PDF", "*.csv", "*.html"] {
        filtre.add_pattern(m);
    }
    boite.add_filter(filtre);
    let reponse = boite.run();
    let choisi = boite.filename();
    boite.destroy();
    let Some(chemin) = choisi.filter(|_| reponse == gtk::ResponseType::Accept) else { return Ok(None) };
    let texte = if chemin.extension().map(|e| e.eq_ignore_ascii_case("pdf")).unwrap_or(false) {
        let o = std::process::Command::new("pdftotext").args(["-layout", &chemin.to_string_lossy(), "-"]).output().map_err(|_| "pdftotext n'est pas installé.".to_string())?;
        String::from_utf8_lossy(&o.stdout).into_owned()
    } else {
        std::fs::read_to_string(&chemin).map_err(|e| e.to_string())?
    };
    let coupe = texte.chars().count() > 12_000;
    let texte: String = texte.chars().take(12_000).collect();
    Ok(Some(json!({ "nom": chemin.file_name().map(|n| n.to_string_lossy().into_owned()), "texte": texte, "coupe": coupe })))
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub fn grenai_lire_document(_fenetre: tauri::WebviewWindow) -> Result<Option<Value>, String> {
    Err("Seulement sous Linux.".into())
}

#[cfg(test)]
mod essais {
    use super::*;

    fn m(role: &str, c: &str) -> Message {
        Message { role: role.into(), contenu: c.into() }
    }

    #[test]
    fn la_cle_va_dans_un_en_tete_jamais_dans_l_adresse() {
        for (f, _, modele) in FOURNISSEURS {
            let (adresse, entetes, _) = requete(f, modele, "sk-SECRET", &[m("user", "bonjour")]).unwrap();
            assert!(!adresse.contains("SECRET"), "{f}");
            if *f == "ollama" {
                assert!(entetes.is_empty());
                assert!(adresse.starts_with("http://127.0.0.1"));
            } else {
                assert!(entetes.iter().any(|e| e.contains("sk-SECRET")), "{f}");
                assert!(adresse.starts_with("https://"), "{f}");
            }
        }
    }

    #[test]
    fn gemini_nomme_les_roles_a_sa_facon() {
        let (_, _, corps) = requete("gemini", "g", "k", &[m("user", "a"), m("assistant", "b")]).unwrap();
        assert_eq!(corps["contents"][1]["role"], "model");
    }

    #[test]
    fn les_reponses_de_chacun() {
        assert_eq!(lire_reponse("openai", &json!({"choices":[{"message":{"content":"ok"}}]})).unwrap(), "ok");
        assert_eq!(lire_reponse("ollama", &json!({"message":{"content":"ok"}})).unwrap(), "ok");
        assert_eq!(lire_reponse("gemini", &json!({"candidates":[{"content":{"parts":[{"text":"ok"}]}}]})).unwrap(), "ok");
        assert!(lire_reponse("openai", &json!({"error":{"message":"Incorrect API key"}})).unwrap_err().contains("Incorrect"));
        assert!(lire_reponse("grok", &json!({})).is_err());
    }
}
