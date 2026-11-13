//! Identity, sidecar and version rules from docs/setup-contract.md §12.

use serde_json::Value;

fn read_json(rel: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn identity_matches_the_contract() {
    let conf = read_json("tauri.conf.json");
    assert_eq!(conf["productName"], "AgentMark");
    assert_eq!(conf["mainBinaryName"], "agentmark-app");
    assert_eq!(conf["identifier"], "app.agentmark.desktop");
}

/// The sidecar is added only by the bundle overlay, so plain `cargo build`
/// of the app never copies it over the CLI's own `target/<profile>/agentmark`.
#[test]
fn sidecar_comes_from_the_bundle_overlay() {
    let conf = read_json("tauri.conf.json");
    assert!(conf["bundle"].get("externalBin").is_none());
    let overlay = read_json("tauri.bundle.conf.json");
    assert_eq!(
        overlay["bundle"]["externalBin"],
        serde_json::json!(["binaries/agentmark"])
    );
}

#[test]
fn one_window_fixed_size() {
    let conf = read_json("tauri.conf.json");
    let windows = conf["app"]["windows"].as_array().unwrap();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0]["label"], "main");
    assert_eq!(windows[0]["resizable"], false);
}

/// The app carries the repo's single version (AGENTS.md, Versioning).
#[test]
fn versions_match() {
    let version = env!("CARGO_PKG_VERSION");
    assert_eq!(read_json("tauri.conf.json")["version"], version);
    assert_eq!(read_json("../package.json")["version"], version);
    let lock = read_json("../package-lock.json");
    assert_eq!(lock["version"], version);
    assert_eq!(lock["packages"][""]["version"], version);

    let cli_toml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cli/Cargo.toml"),
    )
    .unwrap();
    assert!(
        cli_toml.contains(&format!("\nversion = \"{version}\"\n")),
        "packages/cli/Cargo.toml has a different version than the app"
    );
}
