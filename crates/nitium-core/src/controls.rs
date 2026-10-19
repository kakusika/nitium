//! Engine-agnostic types and handles for page controls, events, and geometry.
//!
//! Each page request (`PageDialog`, `PageMenu`, `PageSelect`) wraps an engine-supplied
//! responder trait object. Dropping a handle without answering is always safe: the
//! underlying engine responder's `Drop` implementation dismisses the control cleanly.

/// Stable identifier of a browser tab, unique for the lifetime of the browser.
pub type TabId = u32;

/// Identifies a control so the engine can later withdraw it (e.g. the page navigated away).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ControlId(pub u64);

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

// -- Dialogs ------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    /// `alert()`: a message and OK.
    Alert,
    /// `confirm()`: a message, OK and Cancel.
    Confirm,
    /// `prompt()`: a message, a text field, OK and Cancel.
    Prompt,
    /// A page asks to use a feature (location, camera, …): Allow and Deny.
    Permission,
    /// HTTP authentication: user name, password, Sign in and Cancel.
    Authentication,
}

/// Engine callback for answering a [`PageDialog`].
///
/// Dropping the responder without calling `accept` or `cancel` must dismiss the request.
pub trait DialogResponder {
    fn accept(self: Box<Self>, text: &str, password: &str);
    fn cancel(self: Box<Self>);
}

/// A modal question from a page.
pub struct PageDialog {
    kind: DialogKind,
    origin: String,
    message: String,
    default_text: String,
    control: Option<ControlId>,
    responder: Box<dyn DialogResponder>,
}

impl PageDialog {
    pub fn new(
        kind: DialogKind,
        origin: String,
        message: String,
        default_text: String,
        control: Option<ControlId>,
        responder: Box<dyn DialogResponder>,
    ) -> Self {
        Self {
            kind,
            origin,
            message,
            default_text,
            control,
            responder,
        }
    }

    pub fn kind(&self) -> DialogKind {
        self.kind
    }

    /// The site that is asking, for a heading the page cannot forge ("example.com says").
    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    /// Initial value of the text field of a `prompt()`.
    pub fn default_text(&self) -> &str {
        &self.default_text
    }

    /// Present for dialogs the engine can withdraw (script dialogs).
    pub fn control_id(&self) -> Option<ControlId> {
        self.control
    }

    /// OK / Allow / Sign in. `text` is the prompt's answer or the user name; `password`
    /// is only used for authentication.
    pub fn accept(self, text: &str, password: &str) {
        self.responder.accept(text, password);
    }

    /// Cancel / Deny.
    pub fn cancel(self) {
        self.responder.cancel();
    }
}

// -- Context menus ---------------------------------------------------------------

/// One row of a menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuEntry {
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
}

/// Engine callback for answering a [`PageMenu`].
pub trait MenuResponder {
    fn select(self: Box<Self>, index: usize);
    fn dismiss(self: Box<Self>);
}

/// A context menu (right click) on page content.
pub struct PageMenu {
    entries: Vec<MenuEntry>,
    control: ControlId,
    responder: Box<dyn MenuResponder>,
}

impl PageMenu {
    pub fn new(
        entries: Vec<MenuEntry>,
        control: ControlId,
        responder: Box<dyn MenuResponder>,
    ) -> Self {
        Self {
            entries,
            control,
            responder,
        }
    }

    pub fn entries(&self) -> &[MenuEntry] {
        &self.entries
    }

    pub fn control_id(&self) -> ControlId {
        self.control
    }

    /// Activates the entry at `index`; separators and disabled entries just close the menu.
    pub fn select(self, index: usize) {
        let valid = self
            .entries
            .get(index)
            .is_some_and(|entry| entry.enabled && !entry.separator);
        if valid {
            self.responder.select(index);
        } else {
            self.responder.dismiss();
        }
    }

    pub fn dismiss(self) {
        self.responder.dismiss();
    }
}

// -- <select> drop-downs ---------------------------------------------------------

/// One row of a `<select>` list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectChoice {
    /// The option's identifier; what [`PageSelect::submit`] expects back. Meaningless
    /// for group headings.
    pub id: usize,
    pub label: String,
    /// False for disabled options and group headings.
    pub selectable: bool,
    /// An `<optgroup>` label rather than an option.
    pub heading: bool,
    /// An option inside an `<optgroup>`.
    pub indented: bool,
    pub selected: bool,
}

/// Engine callback for answering a [`PageSelect`].
pub trait SelectResponder {
    fn submit(self: Box<Self>, selected: Vec<usize>);
    fn dismiss(self: Box<Self>);
}

/// The drop-down of a `<select>` element.
pub struct PageSelect {
    area: DeviceRect,
    choices: Vec<SelectChoice>,
    multiple: bool,
    control: ControlId,
    responder: Box<dyn SelectResponder>,
}

impl PageSelect {
    pub fn new(
        area: DeviceRect,
        choices: Vec<SelectChoice>,
        multiple: bool,
        control: ControlId,
        responder: Box<dyn SelectResponder>,
    ) -> Self {
        Self {
            area,
            choices,
            multiple,
            control,
            responder,
        }
    }

    /// Where the `<select>` element is, in device pixels relative to the page.
    pub fn area(&self) -> DeviceRect {
        self.area
    }

    pub fn choices(&self) -> &[SelectChoice] {
        &self.choices
    }

    pub fn multiple(&self) -> bool {
        self.multiple
    }

    pub fn control_id(&self) -> ControlId {
        self.control
    }

    /// Applies the choice. `selected` holds the identifiers of the chosen options.
    pub fn submit(self, selected: Vec<usize>) {
        self.responder.submit(selected);
    }

    /// Closes the list without changing the selection.
    pub fn dismiss(self) {
        self.responder.dismiss();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    struct MockMenuResponder(Rc<Cell<Option<usize>>>);
    impl MenuResponder for MockMenuResponder {
        fn select(self: Box<Self>, index: usize) {
            self.0.set(Some(index));
        }
        fn dismiss(self: Box<Self>) {
            self.0.set(None);
        }
    }

    #[test]
    fn disabled_or_separator_menu_entry_dismisses() {
        let chosen = Rc::new(Cell::new(Some(99)));
        let entries = vec![
            MenuEntry {
                label: "Back".into(),
                enabled: false,
                separator: false,
            },
            MenuEntry {
                label: String::new(),
                enabled: true,
                separator: true,
            },
            MenuEntry {
                label: "Reload".into(),
                enabled: true,
                separator: false,
            },
        ];

        let menu = PageMenu::new(
            entries.clone(),
            ControlId(1),
            Box::new(MockMenuResponder(chosen.clone())),
        );
        menu.select(0);
        assert_eq!(chosen.get(), None);

        let menu = PageMenu::new(
            entries.clone(),
            ControlId(1),
            Box::new(MockMenuResponder(chosen.clone())),
        );
        menu.select(1);
        assert_eq!(chosen.get(), None);

        let menu = PageMenu::new(
            entries,
            ControlId(1),
            Box::new(MockMenuResponder(chosen.clone())),
        );
        menu.select(2);
        assert_eq!(chosen.get(), Some(2));
    }
}
