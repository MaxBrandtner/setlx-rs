use gag::BufferRedirect;
use git2::Repository;
use pretty_assertions::assert_eq;
use serde_derive::Deserialize;
use setlx_rs::{
    cli::InputOpts, cst::cst_parse, interp::exec::exec, ir::def::IRCfg, ir::lower::CSTIRLower,
    setlx_parse,
};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Deserialize)]
struct SuiteData {
    name: String,
    git: String,
    dir: String,
    blacklist: Vec<String>,
    references: Option<String>,
}

fn suite_data_fetch() -> Vec<SuiteData> {
    let mut out: Vec<SuiteData> = Vec::new();

    for i in fs::read_dir("tests/suites/").unwrap() {
        let i = i.unwrap();
        let path = i.path();

        if !path.is_file() || path.extension().is_none() || path.extension().unwrap() != "json" {
            continue;
        }

        let content = fs::read_to_string(&path).unwrap();
        out.push(serde_json::from_str(&content).unwrap());
    }

    out
}

fn suites_clone(suites: &Vec<SuiteData>, test_path: &Path) {
    for i in suites {
        let out_path = test_path.join(&i.name);
        if !out_path.exists() {
            eprintln!("cloning {}", i.git);
            Repository::clone(&i.git, out_path.to_str().unwrap()).unwrap();
        }
    }
}

fn suites_test_tmpl(
    suites: &Vec<SuiteData>,
    test_path: &Path,
    step: &str,
    f: fn(&str, &Path, &str, &Option<String>),
) {
    for i in suites {
        eprintln!("PROJECT: {}", i.name);
        let mut run_path = test_path.join(&i.name);
        run_path.push(&i.dir);

        for j in WalkDir::new(&run_path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_type().is_file()
                    && e.path().extension().and_then(|ext| ext.to_str()) == Some("stlx")
            })
        {
            let abs_pathname = j.path().to_str().unwrap().to_string();
            let rel_path = j.path().strip_prefix(&run_path).unwrap();
            let pathname = rel_path.to_str().unwrap().to_string();

            if i.blacklist.contains(&pathname) {
                continue;
            }

            eprintln!("{step}: {pathname}");
            let bytes = fs::read(j.path()).unwrap();
            let contents = String::from_utf8_lossy(&bytes);
            f(&abs_pathname, &run_path, &contents, &i.references);
        }
    }
}

fn step_parse(_: &str, _: &Path, content: &str, _: &Option<String>) {
    setlx_parse::BlockParser::new().parse(content).unwrap();
}

fn step_validate(_: &str, _: &Path, content: &str, _: &Option<String>) {
    cst_parse(content, &InputOpts::none());
}

fn step_lower(_: &str, _: &Path, content: &str, _: &Option<String>) {
    let opts = InputOpts::none();
    let cst = cst_parse(content, &opts);
    IRCfg::from_cst(&cst, &opts);
}

fn step_exec(pathname: &str, prefix: &Path, content: &str, references: &Option<String>) {
    let opts = InputOpts::none();
    let cst = cst_parse(content, &opts);
    let ir = IRCfg::from_cst(&cst, &opts);

    let mut buf = BufferRedirect::stdout().unwrap();
    exec(ir, &opts, content.to_string());

    let mut output = String::new();
    buf.read_to_string(&mut output).unwrap();

    let ref_path = if let Some(r) = references {
        Path::new(r).join(Path::new(pathname).strip_prefix(prefix).unwrap())
    } else {
        PathBuf::from(format!("{pathname}.reference"))
    };

    eprintln!("reference_path: {}", ref_path.display());

    if let Ok(reference) = fs::read_to_string(ref_path) {
        assert_eq!(reference, output);
    }
}

#[test]
fn suites_main() {
    let test_path = Path::new("__TESTCACHE__");

    fs::create_dir_all(test_path).unwrap();

    let suites = suite_data_fetch();
    eprintln!("{:?}", suites);

    suites_clone(&suites, test_path);

    suites_test_tmpl(&suites, test_path, "parsing", step_parse);
    suites_test_tmpl(&suites, test_path, "validating", step_validate);
    suites_test_tmpl(&suites, test_path, "lowering", step_lower);
    suites_test_tmpl(&suites, test_path, "exec", step_exec);
}
