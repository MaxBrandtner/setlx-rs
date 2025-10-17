use std::fs;
use walkdir::WalkDir;

mod setlx_parse {
    include!(concat!(env!("OUT_DIR"), "/grammar.rs"));
}

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
        let contents = fs::read_to_string(i.path()).unwrap();

        eprintln!("parsing {pathname}");
        let _ = setlx_parse::BlockParser::new().parse(&contents).unwrap();
    }
}
