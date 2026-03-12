pub mod collections;
pub mod init;
pub mod list;
pub mod open;
pub mod save;
pub mod search;
pub mod show;
pub mod tag;

use crate::cli::{Commands, ReprocessArgs};

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
        Commands::Reprocess(args) => {
            dispatch_reprocess(args);
            Ok(())
        }
        Commands::NativeHost => {
            placeholder("native-host");
            Ok(())
        }
    }
}

fn placeholder(command: &str) {
    println!("agentmark {command}: not yet implemented");
}

fn dispatch_reprocess(args: ReprocessArgs) {
    let _ = (&args.id, args.all);
    placeholder("reprocess");
}
