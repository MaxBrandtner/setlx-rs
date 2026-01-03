use git2::Repository;
use serde_derive::Deserialize;
use setlx_rs::{
    cli::InputOpts, cst::cst_parse, ir::def::IRCfg, ir::lower::CSTIRLower, setlx_parse,
};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Deserialize)]
struct SuiteData {
    name: String,
    git: String,
    dir: String,
    blacklist: Vec<String>,
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

fn suites_test_tmpl(suites: &Vec<SuiteData>, test_path: &Path, step: &str, f: fn(&str)) {
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
            let rel_path = j.path().strip_prefix(&run_path).unwrap();
            let pathname = rel_path.to_str().unwrap().to_string();

            if i.blacklist.contains(&pathname) {
                continue;
            }

            eprintln!("{step}: {pathname}");
            let bytes = fs::read(j.path()).unwrap();
            let contents = String::from_utf8_lossy(&bytes);
            f(&contents);
        }
    }
}

fn step_parse(content: &str) {
    setlx_parse::BlockParser::new().parse(content).unwrap();
}

fn step_validate(content: &str) {
    cst_parse(content, &InputOpts::none());
}

fn step_lower(content: &str) {
    let opts = InputOpts::none();
    let cst = cst_parse(content, &opts);
    IRCfg::from_cst(&cst, &opts);
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
}
