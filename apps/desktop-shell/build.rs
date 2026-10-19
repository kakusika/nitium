use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-changed=build.rs");

    let mut libraries = origiri_build::library_paths();
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let nitium_shell_ui = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crates/nitium-shell/ui");
    libraries.insert("nitium-shell".to_string(), nitium_shell_ui.clone());
    libraries.insert("webview".to_string(), nitium_shell_ui);

    let config = slint_build::CompilerConfiguration::new().with_library_paths(libraries);
    slint_build::compile_with_config("ui/app.slint", config).unwrap();
}
