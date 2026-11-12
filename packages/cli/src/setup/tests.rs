//! Tests for the setup core in `mod.rs`.

use std::os::unix::fs::PermissionsExt;

use super::*;
use tempfile::TempDir;

/// A temp home and an empty agent search path (no agents found).
fn env() -> (TempDir, SetupEnv) {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let bin = tmp.path().join("empty-bin");
    std::fs::create_dir_all(&bin).unwrap();
    let env = SetupEnv {
        home,
        search_path: bin.into_os_string(),
    };
    (tmp, env)
}

/// Put a fake executable for `agent` on the search path.
fn install_agent(env: &SetupEnv, agent: &str) {
    let dir = PathBuf::from(&env.search_path);
    let path = dir.join(agent);
    std::fs::write(&path, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn changes() -> SettingsChanges {
    SettingsChanges::default()
}

fn invalid(result: Result<SetupState, SettingsError>) -> Vec<FieldError> {
    match result {
        Err(SettingsError::Invalid(errors)) => errors,
        other => panic!("expected Invalid, got {other:?}"),
    }
}

fn codes(errors: &[FieldError]) -> Vec<(SettingsField, Option<usize>, FieldErrorCode)> {
    errors.iter().map(|e| (e.field, e.index, e.code)).collect()
}

// ── Paths ───────────────────────────────────────────────────────────

#[test]
fn default_storage_path_is_documents_agentmark() {
    assert_eq!(
        default_storage_path(Path::new("/Users/me")),
        PathBuf::from("/Users/me/Documents/AgentMark")
    );
}

#[test]
fn display_path_shortens_home() {
    let home = Path::new("/Users/me");
    assert_eq!(display_path(Path::new("/Users/me"), home), "~");
    assert_eq!(
        display_path(Path::new("/Users/me/Documents/AgentMark"), home),
        "~/Documents/AgentMark"
    );
    assert_eq!(
        display_path(Path::new("/Users/meg/x"), home),
        "/Users/meg/x"
    );
    assert_eq!(display_path(Path::new("/Volumes/X"), home), "/Volumes/X");
}

// ── read_state ──────────────────────────────────────────────────────

#[test]
fn state_before_setup_proposes_defaults_and_creates_nothing() {
    let (_tmp, env) = env();
    let state = read_state(&env);
    assert!(!state.initialized);
    assert_eq!(state.config_problem, None);
    assert_eq!(state.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(state.platform.os, std::env::consts::OS);
    let default = env.home.join("Documents/AgentMark");
    assert_eq!(state.default_storage_path, default.display().to_string());
    assert_eq!(state.default_storage_path_display, "~/Documents/AgentMark");
    assert_eq!(state.settings.storage_path, default.display().to_string());
    assert_eq!(state.settings.storage_path_display, "~/Documents/AgentMark");
    assert_eq!(state.settings.default_agent, "claude");
    assert!(!state.settings.enrichment_enabled, "no agent found");
    assert_eq!(state.settings.system_prompt, None);
    assert!(state.settings.snapshot_sites.is_empty());
    assert_eq!(state.agents.len(), 3);
    assert_eq!(
        state.builtin_snapshot_sites,
        crate::snapshot_rules::BUILTIN_RULES.to_vec()
    );
    assert_eq!(state.bookmark_count, None);
    assert!(!env.home.join(".agentmark").exists());
    assert!(!default.exists());
}

#[test]
fn defaults_pick_the_first_found_agent_and_turn_summaries_on() {
    let (_tmp, env) = env();
    install_agent(&env, "pi");
    install_agent(&env, "codex");
    let state = read_state(&env);
    assert_eq!(state.settings.default_agent, "codex");
    assert!(state.settings.enrichment_enabled);
    assert!(state.agents[1].found && state.agents[2].found && !state.agents[0].found);
}

#[test]
fn state_reports_an_unreadable_config() {
    let (_tmp, env) = env();
    std::fs::create_dir_all(env.home.join(".agentmark")).unwrap();
    std::fs::write(env.home.join(".agentmark/config.toml"), "bad = {").unwrap();
    let state = read_state(&env);
    assert!(!state.initialized);
    let problem = state.config_problem.expect("problem reported");
    assert!(problem.contains("invalid config"), "{problem}");
    assert_eq!(state.bookmark_count, None);
}

#[test]
fn state_after_setup_reads_settings_and_counts_bookmarks() {
    let (_tmp, env) = env();
    let mut c = changes();
    c.system_prompt = Some(Some("Be brief.".into()));
    c.snapshot_sites = Some(vec!["example.com/app/".into()]);
    apply_settings(&env, &c).unwrap();

    let conn = db::open_and_migrate(&config::index_db_path(&env.home)).unwrap();
    let bm = crate::models::Bookmark::new("https://example.com/a", "A");
    BookmarkRepository::new(&conn).insert(&bm).unwrap();

    let state = read_state(&env);
    assert!(state.initialized);
    assert_eq!(state.config_problem, None);
    assert_eq!(state.settings.system_prompt.as_deref(), Some("Be brief."));
    assert_eq!(state.settings.snapshot_sites, vec!["example.com/app/"]);
    assert_eq!(state.bookmark_count, Some(1));
    assert!(is_set_up(&env.home));
}

#[test]
fn state_serializes_every_key() {
    let (_tmp, env) = env();
    let json = serde_json::to_value(read_state(&env)).unwrap();
    for key in [
        "initialized",
        "config_problem",
        "version",
        "platform",
        "settings",
        "default_storage_path",
        "default_storage_path_display",
        "agents",
        "builtin_snapshot_sites",
        "bookmark_count",
    ] {
        assert!(json.get(key).is_some(), "missing {key}");
    }
    assert!(json["config_problem"].is_null());
    assert!(json["bookmark_count"].is_null());
    assert!(json["settings"]["system_prompt"].is_null());
    assert!(json["agents"][0]["path"].is_null());
    assert_eq!(json["agents"][0]["name"], "Claude Code");
}

// ── apply_settings ──────────────────────────────────────────────────

#[test]
fn first_save_with_no_changes_uses_defaults_and_sets_up() {
    let (_tmp, env) = env();
    let state = apply_settings(&env, &changes()).unwrap();
    assert!(state.initialized);
    assert_eq!(state.bookmark_count, Some(0));
    assert!(env.home.join("Documents/AgentMark").is_dir());
    assert!(config::index_db_path(&env.home).is_file());
    let config = Config::load(&env.home).unwrap();
    assert_eq!(config.storage_path, env.home.join("Documents/AgentMark"));
    assert_eq!(config.default_agent, "claude");
    assert!(!config.enrichment.enabled);
}

#[test]
fn full_save_writes_every_field() {
    let (tmp, env) = env();
    let folder = tmp.path().join("marks");
    let c = SettingsChanges {
        storage_path: Some(format!("  {}/  ", folder.display())),
        default_agent: Some(" PI ".into()),
        enrichment_enabled: Some(true),
        system_prompt: Some(Some("  Use bullet points.  ".into())),
        snapshot_sites: Some(vec![
            "HTTPS://Example.COM/App/".into(),
            "example.com/App/".into(),
            "x.com".into(),
            " docs.rs ".into(),
        ]),
    };
    let state = apply_settings(&env, &c).unwrap();
    let s = &state.settings;
    assert_eq!(
        s.storage_path,
        folder.display().to_string(),
        "trailing / dropped"
    );
    assert_eq!(s.default_agent, "pi");
    assert!(s.enrichment_enabled);
    assert_eq!(s.system_prompt.as_deref(), Some("Use bullet points."));
    assert_eq!(
        s.snapshot_sites,
        vec!["example.com/App/", "docs.rs"],
        "host lowercased, path kept, duplicates and built-ins dropped"
    );
}

#[test]
fn partial_update_keeps_other_values() {
    let (tmp, env) = env();
    let folder = tmp.path().join("marks");
    apply_settings(
        &env,
        &SettingsChanges {
            storage_path: Some(folder.display().to_string()),
            default_agent: Some("codex".into()),
            enrichment_enabled: Some(true),
            system_prompt: Some(Some("Keep it short.".into())),
            snapshot_sites: Some(vec!["example.com".into()]),
        },
    )
    .unwrap();

    let mut c = changes();
    c.enrichment_enabled = Some(false);
    let state = apply_settings(&env, &c).unwrap();
    assert_eq!(state.settings.storage_path, folder.display().to_string());
    assert_eq!(state.settings.default_agent, "codex");
    assert!(!state.settings.enrichment_enabled);
    assert_eq!(
        state.settings.system_prompt.as_deref(),
        Some("Keep it short.")
    );
    assert_eq!(state.settings.snapshot_sites, vec!["example.com"]);

    // null clears; an empty string is treated as null; [] clears the list.
    let mut c = changes();
    c.system_prompt = Some(None);
    c.snapshot_sites = Some(Vec::new());
    let state = apply_settings(&env, &c).unwrap();
    assert_eq!(state.settings.system_prompt, None);
    assert!(state.settings.snapshot_sites.is_empty());
    let mut c = changes();
    c.system_prompt = Some(Some("   ".into()));
    assert_eq!(
        apply_settings(&env, &c).unwrap().settings.system_prompt,
        None
    );
}

#[test]
fn same_input_twice_gives_the_same_file() {
    let (tmp, env) = env();
    let c = SettingsChanges {
        storage_path: Some(tmp.path().join("m").display().to_string()),
        default_agent: Some("claude".into()),
        enrichment_enabled: Some(true),
        system_prompt: None,
        snapshot_sites: None,
    };
    apply_settings(&env, &c).unwrap();
    let first = std::fs::read_to_string(config::config_file(&env.home)).unwrap();
    apply_settings(&env, &c).unwrap();
    let second = std::fs::read_to_string(config::config_file(&env.home)).unwrap();
    assert_eq!(first, second);
}

#[test]
fn preserves_comments_and_unknown_keys() {
    let (tmp, env) = env();
    let folder = tmp.path().join("marks");
    std::fs::create_dir_all(env.home.join(".agentmark")).unwrap();
    std::fs::write(
        config::config_file(&env.home),
        format!(
            "# hand-written notes\ndefault_agent = \"claude\"\nstorage_path = \"{}\"\nlog_level = \"debug\"\nmystery = true\n\n[enrichment]\nenabled = true # on for now\n",
            folder.display()
        ),
    )
    .unwrap();
    let mut c = changes();
    c.default_agent = Some("codex".into());
    apply_settings(&env, &c).unwrap();
    let text = std::fs::read_to_string(config::config_file(&env.home)).unwrap();
    assert!(text.starts_with("# hand-written notes\n"), "{text}");
    assert!(text.contains("mystery = true"));
    assert!(text.contains("log_level = \"debug\""));
    assert!(text.contains("enabled = true # on for now"));
    assert!(text.contains("default_agent = \"codex\""));
}

#[test]
fn invalid_existing_toml_is_replaced_using_defaults() {
    let (_tmp, env) = env();
    std::fs::create_dir_all(env.home.join(".agentmark")).unwrap();
    std::fs::write(config::config_file(&env.home), "nope = [").unwrap();
    assert!(read_state(&env).config_problem.is_some());

    let state = apply_settings(&env, &changes()).unwrap();
    assert!(state.initialized);
    assert_eq!(state.config_problem, None);
    let aside = std::fs::read_dir(env.home.join(".agentmark"))
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("config.toml.invalid-")
        });
    assert!(aside, "the broken file is kept beside the new one");
}

fn invalid_copies(env: &SetupEnv) -> Vec<String> {
    std::fs::read_dir(env.home.join(".agentmark"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("config.toml.invalid-"))
        .collect()
}

/// Valid TOML that is not a valid config (here: no `[enrichment]`) keeps
/// every key that is valid on its own, the folder above all, and a copy of
/// the original is kept as the setup screens promise.
#[test]
fn config_missing_a_table_keeps_its_folder() {
    let (tmp, env) = env();
    let store = tmp.path().join("store");
    std::fs::create_dir_all(env.home.join(".agentmark")).unwrap();
    let original = format!(
        "# mine\ndefault_agent = \"pi\"\nstorage_path = \"{}\"\n",
        store.display()
    );
    std::fs::write(config::config_file(&env.home), &original).unwrap();

    let before = read_state(&env);
    assert!(before.config_problem.is_some());
    assert_eq!(before.settings.storage_path, store.display().to_string());
    assert_eq!(before.settings.default_agent, "pi");

    let mut c = changes();
    c.default_agent = Some("codex".into());
    let state = apply_settings(&env, &c).unwrap();
    assert!(state.initialized);
    assert_eq!(state.settings.storage_path, store.display().to_string());
    assert_eq!(state.settings.default_agent, "codex");
    let text = std::fs::read_to_string(config::config_file(&env.home)).unwrap();
    assert!(text.starts_with("# mine\n"), "edited in place: {text}");

    let copies = invalid_copies(&env);
    assert_eq!(copies.len(), 1, "{copies:?}");
    let kept = std::fs::read_to_string(env.home.join(".agentmark").join(&copies[0])).unwrap();
    assert_eq!(kept, original);
}

/// A file our fields cannot repair (`log_level` of the wrong type) is moved
/// aside and a fresh one written, the same as unparseable TOML.
#[test]
fn config_that_cannot_be_repaired_is_moved_aside() {
    let (tmp, env) = env();
    let store = tmp.path().join("store");
    std::fs::create_dir_all(env.home.join(".agentmark")).unwrap();
    let original = format!(
        "default_agent = \"codex\"\nstorage_path = \"{}\"\nlog_level = 5\n[enrichment]\nenabled = false\n",
        store.display()
    );
    std::fs::write(config::config_file(&env.home), &original).unwrap();

    let state = apply_settings(&env, &changes()).unwrap();
    assert!(state.initialized);
    assert_eq!(state.settings.storage_path, store.display().to_string());
    assert_eq!(state.settings.default_agent, "codex");
    let loaded = Config::load(&env.home).unwrap();
    assert_eq!(loaded.log_level, None);

    let copies = invalid_copies(&env);
    assert_eq!(copies.len(), 1, "{copies:?}");
    let kept = std::fs::read_to_string(env.home.join(".agentmark").join(&copies[0])).unwrap();
    assert_eq!(kept, original);
}

/// Changing another setting must not touch the storage folder: it may be on a
/// drive that is not plugged in.
#[test]
fn unchanged_folder_is_not_created_or_probed() {
    let (tmp, env) = env();
    let folder = tmp.path().join("drive");
    let mut c = changes();
    c.storage_path = Some(folder.display().to_string());
    apply_settings(&env, &c).unwrap();
    std::fs::remove_dir(&folder).unwrap(); // the drive is unplugged

    let mut c = changes();
    c.default_agent = Some("pi".into());
    let state = apply_settings(&env, &c).unwrap();
    assert_eq!(state.settings.default_agent, "pi");
    assert!(!folder.exists(), "no stray folder created");

    // Sending the same folder again (as a full form does) is not a change.
    let mut c = changes();
    c.storage_path = Some(folder.display().to_string());
    c.enrichment_enabled = Some(true);
    apply_settings(&env, &c).unwrap();
    assert!(!folder.exists());
}

#[test]
fn stored_path_is_expanded_never_tilde() {
    let (_tmp, env) = env();
    let mut c = changes();
    c.storage_path = Some("~/Bookmarks".into());
    let state = apply_settings(&env, &c).unwrap();
    assert_eq!(
        state.settings.storage_path,
        env.home.join("Bookmarks").display().to_string()
    );
    assert_eq!(state.settings.storage_path_display, "~/Bookmarks");
    let text = std::fs::read_to_string(config::config_file(&env.home)).unwrap();
    assert!(!text.contains("\"~"), "{text}");
    let mut c = changes();
    c.storage_path = Some("~".into());
    assert_eq!(
        apply_settings(&env, &c)
            .unwrap()
            .settings
            .storage_path_display,
        "~"
    );
}

// ── Validation ──────────────────────────────────────────────────────

#[test]
fn rejects_blank_and_relative_paths() {
    let (_tmp, env) = env();
    for (input, code) in [
        ("   ", FieldErrorCode::Blank),
        ("bookmarks", FieldErrorCode::NotAbsolute),
        ("./bookmarks", FieldErrorCode::NotAbsolute),
        ("~user/x", FieldErrorCode::NotAbsolute),
    ] {
        let mut c = changes();
        c.storage_path = Some(input.into());
        let errors = invalid(apply_settings(&env, &c));
        assert_eq!(
            codes(&errors),
            vec![(SettingsField::StoragePath, None, code)],
            "{input}"
        );
    }
    assert!(!env.home.join(".agentmark").exists(), "nothing written");
}

#[test]
fn rejects_an_overlong_path() {
    let (_tmp, env) = env();
    let mut c = changes();
    c.storage_path = Some(format!("/{}", "a".repeat(4100)));
    let errors = invalid(apply_settings(&env, &c));
    assert_eq!(errors[0].code, FieldErrorCode::TooLong);
}

#[test]
fn rejects_a_file_as_folder() {
    let (tmp, env) = env();
    let file = tmp.path().join("a-file");
    std::fs::write(&file, "x").unwrap();
    let mut c = changes();
    c.storage_path = Some(file.display().to_string());
    let errors = invalid(apply_settings(&env, &c));
    assert_eq!(errors[0].code, FieldErrorCode::NotADirectory);
    assert!(errors[0].message.contains("a-file"));
}

#[test]
fn reports_a_folder_that_cannot_be_created() {
    let (tmp, env) = env();
    let file = tmp.path().join("a-file");
    std::fs::write(&file, "x").unwrap();
    let mut c = changes();
    c.storage_path = Some(file.join("child").display().to_string());
    let errors = invalid(apply_settings(&env, &c));
    assert_eq!(errors[0].code, FieldErrorCode::CannotCreate);
}

#[test]
fn reports_a_folder_that_is_not_writable() {
    let (tmp, env) = env();
    let folder = tmp.path().join("locked");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o555)).unwrap();
    // Root ignores permissions; nothing to test there.
    let probe = folder.join("probe");
    if std::fs::write(&probe, "x").is_ok() {
        let _ = std::fs::remove_file(&probe);
        return;
    }
    let mut c = changes();
    c.storage_path = Some(folder.display().to_string());
    let errors = invalid(apply_settings(&env, &c));
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        codes(&errors),
        vec![(
            SettingsField::StoragePath,
            None,
            FieldErrorCode::NotWritable
        )]
    );
    assert_eq!(
        errors[0].message,
        format!("AgentMark cannot write to {}.", folder.display())
    );
    assert!(!config::config_file(&env.home).exists(), "nothing written");
}

#[test]
fn folder_checks_wait_until_other_fields_are_valid() {
    let (tmp, env) = env();
    let folder = tmp.path().join("not-yet");
    let mut c = changes();
    c.storage_path = Some(folder.display().to_string());
    c.default_agent = Some("gpt".into());
    let errors = invalid(apply_settings(&env, &c));
    assert_eq!(
        codes(&errors),
        vec![(
            SettingsField::DefaultAgent,
            None,
            FieldErrorCode::InvalidAgent
        )]
    );
    assert!(errors[0].message.contains("\"gpt\""));
    assert!(
        !folder.exists(),
        "folder not created while another field is invalid"
    );
}

#[test]
fn collects_every_error_in_field_order() {
    let (_tmp, env) = env();
    let mut sites: Vec<String> = vec!["ok.com".into(), "exa mple.com".into(), "".into()];
    sites.extend((0..100).map(|i| format!("site{i}.com")));
    sites.push(format!("long.com/{}", "p".repeat(200)));
    sites.push("bad_host.com/x".into());
    let c = SettingsChanges {
        storage_path: Some("relative".into()),
        default_agent: Some("nobody".into()),
        enrichment_enabled: None,
        system_prompt: Some(Some("x".repeat(20_001))),
        snapshot_sites: Some(sites),
    };
    let errors = invalid(apply_settings(&env, &c));
    assert_eq!(
        codes(&errors),
        vec![
            (
                SettingsField::StoragePath,
                None,
                FieldErrorCode::NotAbsolute
            ),
            (
                SettingsField::DefaultAgent,
                None,
                FieldErrorCode::InvalidAgent
            ),
            (SettingsField::SystemPrompt, None, FieldErrorCode::TooLong),
            (SettingsField::SnapshotSites, None, FieldErrorCode::TooMany),
            (
                SettingsField::SnapshotSites,
                Some(1),
                FieldErrorCode::InvalidSite
            ),
            (
                SettingsField::SnapshotSites,
                Some(2),
                FieldErrorCode::InvalidSite
            ),
            (
                SettingsField::SnapshotSites,
                Some(103),
                FieldErrorCode::TooLong
            ),
            (
                SettingsField::SnapshotSites,
                Some(104),
                FieldErrorCode::InvalidSite
            ),
        ]
    );
    assert_eq!(
        errors[4].message,
        "\"exa mple.com\" is not a host/path rule."
    );
    assert!(!env.home.join(".agentmark").exists());
}

#[test]
fn site_rules_accept_paths_and_wildcards() {
    assert_eq!(
        normalize_site("http://News.Example.org/a/*").unwrap(),
        "news.example.org/a/*"
    );
    assert_eq!(normalize_site("my-site.io").unwrap(), "my-site.io");
    assert!(normalize_site("https://").is_err());
    assert!(normalize_site("/just/a/path").is_err());
    assert!(normalize_site("host:8080/x").is_err());
    assert!(normalize_site("exämple.com").is_err());
}

#[test]
fn not_found_agents_are_allowed() {
    let (_tmp, env) = env();
    let mut c = changes();
    c.default_agent = Some("codex".into());
    let state = apply_settings(&env, &c).unwrap();
    assert_eq!(state.settings.default_agent, "codex");
    assert!(!state.agents[1].found);
}

// ── Wire shapes ─────────────────────────────────────────────────────

#[test]
fn changes_distinguish_absent_null_and_value() {
    let c: SettingsChanges = serde_json::from_str("{}").unwrap();
    assert_eq!(c, SettingsChanges::default());
    let c: SettingsChanges = serde_json::from_str(r#"{"system_prompt":null}"#).unwrap();
    assert_eq!(c.system_prompt, Some(None));
    let c: SettingsChanges = serde_json::from_str(r#"{"system_prompt":"x"}"#).unwrap();
    assert_eq!(c.system_prompt, Some(Some("x".into())));
    assert!(serde_json::from_str::<SettingsChanges>(r#"{"enrichment_enabled":"yes"}"#).is_err());
}

#[test]
fn field_errors_serialize_snake_case() {
    let e = FieldError {
        field: SettingsField::SnapshotSites,
        index: Some(2),
        code: FieldErrorCode::NotADirectory,
        message: "m".into(),
    };
    assert_eq!(
        serde_json::to_value(&e).unwrap(),
        serde_json::json!({"field":"snapshot_sites","index":2,"code":"not_a_directory","message":"m"})
    );
}

// ── choose_folder ───────────────────────────────────────────────────

struct Fixed(FolderChoice);

impl FolderDialog for Fixed {
    fn pick(&self, _start: Option<&Path>) -> FolderChoice {
        self.0.clone()
    }
}

#[test]
fn choose_folder_strips_trailing_slashes() {
    let pick = |p: &str| choose_folder(&Fixed(FolderChoice::Chosen(p.into())), None);
    assert_eq!(
        pick("/Users/me/Marks/"),
        FolderChoice::Chosen("/Users/me/Marks".into())
    );
    assert_eq!(
        pick("/Users/me/Marks"),
        FolderChoice::Chosen("/Users/me/Marks".into())
    );
    assert_eq!(pick("/"), FolderChoice::Chosen("/".into()));
    assert_eq!(
        choose_folder(&Fixed(FolderChoice::Cancelled), None),
        FolderChoice::Cancelled
    );
}

// ── write_file_atomic ───────────────────────────────────────────────

#[test]
fn atomic_write_replaces_and_cleans_up() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("file.json");
    write_file_atomic(&path, b"one").unwrap();
    write_file_atomic(&path, b"two").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
    assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 1);
}
