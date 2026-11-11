use std::path::Path;

/// `tauri::generate_context!` reads `frontendDist` at compile time, so a clean checkout
/// must have a placeholder before the first `cargo check`. `pnpm build` overwrites it.
fn ensure_frontend_dist() {
    let dist = Path::new("../build");
    let index = dist.join("index.html");
    if index.exists() {
        return;
    }
    if let Err(err) = std::fs::create_dir_all(dist) {
        println!("cargo:warning=could not create {}: {err}", dist.display());
        return;
    }
    if let Err(err) = std::fs::write(&index, "<!doctype html><title>Weasel</title>\n") {
        println!("cargo:warning=could not write {}: {err}", index.display());
    }
}

fn main() {
    ensure_frontend_dist();
    tauri_build::build()
}
