use agentmark::cli::{self, Cli};
use clap::{CommandFactory, Parser};

// ── Clap graph integrity ────────────────────────────────────────────

#[test]
fn clap_debug_assert() {
    Cli::command().debug_assert();
}

// ── Top-level help contains all subcommands ─────────────────────────

#[test]
fn top_level_help_lists_all_subcommands() {
    let help = get_long_help::<Cli>();
    for cmd in [
        "init",
        "save",
        "list",
        "show",
        "search",
        "tag",
        "collections",
        "open",
        "reprocess",
        "native-host",
    ] {
        assert!(
            help.contains(cmd),
            "top-level help missing subcommand: {cmd}"
        );
    }
}

// ── Save command ────────────────────────────────────────────────────

#[test]
fn save_parses_url_only() {
    let cli = Cli::try_parse_from(["agentmark", "save", "https://example.com"]).unwrap();
    match cli.command {
        cli::Commands::Save(args) => {
            assert_eq!(args.url, "https://example.com");
            assert!(args.tags.is_none());
            assert!(args.collection.is_none());
            assert!(args.note.is_none());
            assert!(args.action.is_none());
        }
        _ => panic!("expected Save"),
    }
}

#[test]
fn save_parses_all_flags() {
    let cli = Cli::try_parse_from([
        "agentmark",
        "save",
        "https://example.com",
        "--tags",
        "rust,cli",
        "--collection",
        "dev",
        "--note",
        "good article",
        "--action",
        "read later",
    ])
    .unwrap();
    match cli.command {
        cli::Commands::Save(args) => {
            assert_eq!(args.tags.as_deref(), Some("rust,cli"));
            assert_eq!(args.collection.as_deref(), Some("dev"));
            assert_eq!(args.note.as_deref(), Some("good article"));
            assert_eq!(args.action.as_deref(), Some("read later"));
        }
        _ => panic!("expected Save"),
    }
}

#[test]
fn save_requires_url() {
    assert!(Cli::try_parse_from(["agentmark", "save"]).is_err());
}

#[test]
fn save_help_contains_expected_flags() {
    let help = get_subcommand_help("save");
    for flag in ["--tags", "--collection", "--note", "--action"] {
        assert!(help.contains(flag), "save help missing flag: {flag}");
    }
    assert!(
        help.contains("url") || help.contains("URL"),
        "save help missing url positional"
    );
}

#[test]
fn save_parses_no_enrich() {
    let cli =
        Cli::try_parse_from(["agentmark", "save", "https://example.com", "--no-enrich"]).unwrap();
    match cli.command {
        cli::Commands::Save(args) => {
            assert!(args.no_enrich);
        }
        _ => panic!("expected Save"),
    }
}

#[test]
fn save_no_enrich_defaults_to_false() {
    let cli = Cli::try_parse_from(["agentmark", "save", "https://example.com"]).unwrap();
    match cli.command {
        cli::Commands::Save(args) => {
            assert!(!args.no_enrich);
        }
        _ => panic!("expected Save"),
    }
}

#[test]
fn save_help_contains_no_enrich() {
    let help = get_subcommand_help("save");
    assert!(
        help.contains("--no-enrich"),
        "save help missing --no-enrich flag"
    );
}

// ── List command ────────────────────────────────────────────────────

#[test]
fn list_parses_defaults() {
    let cli = Cli::try_parse_from(["agentmark", "list"]).unwrap();
    match cli.command {
        cli::Commands::List(args) => {
            assert!(args.collection.is_none());
            assert!(args.tag.is_none());
            assert_eq!(args.limit, 20);
        }
        _ => panic!("expected List"),
    }
}

#[test]
fn list_parses_filters() {
    let cli = Cli::try_parse_from([
        "agentmark",
        "list",
        "--collection",
        "dev",
        "--tag",
        "rust",
        "--limit",
        "5",
    ])
    .unwrap();
    match cli.command {
        cli::Commands::List(args) => {
            assert_eq!(args.collection.as_deref(), Some("dev"));
            assert_eq!(args.tag.as_deref(), Some("rust"));
            assert_eq!(args.limit, 5);
        }
        _ => panic!("expected List"),
    }
}

#[test]
fn list_parses_state_filter() {
    let cli = Cli::try_parse_from(["agentmark", "list", "--state", "processed"]).unwrap();
    match cli.command {
        cli::Commands::List(args) => {
            assert_eq!(args.state, Some(cli::StateFilter::Processed));
        }
        _ => panic!("expected List"),
    }
}

#[test]
fn list_parses_all_states() {
    for (input, expected) in [
        ("inbox", cli::StateFilter::Inbox),
        ("processed", cli::StateFilter::Processed),
        ("archived", cli::StateFilter::Archived),
    ] {
        let cli = Cli::try_parse_from(["agentmark", "list", "--state", input]).unwrap();
        match cli.command {
            cli::Commands::List(args) => {
                assert_eq!(args.state, Some(expected));
            }
            _ => panic!("expected List"),
        }
    }
}

#[test]
fn list_rejects_invalid_state() {
    assert!(Cli::try_parse_from(["agentmark", "list", "--state", "bogus"]).is_err());
}

#[test]
fn list_help_contains_state_flag() {
    let help = get_subcommand_help("list");
    assert!(help.contains("--state"), "list help missing --state flag");
}

// ── Show command ────────────────────────────────────────────────────

#[test]
fn show_requires_id() {
    assert!(Cli::try_parse_from(["agentmark", "show"]).is_err());
}

#[test]
fn show_parses_id_and_full() {
    let cli = Cli::try_parse_from(["agentmark", "show", "abc123", "--full"]).unwrap();
    match cli.command {
        cli::Commands::Show(args) => {
            assert_eq!(args.id, "abc123");
            assert!(args.full);
        }
        _ => panic!("expected Show"),
    }
}

// ── Search command ──────────────────────────────────────────────────

#[test]
fn search_requires_query() {
    assert!(Cli::try_parse_from(["agentmark", "search"]).is_err());
}

#[test]
fn search_parses_query_and_flags() {
    let cli = Cli::try_parse_from([
        "agentmark",
        "search",
        "rust async",
        "--collection",
        "dev",
        "--limit",
        "10",
    ])
    .unwrap();
    match cli.command {
        cli::Commands::Search(args) => {
            assert_eq!(args.query, "rust async");
            assert_eq!(args.collection.as_deref(), Some("dev"));
            assert_eq!(args.limit, 10);
        }
        _ => panic!("expected Search"),
    }
}

// ── Tag command ─────────────────────────────────────────────────────

#[test]
fn tag_requires_id() {
    assert!(Cli::try_parse_from(["agentmark", "tag"]).is_err());
}

#[test]
fn tag_add_parses() {
    let cli = Cli::try_parse_from(["agentmark", "tag", "abc123", "rust", "cli"]).unwrap();
    match cli.command {
        cli::Commands::Tag(args) => {
            assert_eq!(args.id, "abc123");
            assert_eq!(args.tags, vec!["rust", "cli"]);
            assert!(args.remove.is_empty());
        }
        _ => panic!("expected Tag"),
    }
}

#[test]
fn tag_remove_parses() {
    let cli = Cli::try_parse_from(["agentmark", "tag", "abc123", "--remove", "old-tag"]).unwrap();
    match cli.command {
        cli::Commands::Tag(args) => {
            assert_eq!(args.id, "abc123");
            assert!(args.tags.is_empty());
            assert_eq!(args.remove, vec!["old-tag"]);
        }
        _ => panic!("expected Tag"),
    }
}

#[test]
fn tag_remove_requires_tags() {
    assert!(
        Cli::try_parse_from(["agentmark", "tag", "abc123", "--remove"]).is_err(),
        "tag --remove with no tags should fail"
    );
}

#[test]
fn tag_rejects_mixed_add_and_remove() {
    assert!(
        Cli::try_parse_from(["agentmark", "tag", "abc123", "rust", "--remove", "old-tag"]).is_err(),
        "tag with both positional tags and --remove should fail"
    );
}

// ── Open command ────────────────────────────────────────────────────

#[test]
fn open_requires_id() {
    assert!(Cli::try_parse_from(["agentmark", "open"]).is_err());
}

#[test]
fn open_parses_id() {
    let cli = Cli::try_parse_from(["agentmark", "open", "abc123"]).unwrap();
    match cli.command {
        cli::Commands::Open(args) => assert_eq!(args.id, "abc123"),
        _ => panic!("expected Open"),
    }
}

// ── Reprocess command ───────────────────────────────────────────────

#[test]
fn reprocess_requires_id_or_all() {
    assert!(Cli::try_parse_from(["agentmark", "reprocess"]).is_err());
}

#[test]
fn reprocess_parses_id() {
    let cli = Cli::try_parse_from(["agentmark", "reprocess", "abc123"]).unwrap();
    match cli.command {
        cli::Commands::Reprocess(args) => {
            assert_eq!(args.id.as_deref(), Some("abc123"));
            assert!(!args.all);
        }
        _ => panic!("expected Reprocess"),
    }
}

#[test]
fn reprocess_parses_all() {
    let cli = Cli::try_parse_from(["agentmark", "reprocess", "--all"]).unwrap();
    match cli.command {
        cli::Commands::Reprocess(args) => {
            assert!(args.id.is_none());
            assert!(args.all);
        }
        _ => panic!("expected Reprocess"),
    }
}

#[test]
fn reprocess_rejects_id_and_all() {
    assert!(Cli::try_parse_from(["agentmark", "reprocess", "abc123", "--all"]).is_err());
}

// ── Collections and NativeHost (no args) ────────────────────────────

#[test]
fn collections_parses() {
    let cli = Cli::try_parse_from(["agentmark", "collections"]).unwrap();
    assert!(matches!(cli.command, cli::Commands::Collections));
}

#[test]
fn native_host_parses() {
    let cli = Cli::try_parse_from(["agentmark", "native-host"]).unwrap();
    assert!(matches!(cli.command, cli::Commands::NativeHost));
}

// ── Init (no args) ─────────────────────────────────────────────────

#[test]
fn init_parses() {
    let cli = Cli::try_parse_from(["agentmark", "init"]).unwrap();
    assert!(matches!(cli.command, cli::Commands::Init));
}

// ── No subcommand shows help ────────────────────────────────────────

#[test]
fn no_subcommand_is_error() {
    assert!(Cli::try_parse_from(["agentmark"]).is_err());
}

// ── Helpers ─────────────────────────────────────────────────────────

fn get_long_help<T: CommandFactory>() -> String {
    let mut cmd = T::command();
    let mut buf = Vec::new();
    cmd.write_long_help(&mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

fn get_subcommand_help(name: &str) -> String {
    let mut cmd = Cli::command();
    let sub = cmd
        .find_subcommand_mut(name)
        .unwrap_or_else(|| panic!("subcommand {name} not found"));
    let mut buf = Vec::new();
    sub.write_long_help(&mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}
