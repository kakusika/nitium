//! Slint UI shell and browser application state for Nitium.
//!
//! This crate does **not** depend on Servo: the rendering engine is injected behind the
//! [`WebEngine`] trait so UI and state changes can be type-checked (`cargo check -p nitium-shell`)
//! without building the web engine.

pub mod browser;

use slint::ComponentHandle;
use slint::language::{KeyEvent, PointerEvent};

pub use nitium_core::controls::{
    CompositionPhase, ControlId, DeviceRect, DialogKind, MenuEntry, PageDialog, PageMenu,
    PageSelect, SelectChoice, TabId,
};

slint::include_modules!();

/// Something that happened inside the browser engine that the UI should reflect.
pub enum BrowserEvent {
    /// A new frame of the *active* tab is ready to be displayed.
    Frame(slint::Image),
    /// A tab was created by page content (e.g. `window.open`, `target=_blank`).
    TabOpened { tab: TabId },
    /// A tab was closed by page content (e.g. `window.close`).
    TabClosed { tab: TabId },
    /// The URL of a tab changed.
    Url { tab: TabId, url: String },
    /// The document title of a tab changed. `None` means the page has no title.
    Title { tab: TabId, title: Option<String> },
    /// A tab started or finished loading.
    Loading { tab: TabId, loading: bool },
    /// Back/forward availability of a tab changed.
    History {
        tab: TabId,
        can_go_back: bool,
        can_go_forward: bool,
    },
    /// A text field in the page gained (`Some`, with its area) or lost (`None`) focus.
    /// The UI should route text input through the system input method while it has focus.
    InputMethod {
        tab: TabId,
        field: Option<DeviceRect>,
    },
    /// The page asks the user a question and waits for the answer (script dialog,
    /// permission prompt, HTTP authentication). Answer it, or drop it to dismiss.
    Dialog { tab: TabId, dialog: PageDialog },
    /// The user right-clicked page content. Pick an entry, or drop the menu to dismiss.
    ContextMenu { tab: TabId, menu: PageMenu },
    /// A `<select>` was opened. Submit the choice, or drop it to leave the selection alone.
    SelectPopup { tab: TabId, select: PageSelect },
    /// The engine withdrew a control it asked for earlier; discard it if still shown.
    ControlHidden { tab: TabId, id: ControlId },
    /// The page wants a different mouse cursor. `cursor` is a CSS cursor name that maps
    /// onto Slint's `MouseCursor` (`"default"`, `"pointer"`, `"text"`, `"ew-resize"`, …).
    Cursor { tab: TabId, cursor: &'static str },
}

/// Abstract browser engine driven by [`browser::BrowserApp`].
pub trait WebEngine {
    fn open_tab(&self, url: &str, activate: bool) -> TabId;
    fn close_tab(&self, tab: TabId);
    fn activate_tab(&self, tab: TabId);
    fn load_url(&self, url: &str);
    fn go_back(&self);
    fn go_forward(&self);
    fn reload(&self);
    fn set_theme(&self, dark_mode: bool);
    fn resize(&self, physical_width: f32, physical_height: f32, scale_factor: f32);
    fn scroll(&self, initial_x: f32, initial_y: f32, delta_x: f32, delta_y: f32);
    fn handle_pointer(&self, event: &PointerEvent, x: f32, y: f32);
    fn handle_key_event(&self, event: &KeyEvent, is_pressed: bool);
    fn ime_composition(&self, phase: CompositionPhase, data: &str);
    fn ime_dismiss(&self);
    fn set_zoom(&self, zoom: f32);
    fn zoom(&self) -> f32;
    fn evaluate_script(&self, script: &str, on_result: Box<dyn FnOnce(Option<String>)>);
    /// Clears cookies, web storage, and network cache.
    fn clear_browsing_data(&self) {}
}

/// Factory invoked on `RenderingSetup` once the WGPU device and queue are ready.
pub type EngineFactory = Box<
    dyn Fn(
        slint::wgpu_30::wgpu::Device,
        slint::wgpu_30::wgpu::Queue,
        f32,
        f32,
        f32,
        bool,
        Box<dyn Fn(BrowserEvent)>,
    ) -> Result<Box<dyn WebEngine>, String>,
>;

/// Runs the Nitium desktop application with the provided [`EngineFactory`].
pub fn run(
    cli_args: Vec<String>,
    engine_factory: EngineFactory,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = AppWindow::new()?;
    let browser = browser::BrowserApp::install(&app, cli_args, engine_factory);

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
