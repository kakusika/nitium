mod browser;
mod shortcuts;

use slint::ComponentHandle;

slint::include_modules!();

/// Usage: `nitium [URL...]`. Each argument is opened in its own tab; without arguments
/// the previous session (or the home page) is opened.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = AppWindow::new()?;

    let cli_args: Vec<String> = std::env::args().skip(1).collect();
    let browser = browser::BrowserApp::install(&app, cli_args);

    {
        let browser = browser.clone();
        let _ = app.window().set_rendering_notifier(move |rendering, api| {
            browser.on_rendering_notifier(&rendering, api);
        });
    }

    app.run()?;
    browser.save_all();
    Ok(())
}
