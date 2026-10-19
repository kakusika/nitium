//! Events emitted by [`crate::ServoBrowser`] towards the embedding UI.

use crate::controls::{ControlId, PageDialog, PageMenu, PageSelect};

/// Stable identifier of a browser tab, unique for the lifetime of the browser.
pub type TabId = u32;

/// A rectangle in device pixels, relative to the top-left corner of the page viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// One step of an input-method composition, as the engine wants to hear it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionPhase {
    /// A composition begins (data is ignored).
    Start,
    /// The text being composed changed.
    Update,
    /// The composition ended; the data is the final text (empty if cancelled).
    End,
}

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
