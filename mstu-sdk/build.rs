extern crate cbindgen;

use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let package_name = env::var("CARGO_PKG_NAME").unwrap();
    let output_file = PathBuf::from(&crate_dir).join(format!("abi/{}.h", package_name));

    // cbindgen.toml carries the C helpers that wrap the awkward calls.
    let config = cbindgen::Config::from_root_or_default(&crate_dir);

    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_documentation(true)
        .with_pragma_once(true)
        .with_parse_deps(false)
        .with_language(cbindgen::Language::C)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(output_file);
}
