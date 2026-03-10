use clap::{Arg, ArgAction, Command};
use std::env;
use std::path::PathBuf;

pub struct InputOpts {
    pub path: PathBuf,
    pub shell: bool,
    pub srcname: String,
    pub lib_path: String,
    pub stem: String,
    pub diff_stdout: Option<PathBuf>,
    pub dump_cst_parse: bool,
    pub dump_cst_pass_string: bool,
    pub dump_cst_pass_check: bool,
    pub dump_cst_pass_noop: bool,
    pub dump_ir_lower: bool,
    pub debug_ir: bool,
    pub dry_run: bool,
    pub warn_implicit_decl: bool,
    pub warn_unresolved_tterm: bool,
    pub warn_unreachable_code: bool,
    pub warn_invalid_backslash: bool,
    pub disable_annotations: bool,
    pub bogus_annotations: bool,
}

fn library_path_get() -> String {
    env::var("SETLX_LIBRARY_PATH").unwrap_or_else(|_| {
        if cfg!(target_os = "windows") {
            let mut path = PathBuf::from(env::var("HOMEDRIVE").unwrap());
            path.push(env::var("HOMEPATH").unwrap());
            path.push("setlXlibrary");
            path.to_string_lossy().into_owned()
        } else {
            let mut path = PathBuf::from(env::var("HOME").unwrap());
            path.push("setlXlibrary");
            path.to_string_lossy().into_owned()
        }
    })
}

impl InputOpts {
    #[allow(dead_code)]
    pub fn none() -> Self {
        InputOpts {
            path: env::current_dir().unwrap(),
            srcname: String::from(""),
            lib_path: library_path_get(),
            shell: false,
            stem: String::new(),
            diff_stdout: None,
            dump_cst_parse: false,
            dump_cst_pass_string: false,
            dump_cst_pass_check: false,
            dump_cst_pass_noop: false,
            dump_ir_lower: false,
            debug_ir: false,
            dry_run: false,
            warn_implicit_decl: true,
            warn_invalid_backslash: true,
            warn_unreachable_code: true,
            warn_unresolved_tterm: true,
            disable_annotations: false,
            bogus_annotations: false,
        }
    }

    pub fn exec_opts(&self) -> Self {
        let mut out = InputOpts::none();
        out.path = self.path.clone();
        out.warn_implicit_decl = false;
        out.disable_annotations = self.bogus_annotations;
        out.bogus_annotations = self.bogus_annotations;
        out.lib_path = self.lib_path.clone();
        out.srcname = String::from("execute");
        out.debug_ir = self.debug_ir;

        out
    }
}

pub fn cli() -> InputOpts {
    let cmd = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .disable_help_subcommand(true)
        .arg(
            Arg::new("debug-ir")
                .long("debug-ir")
                .conflicts_with("diff-stdout")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .help("Run all parsing and code-gen steps but don't interpret the IR")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("diff-stdout")
                .long("diff-stdout")
                .value_name("file")
                .required(false)
                .num_args(1)
                .conflicts_with("debug-ir"),
        )
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
        .arg(
            Arg::new("library-path")
                .long("library-path")
                .value_name("path")
                .required(false)
                .num_args(1),
        )
        .arg(
            Arg::new("no-warn")
                .long("no-warn")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("warn-implicit-decl")
                .long("warn-implicit-decl")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("warn-invalid-backslash")
                .long("warn-invalid-backslash")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("warn-unreachable_code")
                .long("warn-unreachable_code")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("warn-unresolved-tterm")
                .long("warn-unresolved-tterm")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("bogus-annotations")
                .long("bogus-annotations")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("disable-annotations")
                .long("disable-annotations")
                .action(ArgAction::SetTrue),
        )
        .arg(Arg::new("file").index(1).required(false).help("input file"));

    let matches = cmd.get_matches();

    let path = matches
        .get_one::<String>("file")
        .map(|path| PathBuf::from(path.to_string().clone()));

    let stem = if let Some(path) = &path {
        path.file_stem().unwrap().to_string_lossy().into_owned()
    } else {
        "shell".to_string()
    };

    let diff_stdout = matches.get_one::<String>("diff-stdout").map(PathBuf::from);
    let lib_path = matches
        .get_one::<String>("library-path")
        .map(|i| i.to_string())
        .unwrap_or(library_path_get());
    let dump_all = matches.get_flag("dump-all");
    let dry_run = matches.get_flag("dry-run");
    let dump_cst_all = matches.get_flag("dump-cst-all") || dump_all;
    let dump_ir_all = matches.get_flag("dump-ir-all") || dump_all;

    let srcname = path
        .clone()
        .map(|i| i.to_string_lossy().into_owned())
        .unwrap_or(String::from("src/shell.stlx"));

    let mut shell = false;
    let path = path.unwrap_or_else(|| {
        shell = true;
        env::current_dir().unwrap()
    });

    InputOpts {
        path,
        shell,
        srcname,
        stem,
        lib_path,
        dry_run,
        dump_cst_parse: matches.get_flag("dump-cst-parse") || dump_cst_all,
        dump_cst_pass_string: matches.get_flag("dump-cst-pass-string") || dump_cst_all,
        dump_cst_pass_check: matches.get_flag("dump-cst-pass-check") || dump_cst_all,
        dump_cst_pass_noop: matches.get_flag("dump-cst-pass-noop") || dump_cst_all,
        dump_ir_lower: matches.get_flag("dump-ir-lower") || dump_ir_all,
        diff_stdout,
        debug_ir: matches.get_flag("debug-ir"),
        warn_implicit_decl: !matches.get_flag("no-warn") || matches.get_flag("warn-implicit-decl"),
        warn_invalid_backslash: !matches.get_flag("no-warn")
            || matches.get_flag("warn-invalid-backslash"),
        warn_unreachable_code: !matches.get_flag("no-warn")
            || matches.get_flag("warn-unreachable-code"),
        warn_unresolved_tterm: !matches.get_flag("no-warn")
            || matches.get_flag("warn-unresolved-tterm"),
        bogus_annotations: matches.get_flag("bogus-annotations") || shell,
        disable_annotations: matches.get_flag("disable-annotations"),
    }
}
