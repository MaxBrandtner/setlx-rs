use clap::{Arg, ArgAction, Command};
use std::path::PathBuf;

pub struct InputOpts {
    pub path: PathBuf,
    pub stem: String,
    pub dump_all: bool,
    pub dump_cst_all: bool,
    pub dump_cst_dot: bool,
    pub dump_cst_struct: bool,
}

pub fn cli() -> InputOpts {
    let cmd = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .disable_help_subcommand(true)
        .arg_required_else_help(true)
        .arg(
            Arg::new("dump-all")
                .long("dump-all")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("dump-cst-all")
                .long("dump-cst-all")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("dump-cst-dot")
                .long("dump-cst-dot")
                .help("generate {filename}-cst-tree.dot")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("dump-cst-struct")
                .long("dump-cst-struct")
                .help("generate {filename}-cst-struct.dump")
                .action(ArgAction::SetTrue),
        )
        .arg(Arg::new("path").index(1).required(true).help("input file"));

    let matches = cmd.get_matches();

    let path = PathBuf::from(matches.get_one::<String>("path").unwrap().to_string());

    InputOpts {
        path: path.clone(),
        stem: path.file_stem().unwrap().to_string_lossy().into_owned(),
        dump_all: matches.get_flag("dump-all"),
        dump_cst_all: matches.get_flag("dump-cst-all"),
        dump_cst_dot: matches.get_flag("dump-cst-dot"),
        dump_cst_struct: matches.get_flag("dump-cst-struct"),
    }
}
