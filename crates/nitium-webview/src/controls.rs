//! Things a page asks the *user* to decide: script dialogs, permission prompts, HTTP
//! authentication, context menus and `<select>` drop-downs.
//!
//! Each request arrives as an opaque handle that the UI keeps until the user has
//! answered. Dropping a handle without answering is always safe: the engine is told
//! the request was dismissed (alerts count as acknowledged, confirms as cancelled,
//! permissions as denied, menus and selects as closed with no change).

use servo::{
    AuthenticationRequest, ContextMenu, ContextMenuAction, ContextMenuItem, DeviceIntRect,
    EmbedderControlId, PermissionFeature, PermissionRequest, SelectElement,
    SelectElementOptionOrOptgroup, SimpleDialog,
};

use crate::event::DeviceRect;

/// Identifies a control so the engine can later withdraw it (e.g. the page navigated away).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlId(pub(crate) EmbedderControlId);

pub(crate) fn device_rect(area: DeviceIntRect) -> DeviceRect {
    DeviceRect {
        x: area.min.x as f32,
        y: area.min.y as f32,
        width: area.width() as f32,
        height: area.height() as f32,
    }
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

enum DialogInner {
    Simple(SimpleDialog),
    Permission(PermissionRequest),
    Authentication(AuthenticationRequest),
}

/// A modal question from a page.
pub struct PageDialog {
    kind: DialogKind,
    origin: String,
    message: String,
    default_text: String,
    control: Option<ControlId>,
    inner: DialogInner,
}

impl PageDialog {
    pub(crate) fn from_simple(id: EmbedderControlId, dialog: SimpleDialog, origin: String) -> Self {
        let (kind, message, default_text) = match &dialog {
            SimpleDialog::Alert(d) => (DialogKind::Alert, d.message().to_string(), String::new()),
            SimpleDialog::Confirm(d) => {
                (DialogKind::Confirm, d.message().to_string(), String::new())
            }
            SimpleDialog::Prompt(d) => (
                DialogKind::Prompt,
                d.message().to_string(),
                d.current_value().to_string(),
            ),
        };
        Self {
            kind,
            origin,
            message,
            default_text,
            control: Some(ControlId(id)),
            inner: DialogInner::Simple(dialog),
        }
    }

    pub(crate) fn from_permission(request: PermissionRequest, origin: String) -> Self {
        let message = format!("{origin} wants to {}.", permission_purpose(request.feature()));
        Self {
            kind: DialogKind::Permission,
            origin,
            message,
            default_text: String::new(),
            control: None,
            inner: DialogInner::Permission(request),
        }
    }

    pub(crate) fn from_authentication(request: AuthenticationRequest) -> Self {
        let host = request
            .url()
            .host_str()
            .unwrap_or("This site")
            .to_string();
        let message = if request.for_proxy() {
            format!("The proxy {host} requires a user name and password.")
        } else {
            format!("{host} requires a user name and password.")
        };
        Self {
            kind: DialogKind::Authentication,
            origin: host,
            message,
            default_text: String::new(),
            control: None,
            inner: DialogInner::Authentication(request),
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
        match self.inner {
            DialogInner::Simple(SimpleDialog::Alert(d)) => d.confirm(),
            DialogInner::Simple(SimpleDialog::Confirm(d)) => d.confirm(),
            DialogInner::Simple(SimpleDialog::Prompt(mut d)) => {
                d.set_current_value(text);
                d.confirm();
            }
            DialogInner::Permission(request) => request.allow(),
            DialogInner::Authentication(request) => {
                request.authenticate(text.to_string(), password.to_string())
            }
        }
    }

    /// Cancel / Deny.
    pub fn cancel(self) {
        match self.inner {
            // An alert has nothing to cancel; closing it acknowledges it.
            DialogInner::Simple(SimpleDialog::Alert(d)) => d.confirm(),
            DialogInner::Simple(SimpleDialog::Confirm(d)) => d.dismiss(),
            DialogInner::Simple(SimpleDialog::Prompt(d)) => d.dismiss(),
            DialogInner::Permission(request) => request.deny(),
            // Dropping an authentication request means "no credentials".
            DialogInner::Authentication(request) => drop(request),
        }
    }
}

fn permission_purpose(feature: PermissionFeature) -> &'static str {
    match feature {
        PermissionFeature::Geolocation => "know your location",
        PermissionFeature::Notifications => "show notifications",
        PermissionFeature::Camera => "use your camera",
        PermissionFeature::Microphone => "use your microphone",
        PermissionFeature::Midi => "use your MIDI devices",
        PermissionFeature::Bluetooth => "use Bluetooth",
        PermissionFeature::Gamepad => "use your game controller",
        PermissionFeature::PersistentStorage => "store data on this device",
        PermissionFeature::ScreenWakeLock(_) => "keep the screen on",
        _ => "use a browser feature",
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

/// A context menu (right click) on page content.
pub struct PageMenu {
    entries: Vec<MenuEntry>,
    /// The action behind each entry; `None` for separators.
    actions: Vec<Option<ContextMenuAction>>,
    control: ControlId,
    inner: ContextMenu,
}

impl PageMenu {
    pub(crate) fn new(menu: ContextMenu) -> Self {
        let mut entries = Vec::new();
        let mut actions = Vec::new();
        for item in menu.items() {
            match item {
                ContextMenuItem::Item {
                    label,
                    action,
                    enabled,
                } => {
                    entries.push(MenuEntry {
                        label: label.clone(),
                        enabled: *enabled,
                        separator: false,
                    });
                    actions.push(Some(*action));
                }
                ContextMenuItem::Separator => {
                    entries.push(MenuEntry {
                        label: String::new(),
                        enabled: false,
                        separator: true,
                    });
                    actions.push(None);
                }
            }
        }
        Self {
            entries,
            actions,
            control: ControlId(menu.id()),
            inner: menu,
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
        let enabled = self.entries.get(index).is_some_and(|entry| entry.enabled);
        let action = self.actions.get(index).copied().flatten().filter(|_| enabled);
        match action {
            Some(action) => self.inner.select(action),
            None => self.inner.dismiss(),
        }
    }

    pub fn dismiss(self) {
        self.inner.dismiss();
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

/// The drop-down of a `<select>` element.
pub struct PageSelect {
    area: DeviceRect,
    choices: Vec<SelectChoice>,
    multiple: bool,
    control: ControlId,
    inner: SelectElement,
}

impl PageSelect {
    pub(crate) fn new(select: SelectElement) -> Self {
        let selected = select.selected_options();
        let is_selected = |id: usize| selected.contains(&id);
        let mut choices = Vec::new();
        for entry in select.options() {
            match entry {
                SelectElementOptionOrOptgroup::Option(option) => choices.push(SelectChoice {
                    id: option.id,
                    label: option.label.clone(),
                    selectable: !option.is_disabled,
                    heading: false,
                    indented: false,
                    selected: is_selected(option.id),
                }),
                SelectElementOptionOrOptgroup::Optgroup { label, options } => {
                    choices.push(SelectChoice {
                        id: 0,
                        label: label.clone(),
                        selectable: false,
                        heading: true,
                        indented: false,
                        selected: false,
                    });
                    for option in options {
                        choices.push(SelectChoice {
                            id: option.id,
                            label: option.label.clone(),
                            selectable: !option.is_disabled,
                            heading: false,
                            indented: true,
                            selected: is_selected(option.id),
                        });
                    }
                }
            }
        }
        Self {
            area: device_rect(select.position()),
            multiple: select.allow_select_multiple(),
            control: ControlId(select.id()),
            choices,
            inner: select,
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
    pub fn submit(mut self, selected: Vec<usize>) {
        self.inner.select(selected);
        self.inner.submit();
    }

    /// Closes the list without changing the selection.
    pub fn dismiss(self) {
        drop(self);
    }
}
