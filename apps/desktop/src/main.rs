use nitium_webview::ServoBrowser;

/// Usage: `nitium [URL...]`. Each argument is opened in its own tab; without arguments
/// the previous session (or the home page) is opened.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args: Vec<String> = std::env::args().skip(1).collect();
    nitium_shell::run(
        cli_args,
        Box::new(
            |device, queue, viewport_width, viewport_height, scale_factor, dark_mode, on_event| {
                let engine = ServoBrowser::new(
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
    )
}
