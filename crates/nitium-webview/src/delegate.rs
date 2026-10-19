// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use servo::{
    AuthenticationRequest, CreateNewWebViewRequest, Cursor, EmbedderControl, EmbedderControlId,
    LoadStatus, PermissionRequest, WebView, WebViewDelegate,
};

use crate::adapter::SlintServoAdapter;
use crate::controls::{
    device_rect, page_dialog_from_authentication, page_dialog_from_permission,
    page_dialog_from_simple, page_menu_from_servo, page_select_from_servo,
};
use crate::event::BrowserEvent;

/// Servo delegate shared by every tab. Translates engine callbacks into [`BrowserEvent`]s.
pub struct ServoDelegate {
    adapter: Rc<SlintServoAdapter>,
}

impl ServoDelegate {
    pub fn new(adapter: Rc<SlintServoAdapter>) -> Self {
        Self { adapter }
    }
}

/// The CSS cursor name for `cursor`, spelled as Slint's `MouseCursor` values.
/// Cursors Slint has no equivalent for fall back to the nearest one.
fn cursor_name(cursor: Cursor) -> &'static str {
    match cursor {
        Cursor::None => "none",
        Cursor::Pointer => "pointer",
        Cursor::Help => "help",
        Cursor::Progress => "progress",
        Cursor::Wait => "wait",
        Cursor::Crosshair | Cursor::Cell => "crosshair",
        Cursor::Text | Cursor::VerticalText => "text",
        Cursor::Alias => "alias",
        Cursor::Copy => "copy",
        Cursor::Move | Cursor::AllScroll => "move",
        Cursor::NoDrop => "no-drop",
        Cursor::NotAllowed => "not-allowed",
        Cursor::Grab => "grab",
        Cursor::Grabbing => "grabbing",
        Cursor::ColResize => "col-resize",
        Cursor::RowResize => "row-resize",
        Cursor::NResize => "n-resize",
        Cursor::EResize => "e-resize",
        Cursor::SResize => "s-resize",
        Cursor::WResize => "w-resize",
        Cursor::NeResize => "ne-resize",
        Cursor::NwResize => "nw-resize",
        Cursor::SeResize => "se-resize",
        Cursor::SwResize => "sw-resize",
        Cursor::EwResize => "ew-resize",
        Cursor::NsResize => "ns-resize",
        Cursor::NeswResize => "nesw-resize",
        Cursor::NwseResize => "nwse-resize",
        _ => "default",
    }
}

impl WebViewDelegate for ServoDelegate {
    fn notify_new_frame_ready(&self, webview: WebView) {
        // Hidden tabs share the rendering context; painting them would clobber the
        // frame that belongs to the active tab.
        if self.adapter.tab_id_of(&webview) != self.adapter.active_tab() {
            return;
        }
        webview.paint();
        match self.adapter.current_frame() {
            Some(frame) => self.adapter.emit(BrowserEvent::Frame(frame)),
            None => eprintln!("nitium-webview: current_frame returned None"),
        }
    }

    fn notify_url_changed(&self, webview: WebView, url: url::Url) {
        if let Some(tab) = self.adapter.tab_id_of(&webview) {
            self.adapter.emit(BrowserEvent::Url {
                tab,
                url: url.to_string(),
            });
        }
    }

    fn notify_page_title_changed(&self, webview: WebView, title: Option<String>) {
        if let Some(tab) = self.adapter.tab_id_of(&webview) {
            self.adapter.emit(BrowserEvent::Title { tab, title });
        }
    }

    fn notify_load_status_changed(&self, webview: WebView, status: LoadStatus) {
        if let Some(tab) = self.adapter.tab_id_of(&webview) {
            self.adapter.emit(BrowserEvent::Loading {
                tab,
                loading: status != LoadStatus::Complete,
            });
        }
    }

    fn notify_history_changed(&self, webview: WebView, entries: Vec<url::Url>, current: usize) {
        if let Some(tab) = self.adapter.tab_id_of(&webview) {
            self.adapter.emit(BrowserEvent::History {
                tab,
                can_go_back: current > 0,
                can_go_forward: current + 1 < entries.len(),
            });
        }
    }

    fn notify_closed(&self, webview: WebView) {
        if let Some(tab) = self.adapter.tab_id_of(&webview) {
            self.adapter.emit(BrowserEvent::TabClosed { tab });
        }
    }

    fn request_create_new(&self, parent_webview: WebView, request: CreateNewWebViewRequest) {
        let rendering_context = self.adapter.rendering_adapter().get_rendering_context();
        let webview = request
            .builder(rendering_context)
            .delegate(parent_webview.delegate())
            .hidpi_scale_factor(parent_webview.hidpi_scale_factor())
            .build();
        let tab = self.adapter.register_tab(webview);
        self.adapter.emit(BrowserEvent::TabOpened { tab });
    }

    fn notify_cursor_changed(&self, webview: WebView, cursor: Cursor) {
        if let Some(tab) = self.adapter.tab_id_of(&webview) {
            self.adapter.emit(BrowserEvent::Cursor {
                tab,
                cursor: cursor_name(cursor),
            });
        }
    }

    fn show_embedder_control(&self, webview: WebView, control: EmbedderControl) {
        let servo_id = control.id();
        let Some(tab) = self.adapter.tab_id_of(&webview) else {
            return;
        };
        match control {
            EmbedderControl::InputMethod(input_method) => {
                self.adapter.set_ime_control(Some(servo_id));
                self.adapter.emit(BrowserEvent::InputMethod {
                    tab,
                    field: Some(device_rect(input_method.position())),
                });
            }
            EmbedderControl::SimpleDialog(dialog) => {
                let id = self.adapter.register_control(servo_id);
                let dialog = page_dialog_from_simple(id, dialog, origin_of(&webview));
                self.adapter.emit(BrowserEvent::Dialog { tab, dialog });
            }
            EmbedderControl::ContextMenu(menu) => {
                let id = self.adapter.register_control(servo_id);
                self.adapter.emit(BrowserEvent::ContextMenu {
                    tab,
                    menu: page_menu_from_servo(id, menu),
                });
            }
            EmbedderControl::SelectElement(select) => {
                let id = self.adapter.register_control(servo_id);
                self.adapter.emit(BrowserEvent::SelectPopup {
                    tab,
                    select: page_select_from_servo(id, select),
                });
            }
            // Color and file pickers are not supported yet; dropping them tells the
            // page they were dismissed.
            _ => {}
        }
    }

    fn hide_embedder_control(&self, webview: WebView, control_id: EmbedderControlId) {
        let Some(tab) = self.adapter.tab_id_of(&webview) else {
            return;
        };
        if self.adapter.ime_control() == Some(control_id) {
            self.adapter.set_ime_control(None);
            self.adapter.emit(BrowserEvent::InputMethod { tab, field: None });
            return;
        }
        if let Some(id) = self.adapter.take_control(control_id) {
            self.adapter.emit(BrowserEvent::ControlHidden { tab, id });
        }
    }

    fn request_permission(&self, webview: WebView, request: PermissionRequest) {
        let Some(tab) = self.adapter.tab_id_of(&webview) else {
            return;
        };
        let dialog = page_dialog_from_permission(request, origin_of(&webview));
        self.adapter.emit(BrowserEvent::Dialog { tab, dialog });
    }

    fn request_authentication(&self, webview: WebView, request: AuthenticationRequest) {
        let Some(tab) = self.adapter.tab_id_of(&webview) else {
            return;
        };
        let dialog = page_dialog_from_authentication(request);
        self.adapter.emit(BrowserEvent::Dialog { tab, dialog });
    }
}

/// The host of the page's address, or a generic name for pages without one.
fn origin_of(webview: &WebView) -> String {
    webview
        .url()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_else(|| "This page".to_string())
}
