use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let grammar_dir = manifest_dir.join("../../tree-sitter-snowflake");
    let src_dir = grammar_dir.join("src");

    let parser_path = src_dir.join("parser.c");
    if !parser_path.exists() {
        panic!(
            "missing generated Tree-sitter parser: {}. Run `npm run generate` in tree-sitter-snowflake.",
            parser_path.display()
        );
    }

    let mut c_config = cc::Build::new();
    c_config.std("c11").include(&src_dir).file(&parser_path);

    #[cfg(target_env = "msvc")]
    c_config.flag("-utf-8");

    let scanner_c = src_dir.join("scanner.c");
    if scanner_c.exists() {
        c_config.file(&scanner_c);
        println!("cargo:rerun-if-changed={}", scanner_c.display());
    }

    c_config.compile("tree-sitter-snowflake");

    println!("cargo:rerun-if-changed={}", parser_path.display());
    println!(
        "cargo:rerun-if-changed={}",
        grammar_dir.join("grammar.js").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        grammar_dir.join("tree-sitter.json").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        grammar_dir.join("queries/highlights.scm").display()
    );
}
