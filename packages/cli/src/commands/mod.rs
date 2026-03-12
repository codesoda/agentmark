pub mod init;
pub mod list;
pub mod save;
pub mod show;

use crate::cli::{Commands, OpenArgs, ReprocessArgs, SearchArgs, TagArgs};

/// Execute the parsed CLI command.
pub fn dispatch(command: Commands) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Commands::Init => init::run_init(),
        Commands::Save(args) => save::run_save(args),
        Commands::List(args) => list::run_list(args),
        Commands::Show(args) => show::run_show(args),
        Commands::Search(args) => {
            dispatch_search(args);
            Ok(())
        }
        Commands::Tag(args) => {
            dispatch_tag(args);
            Ok(())
        }
        Commands::Collections => {
            placeholder("collections");
            Ok(())
        }
        Commands::Open(args) => {
            dispatch_open(args);
            Ok(())
        }
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

fn dispatch_search(args: SearchArgs) {
    let _ = (&args.query, &args.collection, args.limit);
    placeholder("search");
}

fn dispatch_tag(args: TagArgs) {
    let _ = (&args.id, &args.tags, &args.remove);
    placeholder("tag");
}

fn dispatch_open(args: OpenArgs) {
    let _ = &args.id;
    placeholder("open");
}

fn dispatch_reprocess(args: ReprocessArgs) {
    let _ = (&args.id, args.all);
    placeholder("reprocess");
}
