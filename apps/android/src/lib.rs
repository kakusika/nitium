#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: slint::android::AndroidApp) {
    let _base = app
        .internal_data_path()
        .expect("the app has private storage");
    slint::android::init(app).expect("initialize the Android backend");

    let cli_args = Vec::new();
    if let Err(e) = nitium_shell::run(
        cli_args,
        Box::new(
            |device, queue, viewport_width, viewport_height, scale_factor, dark_mode, on_event| {
                let engine = nitium_webview::ServoBrowser::new(
                    device,
                    queue,
                    viewport_width,
                    viewport_height,
                    scale_factor,
                    dark_mode,
                    on_event,
                )?;
                Ok(Box::new(engine))
            },
        ),
    ) {
        eprintln!("nitium-android: error running app: {e}");
    }
}
