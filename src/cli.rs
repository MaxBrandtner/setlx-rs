use clap::{Arg, ArgAction, Command};
use std::env;
use std::path::PathBuf;

pub struct InputOpts {
    pub path: PathBuf,
    pub stem: String,
    pub dump_cst_parse: bool,
    pub dump_cst_pass_string: bool,
    pub dump_cst_pass_check: bool,
    pub dump_cst_pass_noop: bool,
    pub dump_ir_lower: bool,
}

impl InputOpts {
    #[allow(dead_code)]
    pub fn none() -> Self {
        InputOpts {
            path: env::current_dir().unwrap(),
            stem: String::new(),
            dump_cst_parse: false,
            dump_cst_pass_string: false,
            dump_cst_pass_check: false,
            dump_cst_pass_noop: false,
            dump_ir_lower: false,
        }
    }
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
            Arg::new("dump-cst-parse")
                .long("dump-cst-parse")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("dump-cst-pass-string")
                .long("dump-cst-pass-01")
                .visible_alias("dump-cst-pass-string")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("dump-cst-pass-check")
                .long("dump-cst-pass-02")
                .visible_alias("dump-cst-pass-check")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("dump-cst-pass-noop")
                .long("dump-cst-pass-03")
                .visible_alias("dump-cst-pass-noop")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("dump-ir-all")
                .long("dump-ir-all")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("dump-ir-lower")
                .long("dump-ir-lower")
                .action(ArgAction::SetTrue),
        )
        .arg(Arg::new("path").index(1).required(true).help("input file"));

    let matches = cmd.get_matches();

    let path = PathBuf::from(matches.get_one::<String>("path").unwrap().to_string());

    let dump_all = matches.get_flag("dump-all");
    let dump_cst_all = matches.get_flag("dump-cst-all") || dump_all;
    let dump_ir_all = matches.get_flag("dump-ir-all") || dump_all;

    InputOpts {
        path: path.clone(),
        stem: path.file_stem().unwrap().to_string_lossy().into_owned(),
        dump_cst_parse: matches.get_flag("dump-cst-parse") || dump_cst_all,
        dump_cst_pass_string: matches.get_flag("dump-cst-pass-string") || dump_cst_all,
        dump_cst_pass_check: matches.get_flag("dump-cst-pass-check") || dump_cst_all,
        dump_cst_pass_noop: matches.get_flag("dump-cst-pass-noop") || dump_cst_all,
        dump_ir_lower: matches.get_flag("dump-ir-lower") || dump_ir_all,
    }
}
