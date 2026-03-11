use clap::{Parser, Subcommand};

/// AgentMark — agent-first bookmarking for local AI workflows
#[derive(Parser)]
#[command(name = "agentmark", version, about, arg_required_else_help = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize AgentMark configuration and storage
    Init,

    /// Save a URL as a bookmark with optional metadata
    Save(SaveArgs),

    /// List saved bookmarks with optional filters
    List(ListArgs),

    /// Show details of a specific bookmark
    Show(ShowArgs),

    /// Full-text search across bookmarks
    Search(SearchArgs),

    /// Add or remove tags on a bookmark
    Tag(TagArgs),

    /// List all collections
    Collections,

    /// Open a bookmark in the default browser
    Open(OpenArgs),

    /// Reprocess bookmarks to re-extract or re-enrich content
    Reprocess(ReprocessArgs),

    /// Run the native messaging host for the browser extension
    NativeHost,
}

#[derive(clap::Args)]
pub struct SaveArgs {
    /// URL to save
    pub url: String,

    /// Comma-separated tags to apply
    #[arg(long)]
    pub tags: Option<String>,

    /// Collection to save into
    #[arg(long)]
    pub collection: Option<String>,

    /// Note to attach to the bookmark
    #[arg(long)]
    pub note: Option<String>,

    /// Action intent for the bookmark
    #[arg(long)]
    pub action: Option<String>,

    /// Skip enrichment even if enabled in config
    #[arg(long)]
    pub no_enrich: bool,
}

#[derive(clap::Args)]
pub struct ListArgs {
    /// Filter by collection
    #[arg(long)]
    pub collection: Option<String>,

    /// Filter by tag
    #[arg(long)]
    pub tag: Option<String>,

    /// Maximum number of results
    #[arg(long, default_value = "20")]
    pub limit: u32,
}

#[derive(clap::Args)]
pub struct ShowArgs {
    /// Bookmark ID to show
    pub id: String,

    /// Show full content including extracted text
    #[arg(long)]
    pub full: bool,
}

#[derive(clap::Args)]
pub struct SearchArgs {
    /// Search query
    pub query: String,

    /// Filter results by collection
    #[arg(long)]
    pub collection: Option<String>,

    /// Maximum number of results
    #[arg(long, default_value = "20")]
    pub limit: u32,
}

#[derive(clap::Args)]
pub struct TagArgs {
    /// Bookmark ID to tag
    pub id: String,

    /// Tags to add
    #[arg(required_unless_present = "remove")]
    pub tags: Vec<String>,

    /// Tags to remove instead of add
    #[arg(long, num_args = 1..)]
    pub remove: Vec<String>,
}

#[derive(clap::Args)]
pub struct OpenArgs {
    /// Bookmark ID to open
    pub id: String,
}

#[derive(clap::Args)]
pub struct ReprocessArgs {
    /// Bookmark ID to reprocess
    #[arg(conflicts_with = "all", required_unless_present = "all")]
    pub id: Option<String>,

    /// Reprocess all bookmarks
    #[arg(long)]
    pub all: bool,
}
