fn main() {
    // Ensure the extension dist directory exists so include_dir! compiles
    // even when the extension hasn't been built yet (e.g., CLI-only development).
    let dist = std::path::Path::new("../extension/dist");
    if !dist.exists() {
        std::fs::create_dir_all(dist).ok();
    }

    // Track manifest.json as a sentinel — a directory-level rerun-if-changed
    // only fires on directory creation/deletion, not when files inside change.
    // manifest.json is always present after an extension build and its content
    // is stable, so Vite hash-named assets trigger a rebuild via the directory
    // change when new files appear.
    println!("cargo:rerun-if-changed=../extension/dist/manifest.json");
}
