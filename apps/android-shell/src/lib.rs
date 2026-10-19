//! Android Slint UI shell for Nitium.

use slint::ComponentHandle;

pub use nitium_shell::{
    BrowserApp, BrowserEvent, BrowserHost, CompositionPhase, ControlId, DeviceRect, DialogKind,
    EngineFactory, MenuEntry, PageDialog, PageMenu, PageSelect, SelectChoice, TabId, WebEngine,
};

slint::include_modules!();

/// Runs the Nitium Android application with the provided [`EngineFactory`].
pub fn run(
    cli_args: Vec<String>,
    engine_factory: EngineFactory,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = AppWindow::new()?;
    let weak = app.as_weak();
    let host = BrowserHost::new(
        {
            let weak = weak.clone();
            move || weak.upgrade().map_or(1.0, |a| a.window().scale_factor())
        },
        {
            let weak = weak.clone();
            move || {
                if let Some(a) = weak.upgrade() {
                    a.window().request_redraw();
                }
            }
        },
    );
    let browser = BrowserApp::install(
        host,
        app.global::<nitium_shell::BrowserLogic>(),
        app.global::<nitium_shell::WebviewLogic>(),
        app.global::<nitium_shell::Tokens>(),
        cli_args,
        engine_factory,
    );

    {
        let weak = app.as_weak();
        let logic = app.global::<nitium_shell::BrowserLogic>();
        let w = weak.clone();
        logic.on_focus_address(move || {
            if let Some(app) = w.upgrade() {
                app.invoke_focus_address();
            }
        });
        let w = weak.clone();
        logic.on_focus_content(move || {
            if let Some(app) = w.upgrade() {
                app.invoke_focus_content();
            }
        });
        let w = weak.clone();
        logic.on_focus_dialog(move || {
            if let Some(app) = w.upgrade() {
                app.invoke_focus_dialog();
            }
        });
        let w = weak.clone();
        logic.on_focus_find(move || {
            if let Some(app) = w.upgrade() {
                app.invoke_focus_find();
            }
        });
    }

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

