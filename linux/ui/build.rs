fn main() {
    // Le fichier partagé avec le secours Python : le recompiler quand il change.
    println!("cargo:rerun-if-changed=../config/includes.chroot/usr/share/grenos/premier.json");
    tauri_build::build()
}
