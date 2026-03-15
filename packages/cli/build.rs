fn main() {
    // Ensure the extension dist directory exists so include_dir! compiles
    // even when the extension hasn't been built yet (e.g., CLI-only development).
    let dist = std::path::Path::new("../extension/dist");
    if !dist.exists() {
        std::fs::create_dir_all(dist).ok();
    }
    println!("cargo:rerun-if-changed=../extension/dist");
}
