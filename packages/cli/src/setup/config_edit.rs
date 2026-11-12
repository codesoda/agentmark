//! Comment-preserving writes of `config.toml`.
//!
//! The file is edited in place with `toml_edit` rather than re-rendered, so
//! user comments, key order, `log_level` and unknown keys survive. A value is
//! only replaced when it actually changes, which keeps its formatting and any
//! trailing comment, and makes the same settings written twice produce the
//! same file.

use std::path::{Path, PathBuf};

use toml_edit::{Array, DocumentMut, Item, Table, TableLike, Value};
use tracing::{info, warn};

use super::{write_file_atomic, SettingsError, Values};
use crate::config::{self, Config, EnrichmentConfig};

/// Write `values` into `~/.agentmark/config.toml` (the directory must exist).
pub(super) fn write_config(home: &Path, values: &Values) -> Result<(), SettingsError> {
    let path = config::config_file(home);
    let write_error = |source| SettingsError::Write {
        path: path.clone(),
        source,
    };

    let rendered = match std::fs::read_to_string(&path) {
        Ok(text) => match text.parse::<DocumentMut>() {
            Ok(mut doc) => {
                // Valid TOML that is not a valid config (a missing table, a
                // value of the wrong type): keep a copy before changing it, as
                // the setup screens promise.
                let mut kept = false;
                if toml::from_str::<Config>(&text).is_err() {
                    keep_invalid_copy(&path, &text).map_err(write_error)?;
                    kept = true;
                }
                apply_values(&mut doc, values);
                let edited = keep_line_endings(&text, doc.to_string());
                if toml::from_str::<Config>(&edited).is_ok() {
                    edited
                } else {
                    // Setting our fields does not repair it (for example a
                    // `log_level` of the wrong type, which this edit never
                    // touches): start again from the template.
                    if !kept {
                        keep_invalid_copy(&path, &text).map_err(write_error)?;
                    }
                    warn!("config.toml cannot be repaired in place; writing a new one");
                    render_new(values)
                }
            }
            Err(e) => {
                // Unreadable as TOML: keep the user's file beside the new one.
                let aside = invalid_copy_path(&path);
                warn!(
                    "config.toml is not valid TOML ({e}); moving it to {}",
                    aside.display()
                );
                std::fs::rename(&path, &aside).map_err(write_error)?;
                render_new(values)
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => render_new(values),
        Err(e) => return Err(write_error(e)),
    };

    // Never leave a config behind that the CLI cannot load.
    if let Err(e) = toml::from_str::<Config>(&rendered) {
        return Err(write_error(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("the updated config would not load: {e}"),
        )));
    }

    write_file_atomic(&path, rendered.as_bytes()).map_err(write_error)?;
    info!(path = %path.display(), "config written");
    Ok(())
}

/// Copy the current file to `config.toml.invalid-<time>` before it is changed.
fn keep_invalid_copy(path: &Path, text: &str) -> std::io::Result<()> {
    let aside = invalid_copy_path(path);
    warn!(
        "config.toml does not load as a config; keeping a copy at {}",
        aside.display()
    );
    write_file_atomic(&aside, text.as_bytes())
}

/// `toml_edit` writes `\n` line endings. A file that used `\r\n` keeps them,
/// unless converting would change a value (a multi-line string, for example).
fn keep_line_endings(original: &str, edited: String) -> String {
    if !original.contains("\r\n") {
        return edited;
    }
    let crlf = edited.replace("\r\n", "\n").replace('\n', "\r\n");
    let same = match (crlf.parse::<toml::Table>(), edited.parse::<toml::Table>()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if same {
        crlf
    } else {
        edited
    }
}

/// Settings from a file that parses as TOML but does not load as a
/// [`Config`]: each key that is valid on its own replaces the value in `base`,
/// so a hand-written file missing `[enrichment]` keeps its `storage_path`.
pub(super) fn values_from_partial_config(text: &str, mut base: Values) -> Values {
    let Ok(table) = text.parse::<toml::Table>() else {
        return base;
    };
    if let Some(path) = table.get("storage_path").and_then(toml::Value::as_str) {
        let path = PathBuf::from(path.trim());
        if path.is_absolute() {
            base.storage_path = path;
        }
    }
    if let Some(agent) = table
        .get("default_agent")
        .and_then(toml::Value::as_str)
        .and_then(|a| config::validate_agent(a).ok())
    {
        base.default_agent = agent;
    }
    if let Some(enabled) = table
        .get("enrichment")
        .and_then(|e| e.get("enabled"))
        .and_then(toml::Value::as_bool)
    {
        base.enrichment_enabled = enabled;
    }
    if let Some(prompt) = table.get("system_prompt").and_then(toml::Value::as_str) {
        base.system_prompt = Some(prompt.to_string());
    }
    if let Some(sites) = table.get("snapshot_sites").and_then(toml::Value::as_array) {
        let sites: Option<Vec<String>> = sites
            .iter()
            .map(|s| s.as_str().map(str::to_string))
            .collect();
        if let Some(sites) = sites {
            base.snapshot_sites = sites;
        }
    }
    base
}

/// `config.toml.invalid-<UTC YYYYMMDDTHHMMSSZ>`.
fn invalid_copy_path(path: &Path) -> PathBuf {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".invalid-{stamp}"));
    path.with_file_name(name)
}

/// A fresh file from the commented template.
fn render_new(values: &Values) -> String {
    config::render_config_toml(&Config {
        default_agent: values.default_agent.clone(),
        storage_path: values.storage_path.clone(),
        system_prompt: values.system_prompt.clone(),
        log_level: None,
        snapshot_sites: values.snapshot_sites.clone(),
        enrichment: EnrichmentConfig {
            enabled: values.enrichment_enabled,
        },
    })
}

fn apply_values(doc: &mut DocumentMut, values: &Values) {
    let root = doc.as_table_mut();

    let storage = values.storage_path.display().to_string();
    set_value(root, "storage_path", Value::from(storage.as_str()), |v| {
        v.as_str() == Some(storage.as_str())
    });
    set_value(
        root,
        "default_agent",
        Value::from(values.default_agent.as_str()),
        |v| v.as_str() == Some(values.default_agent.as_str()),
    );

    match &values.system_prompt {
        Some(prompt) => set_value(root, "system_prompt", Value::from(prompt.as_str()), |v| {
            v.as_str() == Some(prompt.as_str())
        }),
        None => {
            root.remove("system_prompt");
        }
    }

    if values.snapshot_sites.is_empty() {
        root.remove("snapshot_sites");
    } else {
        let sites: Array = values.snapshot_sites.iter().map(String::as_str).collect();
        set_value(root, "snapshot_sites", Value::Array(sites), |v| {
            v.as_array().is_some_and(|existing| {
                existing.len() == values.snapshot_sites.len()
                    && existing
                        .iter()
                        .zip(&values.snapshot_sites)
                        .all(|(a, b)| a.as_str() == Some(b.as_str()))
            })
        });
    }

    if !root.get("enrichment").is_some_and(Item::is_table_like) {
        root.insert("enrichment", Item::Table(Table::new()));
    }
    let enrichment = root
        .get_mut("enrichment")
        .and_then(Item::as_table_like_mut)
        .expect("enrichment was just made a table");
    let enabled = values.enrichment_enabled;
    set_value(enrichment, "enabled", Value::from(enabled), |v| {
        v.as_bool() == Some(enabled)
    });
}

/// Set `key` unless its current value already equals `new`. A replaced value
/// keeps its decor (surrounding whitespace and trailing comment).
fn set_value(table: &mut dyn TableLike, key: &str, mut new: Value, same: impl Fn(&Value) -> bool) {
    match table.get_mut(key) {
        Some(Item::Value(existing)) if same(existing) => {}
        Some(Item::Value(existing)) => {
            *new.decor_mut() = existing.decor().clone();
            *existing = new;
        }
        _ => {
            table.insert(key, Item::Value(new));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn values(storage: &Path) -> Values {
        Values {
            storage_path: storage.to_path_buf(),
            default_agent: "codex".to_string(),
            enrichment_enabled: false,
            system_prompt: None,
            snapshot_sites: Vec::new(),
        }
    }

    fn home_with_config(text: &str) -> TempDir {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(config::config_dir(tmp.path())).unwrap();
        std::fs::write(config::config_file(tmp.path()), text).unwrap();
        tmp
    }

    fn read(home: &Path) -> String {
        std::fs::read_to_string(config::config_file(home)).unwrap()
    }

    #[test]
    fn missing_file_starts_from_the_commented_template() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(config::config_dir(tmp.path())).unwrap();
        write_config(tmp.path(), &values(Path::new("/data/marks"))).unwrap();
        let text = read(tmp.path());
        assert!(text.contains("# system_prompt = \"\"\""), "{text}");
        let loaded = Config::load(tmp.path()).unwrap();
        assert_eq!(loaded.default_agent, "codex");
        assert_eq!(loaded.storage_path, PathBuf::from("/data/marks"));
        assert!(!loaded.enrichment.enabled);
    }

    #[test]
    fn keeps_comments_unknown_keys_and_log_level() {
        let tmp = home_with_config(
            "# my notes about this file\n\
             default_agent = \"claude\" # the agent I like\n\
             storage_path = \"/old\"\n\
             log_level = \"debug\"\n\
             future_option = 7\n\
             \n\
             # summaries\n\
             [enrichment]\n\
             enabled = true\n\
             \n\
             [extra]\n\
             keep = \"me\"\n",
        );
        write_config(tmp.path(), &values(Path::new("/new"))).unwrap();
        let text = read(tmp.path());
        assert!(text.contains("# my notes about this file"), "{text}");
        assert!(
            text.contains("default_agent = \"codex\" # the agent I like"),
            "{text}"
        );
        assert!(text.contains("storage_path = \"/new\""), "{text}");
        assert!(text.contains("log_level = \"debug\""), "{text}");
        assert!(text.contains("future_option = 7"), "{text}");
        assert!(
            text.contains("# summaries\n[enrichment]\nenabled = false"),
            "{text}"
        );
        assert!(text.contains("[extra]\nkeep = \"me\""), "{text}");
        assert_eq!(
            Config::load(tmp.path()).unwrap().log_level.as_deref(),
            Some("debug")
        );
    }

    #[test]
    fn unchanged_values_leave_the_file_byte_identical() {
        let original = "default_agent   =   \"codex\"\nstorage_path = '/data/marks'\n\n[enrichment]\nenabled=false\n";
        let tmp = home_with_config(original);
        write_config(tmp.path(), &values(Path::new("/data/marks"))).unwrap();
        assert_eq!(read(tmp.path()), original);
    }

    #[test]
    fn writes_and_removes_optional_keys() {
        let tmp = home_with_config(
            "default_agent = \"codex\"\nstorage_path = \"/x\"\n[enrichment]\nenabled = false\n",
        );
        let mut v = values(Path::new("/x"));
        v.system_prompt = Some("line one\nline \"two\"".to_string());
        v.snapshot_sites = vec!["example.com/app/".to_string()];
        write_config(tmp.path(), &v).unwrap();
        let loaded = Config::load(tmp.path()).unwrap();
        assert_eq!(
            loaded.system_prompt.as_deref(),
            Some("line one\nline \"two\"")
        );
        assert_eq!(loaded.snapshot_sites, vec!["example.com/app/"]);
        // New top-level keys stay above the [enrichment] table.
        let text = read(tmp.path());
        assert!(text.find("snapshot_sites").unwrap() < text.find("[enrichment]").unwrap());

        v.system_prompt = None;
        v.snapshot_sites.clear();
        write_config(tmp.path(), &v).unwrap();
        let text = read(tmp.path());
        assert!(!text.contains("system_prompt"), "{text}");
        assert!(!text.contains("snapshot_sites"), "{text}");
    }

    #[test]
    fn adds_missing_required_keys_and_enrichment_table() {
        let tmp = home_with_config("# only a comment\nlog_level = \"info\"\n");
        write_config(tmp.path(), &values(Path::new("/x"))).unwrap();
        let loaded = Config::load(tmp.path()).unwrap();
        assert_eq!(loaded.default_agent, "codex");
        assert!(!loaded.enrichment.enabled);
        assert!(read(tmp.path()).starts_with("# only a comment\n"));
    }

    #[test]
    fn replaces_wrongly_typed_values() {
        let tmp = home_with_config("default_agent = 3\nstorage_path = []\nenrichment = 1\n");
        write_config(tmp.path(), &values(Path::new("/x"))).unwrap();
        assert!(Config::load(tmp.path()).is_ok());
    }

    #[test]
    fn edits_an_inline_enrichment_table() {
        let tmp = home_with_config(
            "default_agent = \"codex\"\nstorage_path = \"/x\"\nenrichment = { enabled = true }\n",
        );
        write_config(tmp.path(), &values(Path::new("/x"))).unwrap();
        assert!(read(tmp.path()).contains("enrichment = { enabled = false }"));
    }

    #[test]
    fn invalid_toml_is_moved_aside_and_replaced() {
        let tmp = home_with_config("this is { not toml");
        write_config(tmp.path(), &values(Path::new("/x"))).unwrap();
        assert!(Config::load(tmp.path()).is_ok());
        let asides: Vec<_> = std::fs::read_dir(config::config_dir(tmp.path()))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with("config.toml.invalid-"))
            .collect();
        assert_eq!(asides.len(), 1, "{asides:?}");
        // config.toml.invalid-YYYYMMDDTHHMMSSZ
        assert_eq!(asides[0].len(), "config.toml.invalid-".len() + 16);
        assert!(asides[0].ends_with('Z'));
        let kept = std::fs::read_to_string(config::config_dir(tmp.path()).join(&asides[0]));
        assert_eq!(kept.unwrap(), "this is { not toml");
    }

    fn invalid_copies(home: &Path) -> Vec<String> {
        std::fs::read_dir(config::config_dir(home))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with("config.toml.invalid-"))
            .collect()
    }

    #[test]
    fn unrepairable_config_is_moved_aside_and_replaced() {
        let original = "default_agent = \"codex\"\nstorage_path = \"/x\"\nlog_level = 5\n";
        let tmp = home_with_config(original);
        write_config(tmp.path(), &values(Path::new("/x"))).unwrap();
        let loaded = Config::load(tmp.path()).unwrap();
        assert_eq!(loaded.log_level, None);
        assert_eq!(loaded.storage_path, PathBuf::from("/x"));
        let copies = invalid_copies(tmp.path());
        assert_eq!(copies.len(), 1, "{copies:?}");
        let kept = std::fs::read_to_string(config::config_dir(tmp.path()).join(&copies[0]));
        assert_eq!(kept.unwrap(), original);
    }

    #[test]
    fn repairable_config_is_edited_in_place_and_a_copy_kept() {
        let original = "# notes\ndefault_agent = \"codex\"\nstorage_path = \"/x\"\n";
        let tmp = home_with_config(original);
        write_config(tmp.path(), &values(Path::new("/x"))).unwrap();
        assert!(read(tmp.path()).starts_with("# notes\n"));
        assert!(Config::load(tmp.path()).is_ok());
        assert_eq!(invalid_copies(tmp.path()).len(), 1);
    }

    #[test]
    fn a_loading_config_gets_no_copy() {
        let tmp = home_with_config(
            "default_agent = \"codex\"\nstorage_path = \"/x\"\n[enrichment]\nenabled = true\n",
        );
        write_config(tmp.path(), &values(Path::new("/x"))).unwrap();
        assert!(invalid_copies(tmp.path()).is_empty());
    }

    #[test]
    fn keeps_crlf_line_endings() {
        let tmp = home_with_config(
            "# notes\r\ndefault_agent = \"claude\"\r\nstorage_path = \"/x\"\r\n\r\n[enrichment]\r\nenabled = true\r\n",
        );
        let mut v = values(Path::new("/y"));
        v.system_prompt = Some("line one\nline two".to_string());
        write_config(tmp.path(), &v).unwrap();
        let text = read(tmp.path());
        assert!(!text.replace("\r\n", "").contains('\n'), "{text:?}");
        assert!(text.contains("storage_path = \"/y\"\r\n"), "{text:?}");
        let loaded = Config::load(tmp.path()).unwrap();
        assert_eq!(loaded.system_prompt.as_deref(), Some("line one\nline two"));
    }

    #[test]
    fn partial_config_keeps_each_valid_key() {
        let base = values(Path::new("/default"));
        let seeded = values_from_partial_config(
            "storage_path = \"/mine\"\ndefault_agent = \"PI\"\nlog_level = 5\nsnapshot_sites = [\"a.com\"]\n[enrichment]\nenabled = true\n",
            base.clone(),
        );
        assert_eq!(seeded.storage_path, PathBuf::from("/mine"));
        assert_eq!(seeded.default_agent, "pi");
        assert!(seeded.enrichment_enabled);
        assert_eq!(seeded.snapshot_sites, vec!["a.com"]);

        let ignored = values_from_partial_config(
            "storage_path = \"relative\"\ndefault_agent = \"gpt\"\nsnapshot_sites = [1]\n",
            base.clone(),
        );
        assert_eq!(ignored, base);
        assert_eq!(values_from_partial_config("not { toml", base.clone()), base);
    }

    #[test]
    fn leaves_no_temp_files() {
        let tmp = home_with_config(
            "default_agent = \"claude\"\nstorage_path = \"/x\"\n[enrichment]\nenabled = true\n",
        );
        write_config(tmp.path(), &values(Path::new("/y"))).unwrap();
        let names: Vec<_> = std::fs::read_dir(config::config_dir(tmp.path()))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["config.toml".to_string()]);
    }
}
