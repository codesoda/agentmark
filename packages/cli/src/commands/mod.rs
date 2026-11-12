pub mod add_skill;
pub mod bookmark_detail;
pub mod collections;
pub mod delete;
pub mod init;
pub mod install_extension;
pub mod list;
pub mod native_host;
pub mod open;
pub mod post_process;
pub mod reprocess;
pub mod save;
pub mod search;
pub mod show;
pub mod tag;

use crate::cli::Commands;

/// Execute the parsed CLI command.
pub fn dispatch(command: Commands) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Commands::Init(args) => init::run_init(args),
        Commands::Save(args) => save::run_save(args),
        Commands::List(args) => list::run_list(args),
        Commands::Show(args) => show::run_show(args),
        Commands::Search(args) => search::run_search(args),
        Commands::Tag(args) => tag::run_tag(args),
        Commands::Collections => collections::run_collections(),
        Commands::Open(args) => open::run_open(args),
        Commands::Reprocess(args) => reprocess::run_reprocess(args),
        Commands::Delete(args) => delete::run_delete(args),
        Commands::NativeHost(args) => native_host::run_native_host(args.origin),
        Commands::InstallExtension(args) => install_extension::run_install_extension(args),
        Commands::AddSkill => add_skill::run_add_skill(),
        Commands::PostProcess(args) => post_process::run_post_process(args),
    }
}
