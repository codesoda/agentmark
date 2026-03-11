pub mod init;

use crate::cli::{
    Commands, ListArgs, OpenArgs, ReprocessArgs, SaveArgs, SearchArgs, ShowArgs, TagArgs,
};

/// Execute the parsed CLI command.
pub fn dispatch(command: Commands) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Commands::Init => init::run_init(),
        Commands::Save(args) => {
            dispatch_save(args);
            Ok(())
        }
        Commands::List(args) => {
            dispatch_list(args);
            Ok(())
        }
        Commands::Show(args) => {
            dispatch_show(args);
            Ok(())
        }
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

fn dispatch_save(args: SaveArgs) {
    let _ = (
        &args.url,
        &args.tags,
        &args.collection,
        &args.note,
        &args.action,
    );
    placeholder("save");
}

fn dispatch_list(args: ListArgs) {
    let _ = (&args.collection, &args.tag, args.limit);
    placeholder("list");
}

fn dispatch_show(args: ShowArgs) {
    let _ = (&args.id, args.full);
    placeholder("show");
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
