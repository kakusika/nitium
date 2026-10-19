fn main() {
    let mut libraries = origiri_build::library_paths();
    let webview_dir = std::env::var("DEP_NITIUM_WEBVIEW_UI_DIR")
        .map(std::path::PathBuf::from)
        .expect("nitium-webview must be a dependency and export UI_DIR");
    libraries.insert("webview".to_string(), webview_dir);
    let config = slint_build::CompilerConfiguration::new().with_library_paths(libraries);
    slint_build::compile_with_config("ui/app.slint", config).unwrap();
}
