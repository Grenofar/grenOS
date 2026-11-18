//! Le pont vers nos bibliothèques Python (`/usr/lib/grenos/pont.py`).
//!
//! La fenêtre est en Rust ; le calcul ponctuel déjà éprouvé par nos essais
//! (`sante.py`, `jeux.py`, `magasin.py`…) reste en Python, appelé à la
//! demande et rendu en JSON. Pourquoi, en détail : voir l'en-tête de pont.py.

use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

const PONT: &str = "/usr/lib/grenos/pont.py";

fn commande(fonction: &str, arguments: &Value) -> Command {
    let mut c = Command::new("python3");
    c.arg("-I").arg(PONT).arg(fonction).arg(arguments.to_string());
    c.stdin(Stdio::null());
    c
}

fn verifier(v: Value) -> Result<Value, String> {
    match v.get("erreur").and_then(Value::as_str) {
        Some(e) => Err(e.to_string()),
        None => Ok(v),
    }
}

/// Une réponse, une seule ligne JSON.
pub fn appeler(fonction: &str, arguments: Value) -> Result<Value, String> {
    let sortie = commande(fonction, &arguments).output().map_err(|e| format!("python3 : {e}"))?;
    let texte = String::from_utf8_lossy(&sortie.stdout);
    let ligne = texte.lines().last().ok_or("le pont n'a rien répondu")?;
    verifier(serde_json::from_str(ligne).map_err(|e| format!("réponse illisible : {e}"))?)
}

/// Une fonction qui rapporte au fil de l'eau : `rappel` reçoit chaque ligne,
/// la dernière porte `"fin": true`.
pub fn suivre(fonction: &str, arguments: Value, mut rappel: impl FnMut(Value)) -> Result<(), String> {
    let mut enfant = commande(fonction, &arguments)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("python3 : {e}"))?;
    let lecteur = BufReader::new(enfant.stdout.take().ok_or("pas de sortie")?);
    for ligne in lecteur.lines().map_while(Result::ok) {
        if let Ok(v) = serde_json::from_str::<Value>(&ligne) {
            rappel(verifier(v)?);
        }
    }
    let _ = enfant.wait();
    Ok(())
}
