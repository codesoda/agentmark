//! `agentmark install-extension` — register the native messaging host for
//! every installed browser and, when this build embeds the extension, extract
//! a development copy to load unpacked.
//!
//! The allowed extension IDs are the Web Store ID plus persisted development
//! IDs (`setup::allowed_extension_ids`); `--extension-id` adds one.

use std::io::Write;
use std::path::Path;

use tracing::{debug, instrument};

use crate::cli::InstallExtensionArgs;
use crate::config;
use crate::extension;
use crate::setup::{self, InstallError, RegistrationOutcome};

/// Printed when no ID is known. `install.sh` looks for this text to decide
/// whether to show the development steps, so keep the two in sync.
pub const NO_EXTENSION_IDS_NOTE: &str =
    "No extension IDs are known yet, so the native host was not registered.";

/// Entry point for `agentmark install-extension`.
pub fn run_install_extension(args: InstallExtensionArgs) -> Result<(), Box<dyn std::error::Error>> {
    let home = config::home_dir()?;
    let current_exe = std::env::current_exe()?;
    let binary = setup::registered_binary_path(&home, &current_exe);
    let mut out = std::io::stdout();
    let mut err = std::io::stderr();
    run_install_extension_with_io(
        &home,
        &args,
        &binary,
        extension::is_embedded(),
        setup::WEB_STORE_EXTENSION_ID,
        &mut out,
        &mut err,
    )
}

/// Testable body over the registered binary, embed flag, store ID (`""` for
/// a build with no store item) and output streams.
#[instrument(skip_all)]
fn run_install_extension_with_io(
    home: &Path,
    args: &InstallExtensionArgs,
    binary: &Path,
    embedded: bool,
    store_id: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate before touching anything.
    if let Some(id) = &args.extension_id {
        if !setup::is_valid_extension_id(id) {
            return Err(InstallError::InvalidExtensionId { id: id.clone() }.into());
        }
    }

    // Development copy of the extension, only when this build carries one.
    let target = args
        .target_dir
        .clone()
        .unwrap_or_else(|| config::config_dir(home).join("extension"));
    if embedded {
        if target.exists() {
            std::fs::remove_dir_all(&target)?;
        }
        extension::extract_to(&target)?;
        debug!(path = %target.display(), "extension extracted");
        writeln!(out, "Extension extracted to {}", target.display())?;
    } else {
        writeln!(
            out,
            "This build has no embedded extension; skipping the development copy."
        )?;
    }

    if let Some(id) = &args.extension_id {
        if setup::add_development_extension_id(home, id)? {
            writeln!(out, "Added development extension ID {id}")?;
        }
    }

    match setup::register_native_hosts_with_store_id(home, binary, store_id) {
        Ok(registrations) => {
            for reg in registrations {
                match reg.outcome {
                    RegistrationOutcome::Written => writeln!(
                        out,
                        "Native host registered for {} at {}",
                        reg.browser,
                        reg.manifest_path.display()
                    )?,
                    RegistrationOutcome::SkippedNotInstalled => {}
                    RegistrationOutcome::Failed(reason) => writeln!(
                        err,
                        "warning: could not register the native host for {}: {reason}",
                        reg.browser
                    )?,
                }
            }
            Ok(())
        }
        Err(InstallError::NoExtensionIds) => {
            writeln!(out, "{NO_EXTENSION_IDS_NOTE}")?;
            writeln!(out, "To use a development build of the extension:")?;
            writeln!(
                out,
                "  1. Open chrome://extensions and turn on Developer mode"
            )?;
            writeln!(
                out,
                "  2. Click 'Load unpacked' and select {}",
                target.display()
            )?;
            writeln!(out, "  3. Copy the extension ID shown on the card")?;
            writeln!(
                out,
                "Then run: agentmark install-extension --extension-id YOUR_EXTENSION_ID"
            )?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const DEV_A: &str = "abcdefghijklmnopabcdefghijklmnop";
    const DEV_B: &str = "ponmlkjihgfedcbaponmlkjihgfedcba";

    fn args(id: Option<&str>) -> InstallExtensionArgs {
        InstallExtensionArgs {
            extension_id: id.map(String::from),
            target_dir: None,
        }
    }

    fn run(home: &Path, args: &InstallExtensionArgs, embedded: bool) -> (String, String) {
        run_with_store_id(home, args, embedded, setup::WEB_STORE_EXTENSION_ID)
    }

    fn run_with_store_id(
        home: &Path,
        args: &InstallExtensionArgs,
        embedded: bool,
        store_id: &str,
    ) -> (String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        run_install_extension_with_io(
            home,
            args,
            Path::new("/opt/agentmark/bin/agentmark"),
            embedded,
            store_id,
            &mut out,
            &mut err,
        )
        .unwrap();
        (
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn chrome_manifest(home: &Path) -> serde_json::Value {
        let dir = if cfg!(target_os = "macos") {
            "Library/Application Support/Google/Chrome/NativeMessagingHosts"
        } else {
            ".config/google-chrome/NativeMessagingHosts"
        };
        let text =
            std::fs::read_to_string(home.join(dir).join("com.agentmark.native.json")).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn rejects_an_invalid_id_before_writing() {
        let tmp = TempDir::new().unwrap();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let e = run_install_extension_with_io(
            tmp.path(),
            &args(Some("ABC")),
            Path::new("/bin/agentmark"),
            false,
            setup::WEB_STORE_EXTENSION_ID,
            &mut out,
            &mut err,
        )
        .unwrap_err();
        assert!(e.to_string().contains("32 letters from a to p"), "{e}");
        assert!(!tmp.path().join(".agentmark").exists());
    }

    #[test]
    fn no_ids_prints_development_steps_and_succeeds() {
        // Only a build without a store ID can get here.
        let tmp = TempDir::new().unwrap();
        let (out, _) = run_with_store_id(tmp.path(), &args(None), false, "");
        assert!(out.contains(NO_EXTENSION_IDS_NOTE), "{out}");
        assert!(out.contains("Load unpacked"));
        assert!(out.contains("--extension-id YOUR_EXTENSION_ID"));
        assert!(
            out.contains("no embedded extension"),
            "tolerates a build without one"
        );
    }

    #[test]
    fn the_store_id_registers_with_no_flags() {
        let tmp = TempDir::new().unwrap();
        let (out, _) = run(tmp.path(), &args(None), false);
        assert!(!out.contains(NO_EXTENSION_IDS_NOTE), "{out}");
        assert!(out.contains("Native host registered for Chrome"), "{out}");
        assert_eq!(
            chrome_manifest(tmp.path())["allowed_origins"],
            serde_json::json!([format!(
                "chrome-extension://{}/",
                setup::WEB_STORE_EXTENSION_ID
            )])
        );
    }

    #[test]
    fn extension_id_registers_and_persists() {
        let tmp = TempDir::new().unwrap();
        let (out, _) = run(tmp.path(), &args(Some(DEV_A)), false);
        assert!(out.contains("Native host registered for Chrome"), "{out}");
        let manifest = chrome_manifest(tmp.path());
        assert_eq!(manifest["path"], "/opt/agentmark/bin/agentmark");
        assert!(manifest["allowed_origins"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(format!("chrome-extension://{DEV_A}/"))));

        // A second ID is added, not swapped in; a bare run re-registers both.
        run(tmp.path(), &args(Some(DEV_B)), false);
        run(tmp.path(), &args(None), false);
        let origins = chrome_manifest(tmp.path())["allowed_origins"].clone();
        let origins: Vec<String> = serde_json::from_value(origins).unwrap();
        assert!(origins.ends_with(&[
            format!("chrome-extension://{DEV_A}/"),
            format!("chrome-extension://{DEV_B}/"),
        ]));
    }

    #[test]
    fn embedded_build_extracts_the_development_copy() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("ext");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("stale.js"), "old").unwrap();
        let a = InstallExtensionArgs {
            extension_id: Some(DEV_A.into()),
            target_dir: Some(target.clone()),
        };
        let (out, _) = run(tmp.path(), &a, true);
        assert!(out.contains("Extension extracted to"));
        assert!(!target.join("stale.js").exists(), "old copy replaced");
    }
}
