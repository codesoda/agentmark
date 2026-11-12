//! Launch steps and settings calls against a temp HOME. Nothing here touches
//! the real home folder, `~/.local/bin` or a browser's manifest directory.

use super::*;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

const DEV_ID: &str = "abcdefghijklmnopabcdefghijklmnop";

struct Fixture {
    _tmp: TempDir,
    paths: AppPaths,
}

impl Fixture {
    fn home(&self) -> &Path {
        self.paths.home()
    }
}

/// A temp HOME, a temp `local_bin`, and a fake sidecar reporting `version`.
fn fixture_with_sidecar(version: Option<&str>) -> Fixture {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let app_dir = tmp.path().join("AgentMark.app/Contents/MacOS");
    std::fs::create_dir_all(&app_dir).unwrap();
    let sidecar = app_dir.join("agentmark");
    if let Some(version) = version {
        write_script(&sidecar, version);
    }
    let paths = AppPaths {
        env: SetupEnv {
            home: home.clone(),
            search_path: OsString::new(),
        },
        local_bin: home.join(".local/bin"),
        sidecar,
        dev: None,
    };
    Fixture { _tmp: tmp, paths }
}

fn fixture() -> Fixture {
    fixture_with_sidecar(Some(env!("CARGO_PKG_VERSION")))
}

fn write_script(path: &Path, version: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("#!/bin/sh\necho \"agentmark {version}\"\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

// ── CLI step ────────────────────────────────────────────────────────

#[test]
fn sidecar_sits_beside_the_app_binary() {
    let exe = Path::new("/Applications/AgentMark.app/Contents/MacOS/agentmark-app");
    assert_eq!(
        sidecar_path(exe),
        Path::new("/Applications/AgentMark.app/Contents/MacOS/agentmark")
    );
}

#[test]
fn cli_step_fails_clearly_without_the_sidecar() {
    let f = fixture_with_sidecar(None);
    let report = run_step(&f.paths, StepId::Cli);
    assert_eq!(report.status, StepStatus::Failed);
    assert!(report.summary.contains("missing"));
    assert!(!setup::stable_cli_path(f.home()).exists());
}

// Review finding #13: `npm run tauri dev` must not replace a release CLI.

#[test]
fn a_dev_run_is_a_debug_build_outside_an_app_bundle() {
    let dev_exe = Path::new("/repo/target/debug/agentmark-app");
    let bundled =
        Path::new("/repo/target/debug/bundle/macos/AgentMark.app/Contents/MacOS/agentmark-app");
    assert_eq!(
        DevRun::detect(true, dev_exe, None),
        Some(DevRun { install_cli: false })
    );
    assert_eq!(
        DevRun::detect(true, dev_exe, Some(std::ffi::OsStr::new("1"))),
        Some(DevRun { install_cli: true })
    );
    assert_eq!(
        DevRun::detect(true, dev_exe, Some(std::ffi::OsStr::new("yes"))),
        Some(DevRun { install_cli: false })
    );
    assert_eq!(DevRun::detect(true, bundled, None), None);
    assert_eq!(DevRun::detect(false, dev_exe, None), None);
}

#[test]
fn a_dev_run_leaves_the_installed_cli_alone() {
    let mut f = fixture();
    f.paths.dev = Some(DevRun { install_cli: false });
    let stable = setup::stable_cli_path(f.home());
    write_script(&stable, env!("CARGO_PKG_VERSION"));
    let before = std::fs::read(&stable).unwrap();

    let report = run_step(&f.paths, StepId::Cli);
    assert_eq!(report.status, StepStatus::Warning, "{report:?}");
    assert!(
        report.summary.contains("left the installed agentmark"),
        "{report:?}"
    );
    let detail = report.detail.unwrap();
    assert!(detail.contains(DEV_INSTALL_CLI_ENV), "{detail}");
    assert_eq!(std::fs::read(&stable).unwrap(), before);
}

#[test]
fn a_dev_run_without_a_cli_says_how_to_install_one() {
    let mut f = fixture();
    f.paths.dev = Some(DevRun { install_cli: false });
    let report = run_step(&f.paths, StepId::Cli);
    assert_eq!(report.status, StepStatus::Warning);
    assert!(report.detail.unwrap().contains(DEV_INSTALL_CLI_ENV));
    assert!(!setup::stable_cli_path(f.home()).exists());
}

#[test]
fn a_dev_run_installs_when_asked() {
    let mut f = fixture();
    f.paths.dev = Some(DevRun { install_cli: true });
    let report = run_step(&f.paths, StepId::Cli);
    assert_eq!(report.status, StepStatus::Done, "{report:?}");
    assert!(setup::stable_cli_path(f.home()).is_file());
}

#[test]
fn a_dev_run_without_a_built_cli_does_not_say_download_again() {
    let mut f = fixture_with_sidecar(None);
    f.paths.dev = Some(DevRun { install_cli: true });
    let report = run_step(&f.paths, StepId::Cli);
    assert_eq!(report.status, StepStatus::Failed);
    let detail = report.detail.unwrap();
    assert!(!detail.contains("Download the app again"), "{detail}");
    assert!(detail.contains("cargo build -p agentmark"), "{detail}");
}

#[test]
fn adding_an_id_with_letters_after_p_is_refused() {
    // Review finding #10: Chrome IDs use the letters a to p only.
    let f = fixture();
    let err = add_extension_id(&f.paths, "abcdefghijklmnopqrstuvwxyzabcdef").unwrap_err();
    assert!(err.contains("32 letters from a to p"), "{err}");
}

#[test]
fn cli_step_installs_and_links_then_is_idempotent() {
    let f = fixture();
    let first = run_step(&f.paths, StepId::Cli);
    assert_eq!(first.status, StepStatus::Done, "{first:?}");
    assert!(first.summary.starts_with("Installed agentmark"));
    assert_eq!(
        first.detail.as_deref(),
        Some("~/.agentmark/bin/agentmark, linked from ~/.local/bin/agentmark")
    );
    let stable = setup::stable_cli_path(f.home());
    assert!(stable.is_file());
    assert_eq!(
        std::fs::read_link(f.paths.local_bin.join("agentmark")).unwrap(),
        stable
    );

    let second = run_step(&f.paths, StepId::Cli);
    assert_eq!(second.status, StepStatus::Done);
    assert!(second.summary.ends_with("is installed."), "{second:?}");
}

#[test]
fn cli_step_updates_an_older_copy() {
    let f = fixture();
    write_script(&setup::stable_cli_path(f.home()), "0.0.1");
    let report = run_step(&f.paths, StepId::Cli);
    assert_eq!(report.status, StepStatus::Done);
    assert!(
        report.summary.starts_with("Updated agentmark from 0.0.1"),
        "{report:?}"
    );
}

#[test]
fn cli_step_keeps_a_newer_copy() {
    let f = fixture();
    write_script(&setup::stable_cli_path(f.home()), "99.0.0");
    let report = run_step(&f.paths, StepId::Cli);
    assert_eq!(report.status, StepStatus::Done);
    assert!(
        report.summary.contains("newer agentmark 99.0.0"),
        "{report:?}"
    );
    let kept = std::fs::read_to_string(setup::stable_cli_path(f.home())).unwrap();
    assert!(kept.contains("99.0.0"));
}

#[test]
fn cli_step_warns_when_a_file_blocks_the_link() {
    let f = fixture();
    std::fs::create_dir_all(&f.paths.local_bin).unwrap();
    std::fs::write(f.paths.local_bin.join("agentmark"), "not ours").unwrap();
    let report = run_step(&f.paths, StepId::Cli);
    assert_eq!(report.status, StepStatus::Warning);
    assert!(report.detail.unwrap().contains("left alone"));
    assert_eq!(
        std::fs::read_to_string(f.paths.local_bin.join("agentmark")).unwrap(),
        "not ours"
    );
}

// ── Native host step ────────────────────────────────────────────────

#[test]
fn native_host_step_registers_the_store_id_without_a_pasted_id() {
    let f = fixture();
    let report = run_step(&f.paths, StepId::NativeHost);
    assert_eq!(report.status, StepStatus::Done, "{report:?}");
    assert!(
        report.summary.starts_with("Registered with Chrome"),
        "{report:?}"
    );
    let origins = setup::allowed_extension_ids(f.home());
    assert_eq!(origins, vec![setup::WEB_STORE_EXTENSION_ID]);
}

#[test]
fn native_host_step_waits_without_an_extension_id() {
    // Only a build with no store ID can get here.
    let f = fixture();
    let report = native_host_step_with_store_id(&f.paths, "");
    assert_eq!(report.status, StepStatus::Waiting, "{report:?}");
    assert_eq!(report.summary, "Waiting for the Chrome extension's ID.");
    assert!(report.detail.unwrap().starts_with("Development build"));
}

#[test]
fn adding_an_id_registers_chrome_at_the_stable_path() {
    let f = fixture();
    let report = add_extension_id(&f.paths, &format!("  {DEV_ID} ")).unwrap();
    assert_eq!(report.status, StepStatus::Done, "{report:?}");
    assert!(report.summary.contains("Chrome"));

    let chrome_dir = if cfg!(target_os = "macos") {
        "Library/Application Support/Google/Chrome/NativeMessagingHosts"
    } else {
        ".config/google-chrome/NativeMessagingHosts"
    };
    let manifest_path = f
        .home()
        .join(chrome_dir)
        .join(format!("{}.json", setup::NATIVE_HOST_NAME));
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(manifest_path).unwrap()).unwrap();
    assert_eq!(
        manifest["path"],
        setup::stable_cli_path(f.home()).display().to_string()
    );
    assert!(manifest["allowed_origins"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!(format!("chrome-extension://{DEV_ID}/"))));

    // Later launches register again without asking.
    let again = run_step(&f.paths, StepId::NativeHost);
    assert_eq!(again.status, StepStatus::Done);
}

#[test]
fn adding_an_invalid_id_explains_the_format() {
    let f = fixture();
    let err = add_extension_id(&f.paths, "not-an-id").unwrap_err();
    assert!(err.contains("32 letters"), "{err}");
}

#[test]
fn join_names_reads_naturally() {
    assert_eq!(join_names::<&str>(&[]), "no browsers");
    assert_eq!(join_names(&["Chrome"]), "Chrome");
    assert_eq!(join_names(&["Chrome", "Arc"]), "Chrome and Arc");
    assert_eq!(
        join_names(&["Chrome", "Edge", "Arc"]),
        "Chrome, Edge and Arc"
    );
}

// ── Skill step ──────────────────────────────────────────────────────

#[test]
fn skill_step_installs_into_the_temp_home() {
    let f = fixture();
    let report = run_step(&f.paths, StepId::Skill);
    assert_eq!(report.status, StepStatus::Done, "{report:?}");
    assert!(f.home().join(".agentmark/skill/SKILL.md").is_file());
    assert!(f.home().join(".agents/skills/agentmark").exists());
}

#[test]
fn skill_step_reports_failure() {
    let f = fixture();
    // A file where the config dir should be makes the write fail.
    std::fs::write(f.home().join(".agentmark"), "").unwrap();
    let report = run_step(&f.paths, StepId::Skill);
    assert_eq!(report.status, StepStatus::Failed);
    assert!(report.detail.is_some());
}

// ── Development extension and liveness ──────────────────────────────

#[test]
fn dev_extension_is_written_under_agentmark() {
    let f = fixture();
    match prepare_dev_extension(&f.paths) {
        Ok(dev) => {
            assert_eq!(dev.path_display, "~/.agentmark/extension");
            assert!(f
                .home()
                .join(".agentmark/extension/manifest.json")
                .is_file());
        }
        Err(e) => {
            assert!(!agentmark::extension::is_embedded());
            assert!(e.contains("does not include"));
        }
    }
}

#[test]
fn connection_appears_once_the_host_records_it() {
    let f = fixture();
    assert_eq!(connection(&f.paths), None);
    std::fs::create_dir_all(f.home().join(".agentmark")).unwrap();
    let record = ConnectionRecord::now(Some(DEV_ID.into()), Some("0.2.6".into()));
    assert!(setup::record_connection(f.home(), &record));
    assert_eq!(connection(&f.paths), Some(record));
}

// ── Settings ────────────────────────────────────────────────────────

#[test]
fn state_before_setup_proposes_defaults() {
    let f = fixture();
    let state = read_state(&f.paths);
    assert!(!state.initialized);
    assert_eq!(state.settings.storage_path_display, "~/Documents/AgentMark");
    assert!(
        !state.settings.enrichment_enabled,
        "no agent on the empty PATH"
    );
    assert_eq!(state.agents.len(), 3);
}

#[test]
fn invalid_settings_are_a_result_not_an_error() {
    let f = fixture();
    let changes = SettingsChanges {
        storage_path: Some("relative/path".into()),
        ..Default::default()
    };
    match save_settings(&f.paths, &changes).unwrap() {
        SaveResult::Invalid { errors } => {
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].field, setup::SettingsField::StoragePath);
        }
        other => panic!("expected invalid, got {other:?}"),
    }
    assert!(!setup::is_set_up(f.home()), "nothing written");
}

#[test]
fn saving_settings_sets_up_agentmark() {
    let f = fixture();
    let changes = SettingsChanges {
        storage_path: Some("~/Bookmarks".into()),
        default_agent: Some("codex".into()),
        enrichment_enabled: Some(false),
        ..Default::default()
    };
    let SaveResult::Saved { state } = save_settings(&f.paths, &changes).unwrap() else {
        panic!("expected saved");
    };
    assert!(state.initialized);
    assert_eq!(state.settings.storage_path_display, "~/Bookmarks");
    assert_eq!(state.settings.default_agent, "codex");
    assert_eq!(state.bookmark_count, Some(0));
    assert!(f.home().join("Bookmarks").is_dir());

    let json = serde_json::to_value(SaveResult::Saved { state }).unwrap();
    assert_eq!(json["type"], "saved");
}

#[test]
fn folder_results_match_the_native_message_shape() {
    let f = fixture();
    let chosen = folder_result(&f.paths, FolderChoice::Chosen(f.home().join("Notes")));
    assert_eq!(chosen.outcome, "chosen");
    assert_eq!(chosen.path_display.as_deref(), Some("~/Notes"));
    let cancelled = folder_result(&f.paths, FolderChoice::Cancelled);
    assert_eq!((cancelled.outcome, cancelled.path), ("cancelled", None));
    let unavailable = folder_result(
        &f.paths,
        FolderChoice::Unavailable {
            reason: "none".into(),
        },
    );
    assert_eq!(unavailable.reason.as_deref(), Some("none"));
}

#[test]
fn dialog_starts_at_the_nearest_existing_folder() {
    let f = fixture();
    let docs = f.home().join("Documents");
    std::fs::create_dir_all(&docs).unwrap();
    assert_eq!(dialog_start(&f.paths, Some("~/Documents/AgentMark")), docs);
    assert_eq!(dialog_start(&f.paths, Some("~")), f.home());
    assert_eq!(dialog_start(&f.paths, Some("relative")), f.home());
    assert_eq!(dialog_start(&f.paths, None), f.home());
}
