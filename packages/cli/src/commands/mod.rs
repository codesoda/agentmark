pub mod collections;
pub mod init;
pub mod list;
pub mod native_host;
pub mod open;
pub mod reprocess;
pub mod save;
pub mod search;
pub mod show;
pub mod tag;

use crate::cli::Commands;

/// Execute the parsed CLI command.
pub fn dispatch(command: Commands) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Commands::Init => init::run_init(),
        Commands::Save(args) => save::run_save(args),
        Commands::List(args) => list::run_list(args),
        Commands::Show(args) => show::run_show(args),
        Commands::Search(args) => search::run_search(args),
        Commands::Tag(args) => tag::run_tag(args),
        Commands::Collections => collections::run_collections(),
        Commands::Open(args) => open::run_open(args),
        Commands::Reprocess(args) => reprocess::run_reprocess(args),
        Commands::NativeHost => native_host::run_native_host(),
    }
}
