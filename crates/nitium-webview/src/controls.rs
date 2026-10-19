//! Servo-backed responder implementations for `nitium_core::controls`.

use servo::{
    AuthenticationRequest, ContextMenu, ContextMenuAction, ContextMenuItem, DeviceIntRect,
    PermissionFeature, PermissionRequest, SelectElement, SelectElementOptionOrOptgroup,
    SimpleDialog,
};

pub use nitium_core::controls::{
    ControlId, DeviceRect, DialogKind, DialogResponder, MenuEntry, MenuResponder, PageDialog,
    PageMenu, PageSelect, SelectChoice, SelectResponder,
};

pub(crate) fn device_rect(area: DeviceIntRect) -> DeviceRect {
    DeviceRect {
        x: area.min.x as f32,
        y: area.min.y as f32,
        width: area.width() as f32,
        height: area.height() as f32,
    }
}

// -- Dialogs ------------------------------------------------------------------

enum ServoDialogResponder {
    Simple(SimpleDialog),
    Permission(PermissionRequest),
    Authentication(AuthenticationRequest),
}

impl DialogResponder for ServoDialogResponder {
    fn accept(self: Box<Self>, text: &str, password: &str) {
        match *self {
            Self::Simple(SimpleDialog::Alert(d)) => d.confirm(),
            Self::Simple(SimpleDialog::Confirm(d)) => d.confirm(),
            Self::Simple(SimpleDialog::Prompt(mut d)) => {
                d.set_current_value(text);
                d.confirm();
            }
            Self::Permission(request) => request.allow(),
            Self::Authentication(request) => {
                request.authenticate(text.to_string(), password.to_string())
            }
        }
    }

    fn cancel(self: Box<Self>) {
        match *self {
            // An alert has nothing to cancel; closing it acknowledges it.
            Self::Simple(SimpleDialog::Alert(d)) => d.confirm(),
            Self::Simple(SimpleDialog::Confirm(d)) => d.dismiss(),
            Self::Simple(SimpleDialog::Prompt(d)) => d.dismiss(),
            Self::Permission(request) => request.deny(),
            // Dropping an authentication request means "no credentials".
            Self::Authentication(request) => drop(request),
        }
    }
}

pub(crate) fn page_dialog_from_simple(
    id: ControlId,
    dialog: SimpleDialog,
    origin: String,
) -> PageDialog {
    let (kind, message, default_text) = match &dialog {
        SimpleDialog::Alert(d) => (DialogKind::Alert, d.message().to_string(), String::new()),
        SimpleDialog::Confirm(d) => (DialogKind::Confirm, d.message().to_string(), String::new()),
        SimpleDialog::Prompt(d) => (
            DialogKind::Prompt,
            d.message().to_string(),
            d.current_value().to_string(),
        ),
    };
    PageDialog::new(
        kind,
        origin,
        message,
        default_text,
        Some(id),
        Box::new(ServoDialogResponder::Simple(dialog)),
    )
}

pub(crate) fn page_dialog_from_permission(request: PermissionRequest, origin: String) -> PageDialog {
    let message = format!("{origin} wants to {}.", permission_purpose(request.feature()));
    PageDialog::new(
        DialogKind::Permission,
        origin,
        message,
        String::new(),
        None,
        Box::new(ServoDialogResponder::Permission(request)),
    )
}

pub(crate) fn page_dialog_from_authentication(request: AuthenticationRequest) -> PageDialog {
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
    PageDialog::new(
        DialogKind::Authentication,
        host,
        message,
        String::new(),
        None,
        Box::new(ServoDialogResponder::Authentication(request)),
    )
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

struct ServoMenuResponder {
    actions: Vec<Option<ContextMenuAction>>,
    inner: ContextMenu,
}

impl MenuResponder for ServoMenuResponder {
    fn select(self: Box<Self>, index: usize) {
        match self.actions.get(index).copied().flatten() {
            Some(action) => self.inner.select(action),
            None => self.inner.dismiss(),
        }
    }

    fn dismiss(self: Box<Self>) {
        self.inner.dismiss();
    }
}

pub(crate) fn page_menu_from_servo(id: ControlId, menu: ContextMenu) -> PageMenu {
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
    PageMenu::new(
        entries,
        id,
        Box::new(ServoMenuResponder {
            actions,
            inner: menu,
        }),
    )
}

// -- <select> drop-downs ---------------------------------------------------------

struct ServoSelectResponder(SelectElement);

impl SelectResponder for ServoSelectResponder {
    fn submit(mut self: Box<Self>, selected: Vec<usize>) {
        self.0.select(selected);
        self.0.submit();
    }

    fn dismiss(self: Box<Self>) {
        drop(self);
    }
}

pub(crate) fn page_select_from_servo(id: ControlId, select: SelectElement) -> PageSelect {
    let selected = select.selected_options();
    let is_selected = |opt_id: usize| selected.contains(&opt_id);
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
    PageSelect::new(
        device_rect(select.position()),
        choices,
        select.allow_select_multiple(),
        id,
        Box::new(ServoSelectResponder(select)),
    )
}
