//! `agentmark install-extension` — extract the embedded Chrome extension
//! and optionally register the Chrome native messaging host.

use std::path::{Path, PathBuf};

use crate::cli::InstallExtensionArgs;
use crate::config;
use crate::extension;

const NATIVE_HOST_NAME: &str = "com.agentmark.native";

/// Entry point for `agentmark install-extension`.
pub fn run_install_extension(args: InstallExtensionArgs) -> Result<(), Box<dyn std::error::Error>> {
    let home = config::home_dir()?;
    run_install_extension_with_home(&home, &args)
}

fn run_install_extension_with_home(
    home: &Path,
    args: &InstallExtensionArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Check that we have an embedded extension
    if !extension::is_embedded() {
        return Err("No extension embedded in this build. \
             Build the extension first: cd packages/extension && npm run build"
            .into());
    }

    // 2. Resolve target directory
    let target = args
        .target_dir
        .clone()
        .unwrap_or_else(|| config::config_dir(home).join("extension"));

    // 3. Extract extension files
    if target.exists() {
        std::fs::remove_dir_all(&target)?;
    }
    extension::extract_to(&target)?;
    println!("Extension extracted to {}", target.display());

    // 4. Register native host manifest if extension ID provided
    if let Some(ref ext_id) = args.extension_id {
        validate_extension_id(ext_id)?;

        let binary_path = config::config_dir(home).join("bin").join("agentmark");
        let manifest_path = write_native_host_manifest(home, ext_id, &binary_path)?;
        println!("Native host registered at {}", manifest_path.display());
    } else {
        eprintln!("No --extension-id provided — native host manifest not written.");
        eprintln!("  After loading the extension in Chrome, find its ID at chrome://extensions");
        eprintln!("  Then run: agentmark install-extension --extension-id YOUR_EXTENSION_ID");
    }

    Ok(())
}

/// Validate that the extension ID is 32 lowercase alpha characters.
fn validate_extension_id(id: &str) -> Result<(), Box<dyn std::error::Error>> {
    if id.len() != 32 {
        return Err(format!(
            "Invalid extension ID '{}' — must be exactly 32 characters (got {})",
            id,
            id.len()
        )
        .into());
    }
    if !id.chars().all(|c| c.is_ascii_lowercase()) {
        return Err(format!(
            "Invalid extension ID '{}' — must be 32 lowercase letters (found at chrome://extensions)",
            id
        )
        .into());
    }
    Ok(())
}

/// Returns the native messaging host directory for the current OS.
///
/// Uses compile-time `cfg!(target_os)` — the binary must be compiled on
/// (or for) the same OS it will run on. The release workflow builds macOS
/// targets on macOS runners and Linux targets on Linux runners, so this
/// is correct. If cross-compilation changes, this must be revisited.
fn native_host_dir(home: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if cfg!(target_os = "macos") {
        Ok(home.join("Library/Application Support/Google/Chrome/NativeMessagingHosts"))
    } else if cfg!(target_os = "linux") {
        Ok(home.join(".config/google-chrome/NativeMessagingHosts"))
    } else {
        Err("Unsupported OS for native host registration".into())
    }
}

/// Write the Chrome native messaging host manifest and return its path.
fn write_native_host_manifest(
    home: &Path,
    extension_id: &str,
    binary_path: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let host_dir = native_host_dir(home)?;
    std::fs::create_dir_all(&host_dir)?;

    let manifest_path = host_dir.join(format!("{NATIVE_HOST_NAME}.json"));

    let manifest = serde_json::json!({
        "name": NATIVE_HOST_NAME,
        "description": "AgentMark native messaging host",
        "path": binary_path.to_str().ok_or("binary path is not valid UTF-8")?,
        "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{extension_id}/")]
    });

    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    Ok(manifest_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn validate_extension_id_valid() {
        assert!(validate_extension_id("abcdefghijklmnopabcdefghijklmnop").is_ok());
    }

    #[test]
    fn validate_extension_id_too_short() {
        let err = validate_extension_id("tooshort").unwrap_err();
        assert!(err.to_string().contains("32 characters"));
    }

    #[test]
    fn validate_extension_id_uppercase() {
        let err = validate_extension_id("ABCDEFGHIJKLMNOPABCDEFGHIJKLMNOP").unwrap_err();
        assert!(err.to_string().contains("lowercase"));
    }

    #[test]
    fn validate_extension_id_with_numbers() {
        let err = validate_extension_id("abcdefghijklmnop1234567890abcdef").unwrap_err();
        assert!(err.to_string().contains("lowercase"));
    }

    #[test]
    fn native_host_dir_returns_platform_path() {
        let home = Path::new("/Users/test");
        let dir = native_host_dir(home).unwrap();
        if cfg!(target_os = "macos") {
            assert!(dir
                .to_str()
                .unwrap()
                .contains("Library/Application Support"));
        } else if cfg!(target_os = "linux") {
            assert!(dir.to_str().unwrap().contains(".config/google-chrome"));
        }
    }

    #[test]
    fn write_native_host_manifest_creates_valid_json() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();

        let binary = Path::new("/usr/local/bin/agentmark");
        let manifest_path =
            write_native_host_manifest(home, "abcdefghijklmnopabcdefghijklmnop", binary).unwrap();

        assert!(manifest_path.exists());

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();

        assert_eq!(content["name"], "com.agentmark.native");
        assert_eq!(content["type"], "stdio");
        assert_eq!(content["path"], "/usr/local/bin/agentmark");
        assert!(content.get("args").is_none());
        assert_eq!(
            content["allowed_origins"][0],
            "chrome-extension://abcdefghijklmnopabcdefghijklmnop/"
        );
    }
}
