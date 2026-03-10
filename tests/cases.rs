use gag::BufferRedirect;
use pretty_assertions::assert_eq;
use setlx_rs::{
    cli::InputOpts, cst::cst_parse, interp::exec::exec, ir::def::IRCfg, ir::lower::CSTIRLower,
};
use std::fs;
use std::io::Read;
use walkdir::WalkDir;

#[test]
fn cases_main() {
    for i in WalkDir::new("tests/cases/")
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().and_then(|ext| ext.to_str()) == Some("stlx")
        })
    {
        let pathname = i.path().strip_prefix("tests/").unwrap().to_str().unwrap();
        let content = fs::read_to_string(i.path()).unwrap();

        eprintln!("parsing {pathname}");
        let opts = InputOpts::none();
        let cst = cst_parse(&content, &opts);
        let ir = IRCfg::from_cst(&cst, &opts);

        if let Ok(reference) = fs::read_to_string(format!("tests/{pathname}.reference")) {
            let mut buf = BufferRedirect::stdout().unwrap();
            exec(ir, &opts, content);

            let mut output = String::new();
            buf.read_to_string(&mut output).unwrap();

            assert_eq!(reference, output);
        }
    }
}
