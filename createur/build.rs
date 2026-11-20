// Le créateur doit effacer une clé et la reformater : sous Windows, c'est un
// droit d'administrateur. Le manifeste le demande dès l'ouverture (une seule
// fenêtre « Autoriser ? » de Windows), plutôt qu'au milieu du travail.
fn main() {
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("tauri-build");
}
