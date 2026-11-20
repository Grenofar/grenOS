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
    // Claude (19 novembre, Grenofar : « ajoute Claude »). Le modèle le plus
    // récent d'Anthropic par défaut ; la personne peut en écrire un autre.
    ("anthropic", "Anthropic (Claude)", "claude-opus-5-5"),
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
        // L'API Messages d'Anthropic, en HTTP direct (Rust n'a pas de SDK
        // officiel ; curl, comme les autres). Les consignes « system » vont
        // dans le champ `system`, pas dans la liste. `fallbacks: "default"`
        // (bêta server-side-fallback-2026-07-01) : si le modèle décline une
        // demande, l'API la refait elle-même sur un modèle de repli adapté.
        "anthropic" => {
            let systeme: Vec<&str> = messages.iter().filter(|m| m.role == "system").map(|m| m.contenu.as_str()).collect();
            let fil: Vec<Value> = messages
                .iter()
                .filter(|m| m.role == "user" || m.role == "assistant")
                .map(|m| json!({ "role": m.role, "content": m.contenu }))
                .collect();
            let mut corps = json!({ "model": modele, "max_tokens": 16000, "messages": fil, "fallbacks": "default" });
            if !systeme.is_empty() {
                corps["system"] = json!(systeme.join("

"));
            }
            ("https://api.anthropic.com/v1/messages".into(),
             vec![format!("x-api-key: {cle}"), "anthropic-version: 2026-10-21".into(), "anthropic-beta: server-side-fallback-2026-07-01".into()],
             corps)
        }
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
    if fournisseur == "anthropic" {
        // Le refus se lit dans stop_reason, AVANT le contenu (réponse 200).
        if v.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
            return Err("Claude a décliné cette demande.".into());
        }
        // Le texte est réparti en blocs ; on ne garde que les blocs « text »
        // (les blocs de réflexion, vides par défaut, ne s'affichent pas).
        let texte: String = v.get("content").and_then(Value::as_array).map(|blocs| {
            blocs.iter().filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("")
        }).unwrap_or_default();
        return if texte.is_empty() { Err("Réponse illisible.".into()) } else { Ok(texte) };
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
            // 300 s : un modèle qui réfléchit (Claude, entre autres) peut
            // mettre plus de deux minutes sur une longue demande.
            .args(["--silent", "--show-error", "--max-time", "300", "--header"])
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

/// CLAUDE CODE (19 novembre). Grenofar : « dans GrenAI, et ça ouvre une
/// console, et aussi Claude Code ». Il n'existe pas dans Flathub ni dans
/// Debian : Anthropic le distribue par son propre installateur. Le bouton
/// ouvre donc un terminal qui, la première fois, lance CET installateur
/// officiel sous les yeux de la personne (rien ne s'installe en cachette),
/// puis démarre `claude` dans son dossier personnel. La connexion au compte
/// Claude se fait dans la console, par Claude Code lui-même.
pub const SCRIPT_CLAUDE_CODE: &str = r#"export PATH="$HOME/.local/bin:$PATH"
if ! command -v claude >/dev/null 2>&1; then
  printf '[1mClaude Code n est pas encore installe.[0m
'
  printf 'Installation par l installateur officiel d Anthropic (https://claude.ai/install.sh)...

'
  if ! curl -fsSL https://claude.ai/install.sh | bash; then
    printf '
L installation a echoue (reseau ?). Appuie sur Entree pour fermer.'
    read -r _
    exit 1
  fi
  export PATH="$HOME/.local/bin:$PATH"
fi
if [ "$(cat "$HOME/.config/grenos/mcp" 2>/dev/null)" = oui ] && ! claude mcp get grenos >/dev/null 2>&1; then
  claude mcp add --scope user grenos -- /usr/lib/grenos/grenos-mcp >/dev/null 2>&1 || true
fi
cd "$HOME" && claude
exec bash"#;

#[tauri::command]
pub fn grenai_claude_code() -> Result<(), String> {
    crate::systeme::detacher("xfce4-terminal", &["--title=Claude Code", "-x", "bash", "-lc", SCRIPT_CLAUDE_CODE])
        .map_err(|e| format!("Le terminal ne s'ouvre pas : {e}"))?;
    crate::systeme::dire("grenai : Claude Code ouvert dans un terminal");
    Ok(())
}

// ------------------------------------------- grenOS branché à Claude (MCP)
//
// Grenofar, le 20 novembre : « l'équivalent de mon MCP powershell local… plug
// and play, rien à faire, juste cliquer sur un bouton ». Le bouton de GrenAI
// inscrit /usr/lib/grenos/grenos-mcp :
//   - dans Claude Desktop : ~/.config/Claude/claude_desktop_config.json,
//     clé `mcpServers` (chemin Linux lu sur modelcontextprotocol.io, « Connect
//     to local MCP servers », le 20 novembre) — sans toucher au reste du
//     fichier, et en refusant d'écrire par-dessus un fichier illisible ;
//   - dans Claude Code, par SA commande (`claude mcp add --scope user`), plutôt
//     qu'en écrivant dans ~/.claude.json, dont la forme n'est pas documentée.
//     Claude Code pas encore installé : le réglage `mcp` le rappelle à
//     l'installation (SCRIPT_CLAUDE_CODE).

pub const SERVEUR_MCP: &str = "/usr/lib/grenos/grenos-mcp";
const NOM_MCP: &str = "grenos";

fn config_claude_desktop() -> PathBuf {
    let maison = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(maison).join(".config/Claude/claude_desktop_config.json")
}

/// Le fichier de Claude Desktop avec (ou sans) notre serveur, le reste gardé.
/// `None` : pas de fichier. Erreur : un fichier qu'on ne sait pas lire — on
/// ne l'écrase jamais.
pub fn config_avec_grenos(existant: Option<&str>, brancher: bool) -> Result<String, String> {
    let mut v: Value = match existant.map(str::trim) {
        None | Some("") => json!({}),
        Some(t) => serde_json::from_str(t).map_err(|_| {
            "Le fichier de configuration de Claude Desktop est illisible : grenOS n'y touche pas.".to_string()
        })?,
    };
    let Some(objet) = v.as_object_mut() else {
        return Err("Le fichier de configuration de Claude Desktop n'a pas la forme attendue.".into());
    };
    let serveurs = objet.entry("mcpServers").or_insert_with(|| json!({}));
    let Some(serveurs) = serveurs.as_object_mut() else {
        return Err("« mcpServers » n'a pas la forme attendue dans la configuration de Claude Desktop.".into());
    };
    if brancher {
        serveurs.insert(NOM_MCP.into(), json!({ "command": SERVEUR_MCP, "args": [] }));
    } else {
        serveurs.remove(NOM_MCP);
    }
    serde_json::to_string_pretty(&v).map_err(|e| e.to_string())
}

fn ecrire_config_desktop(brancher: bool) -> Result<(), String> {
    let chemin = config_claude_desktop();
    let existant = std::fs::read_to_string(&chemin).ok();
    if !brancher && existant.is_none() {
        return Ok(());
    }
    let neuf = config_avec_grenos(existant.as_deref(), brancher)?;
    if let Some(dossier) = chemin.parent() {
        std::fs::create_dir_all(dossier).map_err(|e| e.to_string())?;
    }
    // Écrit à côté puis renommé : une coupure n'abîme jamais le fichier de Claude.
    let temporaire = chemin.with_extension("json.grenos");
    std::fs::write(&temporaire, neuf + "\n").map_err(|e| e.to_string())?;
    std::fs::rename(&temporaire, &chemin).map_err(|e| e.to_string())
}

fn desktop_branche() -> bool {
    std::fs::read_to_string(config_claude_desktop())
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .map(|v| v["mcpServers"][NOM_MCP].is_object())
        .unwrap_or(false)
}

/// La commande `claude` (Claude Code), installée dans ~/.local/bin par
/// l'installateur officiel, ou ailleurs dans le PATH.
fn claude_code() -> Option<PathBuf> {
    let maison = std::env::var("HOME").unwrap_or_default();
    let local = PathBuf::from(&maison).join(".local/bin/claude");
    if local.is_file() {
        return Some(local);
    }
    std::env::var("PATH").ok()?.split(':').map(|d| PathBuf::from(d).join("claude")).find(|p| p.is_file())
}

fn brancher_claude_code(brancher: bool) -> Option<bool> {
    let claude = claude_code()?;
    let _ = std::process::Command::new(&claude).args(["mcp", "remove", "--scope", "user", NOM_MCP]).output();
    if !brancher {
        return Some(true);
    }
    let fait = std::process::Command::new(&claude)
        .args(["mcp", "add", "--scope", "user", NOM_MCP, "--", SERVEUR_MCP])
        .output()
        .map(|s| s.status.success())
        .unwrap_or(false);
    Some(fait)
}

#[tauri::command]
pub fn grenai_mcp_etat() -> Value {
    let reglage = std::fs::read_to_string(crate::systeme::chemin_reglage("mcp")).unwrap_or_default();
    json!({
        "branche": reglage.trim() == "oui",
        "desktop": desktop_branche(),
        "desktop_installe": std::path::Path::new("/usr/bin/claude-desktop").exists(),
        "code_installe": claude_code().is_some(),
        "serveur": std::path::Path::new(SERVEUR_MCP).exists(),
    })
}

#[tauri::command]
pub fn grenai_mcp(brancher: bool) -> Result<String, String> {
    if brancher && !std::path::Path::new(SERVEUR_MCP).exists() {
        return Err("Le serveur grenos-mcp manque : fais la mise à jour de grenOS.".into());
    }
    ecrire_config_desktop(brancher)?;
    let code = brancher_claude_code(brancher);
    crate::systeme::ecrire_reglage("mcp", if brancher { "oui" } else { "non" })?;
    crate::systeme::dire(&format!("grenai : mcp {}", if brancher { "branche" } else { "debranche" }));
    if !brancher {
        return Ok("grenOS est débranché de Claude. Redémarre Claude pour que ce soit pris en compte.".into());
    }
    let suite = match code {
        Some(true) => "Claude Code aussi.",
        Some(false) => "Claude Code n'a pas accepté l'ajout : réessaie une fois Claude Code à jour.",
        None => "Claude Code le sera dès que tu l'installeras (bouton « Claude Code »).",
    };
    Ok(format!(
        "C'est branché : Claude peut maintenant voir l'écran, piloter la souris et le clavier, et lancer des commandes sur cette machine. {suite} Redémarre Claude Desktop s'il est ouvert. Arrêt d'urgence : le pointeur dans le coin en haut à gauche."
    ))
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn le_fichier_de_claude_desktop_garde_le_reste() {
        let neuf = config_avec_grenos(None, true).unwrap();
        let v: Value = serde_json::from_str(&neuf).unwrap();
        assert_eq!(v["mcpServers"]["grenos"]["command"], SERVEUR_MCP);
        let existant = r#"{"mcpServers":{"autre":{"command":"npx"}},"theme":"dark"}"#;
        let v: Value = serde_json::from_str(&config_avec_grenos(Some(existant), true).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["autre"]["command"], "npx");
        assert_eq!(v["theme"], "dark");
        assert!(v["mcpServers"]["grenos"].is_object());
        let sans: Value = serde_json::from_str(&config_avec_grenos(Some(&v.to_string()), false).unwrap()).unwrap();
        assert!(sans["mcpServers"]["grenos"].is_null());
        assert_eq!(sans["mcpServers"]["autre"]["command"], "npx");
        // Illisible : on refuse, on n'écrase pas.
        assert!(config_avec_grenos(Some("{pas du json"), true).is_err());
        assert!(config_avec_grenos(Some("[1,2]"), true).is_err());
    }

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
    fn claude_code_n_installe_que_par_l_installateur_officiel() {
        assert!(SCRIPT_CLAUDE_CODE.contains("https://claude.ai/install.sh"));
        assert!(SCRIPT_CLAUDE_CODE.contains("command -v claude"));
        // Toute adresse du script est celle de l'installateur d'Anthropic.
        for (i, _) in SCRIPT_CLAUDE_CODE.match_indices("https://") {
            assert!(SCRIPT_CLAUDE_CODE[i..].starts_with("https://claude.ai/install.sh"));
        }
    }

    #[test]
    fn claude_recoit_le_systeme_a_part_et_rend_le_texte_seul() {
        let (adresse, entetes, corps) = requete("anthropic", "claude-opus-5-5", "k",
            &[m("system", "sois bref"), m("user", "a"), m("assistant", "b"), m("user", "c")]).unwrap();
        assert_eq!(adresse, "https://api.anthropic.com/v1/messages");
        assert!(entetes.iter().any(|e| e == "anthropic-version: 2026-10-21"));
        assert_eq!(corps["system"], "sois bref");
        assert_eq!(corps["messages"].as_array().unwrap().len(), 3);
        assert_eq!(corps["messages"][0]["role"], "user");
        assert!(corps.get("temperature").is_none());
        let rep = json!({"stop_reason":"end_turn","content":[{"type":"thinking","thinking":""},{"type":"text","text":"ok"}]});
        assert_eq!(lire_reponse("anthropic", &rep).unwrap(), "ok");
        let refus = json!({"stop_reason":"refusal","content":[]});
        assert!(lire_reponse("anthropic", &refus).unwrap_err().contains("décliné"));
        let erreur = json!({"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}});
        assert!(lire_reponse("anthropic", &erreur).unwrap_err().contains("invalid x-api-key"));
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
