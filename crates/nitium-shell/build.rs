use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-changed=build.rs");

    let mut libraries = origiri_build::library_paths();
    let ui_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("ui");
    libraries.insert("webview".to_string(), ui_dir);

    let config = slint_build::CompilerConfiguration::new().with_library_paths(libraries);
    slint_build::compile_with_config("ui/app.slint", config).unwrap();
}
