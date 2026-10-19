// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::cell::Cell;
use std::rc::Rc;

use euclid::Scale;
use slint::language::{KeyEvent, PointerEvent};
use smol::channel;
use winit::dpi::PhysicalSize;

use servo::{
    CompositionEvent, CompositionState, DevicePoint, DeviceVector2D, ImeEvent, InputEvent, JSValue,
    Scroll, ServoBuilder, Theme, WebViewBuilder,
};

use crate::{
    adapter::SlintServoAdapter,
    delegate::ServoDelegate,
    event::{BrowserEvent, CompositionPhase, TabId},
    events_utils::{
        convert_slint_key_event_to_servo_input_event,
        convert_slint_pointer_event_to_servo_input_event, parse_servo_url,
    },
    rendering_context::try_create_gpu_context,
    waker::Waker,
};

/// A multi-tab web browser powered by the Servo engine.
///
/// All tabs share one Servo instance and one rendering context. Exactly one tab is
/// *active* (visible and receiving input); input and navigation methods act on it.
pub struct ServoBrowser {
    adapter: Rc<SlintServoAdapter>,
    delegate: Rc<ServoDelegate>,
    scale_factor: Cell<f32>,
    dark_mode: Cell<bool>,
}

impl ServoBrowser {
    /// Creates and initializes the browser engine. No tab is open yet; call [`Self::open_tab`].
    pub fn new(
        device: slint::wgpu_30::wgpu::Device,
        queue: slint::wgpu_30::wgpu::Queue,
        viewport_width: f32,
        viewport_height: f32,
        scale_factor: f32,
        dark_mode: bool,
        on_event: impl Fn(BrowserEvent) + 'static,
    ) -> Result<Self, String> {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let (waker_sender, waker_receiver) = channel::unbounded::<()>();

        let adapter = Rc::new(SlintServoAdapter::new(
            waker_sender,
            waker_receiver,
            device,
            queue,
            on_event,
        ));

        let physical_width = (viewport_width * scale_factor).max(1.0) as u32;
        let physical_height = (viewport_height * scale_factor).max(1.0) as u32;
        let physical_size = PhysicalSize::new(physical_width, physical_height);

        let rendering_adapter =
            try_create_gpu_context(adapter.wgpu_device(), adapter.wgpu_queue(), physical_size)
                .ok_or_else(|| "Failed to create GPU context".to_string())?;

        let waker = Waker::new(adapter.waker_sender());
        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(waker))
            .build();

        adapter.set_inner(servo, Rc::new(rendering_adapter));

        Self::spin_servo_event_loop(adapter.clone());

        Ok(Self {
            delegate: Rc::new(ServoDelegate::new(adapter.clone())),
            adapter,
            scale_factor: Cell::new(scale_factor),
            dark_mode: Cell::new(dark_mode),
        })
    }

    // -- Tabs ---------------------------------------------------------------

    /// Opens a new tab loading `url`, an absolute URL (see `nitium_core::input::resolve`).
    /// The tab is activated when `activate` is true or when it is the only tab.
    pub fn open_tab(&self, url: &str, activate: bool) -> TabId {
        let rendering_context = self.adapter.rendering_adapter().get_rendering_context();
        let scale = Scale::new(self.scale_factor.get());
        let webview = {
            let servo = self.adapter.servo();
            WebViewBuilder::new(&servo, rendering_context)
                .url(parse_servo_url(url).into_url())
                .delegate(self.delegate.clone())
                .hidpi_scale_factor(scale)
                .build()
        };
        webview.notify_theme_change(self.theme());
        let tab = self.adapter.register_tab(webview);
        if activate || self.adapter.active_tab().is_none() {
            self.activate_tab(tab);
        }
        tab
    }

    /// Closes a tab. If it was the active tab no tab is active afterwards; the caller
    /// is expected to activate a successor.
    pub fn close_tab(&self, tab: TabId) {
        // Dropping the last handle destroys the page.
        drop(self.adapter.unregister_tab(tab));
    }

    /// Makes `tab` the visible tab that receives input.
    pub fn activate_tab(&self, tab: TabId) {
        let Some(next) = self.adapter.webview_of(tab) else {
            return;
        };
        if let Some(previous) = self.adapter.active_webview() {
            if previous.id() == next.id() {
                return;
            }
            previous.blur();
            previous.hide();
        }
        self.adapter.set_active_tab(Some(tab));
        next.show();
        next.focus();
    }

    pub fn active_tab(&self) -> Option<TabId> {
        self.adapter.active_tab()
    }

    // -- Navigation (active tab) --------------------------------------------

    pub fn load_url(&self, url: &str) {
        if let Some(webview) = self.adapter.active_webview() {
            webview.load(parse_servo_url(url).into_url());
        }
    }

    pub fn go_back(&self) {
        if let Some(webview) = self.adapter.active_webview() {
            webview.go_back(1);
        }
    }

    pub fn go_forward(&self) {
        if let Some(webview) = self.adapter.active_webview() {
            webview.go_forward(1);
        }
    }

    pub fn reload(&self) {
        if let Some(webview) = self.adapter.active_webview() {
            webview.reload();
        }
    }

    // -- Appearance ---------------------------------------------------------

    pub fn set_theme(&self, dark_mode: bool) {
        self.dark_mode.set(dark_mode);
        let theme = self.theme();
        for tab in self.adapter.tab_ids() {
            if let Some(webview) = self.adapter.webview_of(tab) {
                webview.notify_theme_change(theme);
            }
        }
    }

    /// Resizes the viewport. The size is in **physical** pixels.
    pub fn resize(&self, physical_width: f32, physical_height: f32, scale_factor: f32) {
        self.scale_factor.set(scale_factor);
        let physical_width = physical_width.max(1.0) as u32;
        let physical_height = physical_height.max(1.0) as u32;
        // All tabs share one rendering context, so resizing any one resizes them all.
        let mut resized = false;
        for tab in self.adapter.tab_ids() {
            if let Some(webview) = self.adapter.webview_of(tab) {
                webview.set_hidpi_scale_factor(Scale::new(scale_factor));
                if !resized {
                    webview.resize(PhysicalSize::new(physical_width, physical_height));
                    resized = true;
                }
            }
        }
    }

    // -- Input (active tab) -------------------------------------------------

    pub fn scroll(&self, initial_x: f32, initial_y: f32, delta_x: f32, delta_y: f32) {
        let Some(webview) = self.adapter.active_webview() else {
            return;
        };
        let point = DevicePoint::new(initial_x, initial_y);
        let moved_by = DeviceVector2D::new(delta_x, delta_y);
        let servo_delta = -moved_by;
        webview.notify_scroll_event(Scroll::Delta(servo_delta.into()), point.into());
    }

    pub fn handle_pointer(&self, event: &PointerEvent, x: f32, y: f32) {
        let Some(webview) = self.adapter.active_webview() else {
            return;
        };
        let point = DevicePoint::new(x, y);
        let input_event = convert_slint_pointer_event_to_servo_input_event(event, point.into());
        webview.notify_input_event(input_event);
    }

    pub fn handle_key_event(&self, event: &KeyEvent, is_pressed: bool) {
        let Some(webview) = self.adapter.active_webview() else {
            return;
        };
        let input_event = convert_slint_key_event_to_servo_input_event(event, is_pressed);
        webview.notify_input_event(input_event);
    }

    // -- Input method (active tab) ---------------------------------------------

    /// Reports a step of an input-method composition to the page's focused text field.
    pub fn ime_composition(&self, phase: CompositionPhase, data: &str) {
        let Some(webview) = self.adapter.active_webview() else {
            return;
        };
        let state = match phase {
            CompositionPhase::Start => CompositionState::Start,
            CompositionPhase::Update => CompositionState::Update,
            CompositionPhase::End => CompositionState::End,
        };
        let event = CompositionEvent {
            state,
            data: data.to_string(),
        };
        webview.notify_input_event(InputEvent::Ime(ImeEvent::Composition(event)));
    }

    /// The input method went away mid-composition; the page should abandon it.
    pub fn ime_dismiss(&self) {
        if let Some(webview) = self.adapter.active_webview() {
            webview.notify_input_event(InputEvent::Ime(ImeEvent::Dismissed));
        }
    }

    // -- Zoom and scripting (active tab) -----------------------------------------

    /// Sets the page zoom (1.0 = 100%); the engine clamps it to a sane range.
    pub fn set_zoom(&self, zoom: f32) {
        if let Some(webview) = self.adapter.active_webview() {
            webview.set_page_zoom(zoom);
        }
    }

    pub fn zoom(&self) -> f32 {
        self.adapter
            .active_webview()
            .map_or(1.0, |webview| webview.page_zoom())
    }

    /// Runs `script` in the active tab. `on_result` receives the script's value if it is
    /// a string, and `None` if it is anything else or the script failed (or there is no tab).
    pub fn evaluate_script(&self, script: &str, on_result: impl FnOnce(Option<String>) + 'static) {
        let Some(webview) = self.adapter.active_webview() else {
            on_result(None);
            return;
        };
        webview.evaluate_javascript(script, move |result| {
            on_result(match result {
                Ok(JSValue::String(text)) => Some(text),
                _ => None,
            });
        });
    }

    fn theme(&self) -> Theme {
        if self.dark_mode.get() {
            Theme::Dark
        } else {
            Theme::Light
        }
    }

    fn spin_servo_event_loop(state: Rc<SlintServoAdapter>) {
        let state_weak = Rc::downgrade(&state);

        slint::spawn_local(async move {
            loop {
                let state = match state_weak.upgrade() {
                    Some(s) => s,
                    None => break,
                };

                let _ = state.waker_receiver().recv().await;
                state.servo().spin_event_loop();
            }
        })
        .expect("Failed to spawn servo event loop task");
    }
}
