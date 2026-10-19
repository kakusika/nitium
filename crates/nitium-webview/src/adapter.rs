// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::cell::{Cell, Ref, RefCell, RefMut};
use std::rc::{Rc, Weak};

use servo::{EmbedderControlId, Servo, WebView};
use smol::channel::{Receiver, Sender};

use slint::wgpu_30::wgpu;

use crate::controls::ControlId;
use crate::event::{BrowserEvent, TabId};
use crate::rendering_context::ServoRenderingAdapter;

/// Upgrades a weak reference to `SlintServoAdapter` to a strong reference.
///
/// # Panics
///
/// Panics if the adapter has been dropped (weak reference cannot be upgraded).
pub fn upgrade_adapter(weak_ref: &Weak<SlintServoAdapter>) -> Rc<SlintServoAdapter> {
    weak_ref
        .upgrade()
        .expect("Failed to upgrade SlintServoAdapter")
}

/// A single browser tab: one Servo `WebView` plus our stable identifier.
struct Tab {
    id: TabId,
    webview: WebView,
}

#[derive(Default)]
struct Tabs {
    list: Vec<Tab>,
    active: Option<TabId>,
    next_id: TabId,
}

/// Bridge between Slint UI and Servo browser engine.
///
/// Owns everything that is shared by all tabs (the Servo instance, the rendering
/// context, the wake-up channel) and the set of open tabs. All tabs paint into the
/// same rendering context; only the active tab is shown.
///
/// # Borrowing rules
///
/// `servo()` keeps `inner` borrowed while Servo's event loop spins, and delegate
/// callbacks run inside that loop. Tab state therefore lives in its own `RefCell`,
/// and no tab borrow may be held while emitting a [`BrowserEvent`] (the UI is free
/// to call back into the browser from its event handler).
pub struct SlintServoAdapter {
    /// Channel sender to wake the event loop
    waker_sender: Sender<()>,
    /// Channel receiver for event loop wake signals
    waker_receiver: Receiver<()>,
    inner: RefCell<SlintServoAdapterInner>,
    tabs: RefCell<Tabs>,
    /// The text field the page currently wants the input method for, if any.
    ime_control: RefCell<Option<EmbedderControlId>>,
    /// Maps Servo's `EmbedderControlId` to our engine-agnostic `ControlId`.
    controls: RefCell<Vec<(EmbedderControlId, ControlId)>>,
    next_control_id: Cell<u64>,
    on_event: Box<dyn Fn(BrowserEvent)>,
}

/// Internal state for `SlintServoAdapter`.
pub struct SlintServoAdapterInner {
    servo: Option<Servo>,
    rendering_adapter: Option<Rc<Box<dyn ServoRenderingAdapter>>>,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl SlintServoAdapter {
    pub fn new(
        waker_sender: Sender<()>,
        waker_receiver: Receiver<()>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        on_event: impl Fn(BrowserEvent) + 'static,
    ) -> Self {
        Self {
            waker_sender,
            waker_receiver,
            inner: RefCell::new(SlintServoAdapterInner {
                servo: None,
                rendering_adapter: None,
                device,
                queue,
            }),
            tabs: RefCell::new(Tabs::default()),
            ime_control: RefCell::new(None),
            controls: RefCell::new(Vec::new()),
            next_control_id: Cell::new(1),
            on_event: Box::new(on_event),
        }
    }

    pub fn inner(&self) -> Ref<'_, SlintServoAdapterInner> {
        self.inner.borrow()
    }

    pub fn inner_mut(&self) -> RefMut<'_, SlintServoAdapterInner> {
        self.inner.borrow_mut()
    }

    pub fn waker_sender(&self) -> Sender<()> {
        self.waker_sender.clone()
    }

    pub fn waker_receiver(&self) -> Receiver<()> {
        self.waker_receiver.clone()
    }

    pub fn wgpu_device(&self) -> wgpu::Device {
        self.inner().device.clone()
    }

    pub fn wgpu_queue(&self) -> wgpu::Queue {
        self.inner().queue.clone()
    }

    pub fn servo(&self) -> Ref<'_, Servo> {
        Ref::map(self.inner(), |inner| {
            inner.servo.as_ref().expect("Servo not initialized yet")
        })
    }

    pub fn rendering_adapter(&self) -> Rc<Box<dyn ServoRenderingAdapter>> {
        self.inner()
            .rendering_adapter
            .clone()
            .expect("Rendering adapter not initialized yet")
    }

    pub fn set_inner(&self, servo: Servo, rendering_adapter: Rc<Box<dyn ServoRenderingAdapter>>) {
        let mut inner = self.inner_mut();
        inner.servo = Some(servo);
        inner.rendering_adapter = Some(rendering_adapter);
    }

    /// Delivers an event to the UI. Must not be called with a tab borrow held.
    pub fn emit(&self, event: BrowserEvent) {
        (self.on_event)(event);
    }

    /// Captures the current Servo framebuffer as a Slint image.
    pub fn current_frame(&self) -> Option<slint::Image> {
        let inner = self.inner();
        let rendering_adapter = inner.rendering_adapter.as_ref()?;
        Some(rendering_adapter.current_framebuffer_as_image())
    }

    // -- Input method & controls ----------------------------------------------

    pub fn set_ime_control(&self, control: Option<EmbedderControlId>) {
        *self.ime_control.borrow_mut() = control;
    }

    pub fn ime_control(&self) -> Option<EmbedderControlId> {
        *self.ime_control.borrow()
    }

    pub fn register_control(&self, servo_id: EmbedderControlId) -> ControlId {
        let raw = self.next_control_id.get();
        self.next_control_id.set(raw + 1);
        let id = ControlId(raw);
        self.controls.borrow_mut().push((servo_id, id));
        id
    }

    pub fn take_control(&self, servo_id: EmbedderControlId) -> Option<ControlId> {
        let mut controls = self.controls.borrow_mut();
        let index = controls.iter().position(|(sid, _)| *sid == servo_id)?;
        Some(controls.swap_remove(index).1)
    }

    // -- Tabs ---------------------------------------------------------------

    /// Registers a freshly built `WebView` and returns its tab id.
    pub fn register_tab(&self, webview: WebView) -> TabId {
        let mut tabs = self.tabs.borrow_mut();
        let id = tabs.next_id;
        tabs.next_id += 1;
        tabs.list.push(Tab { id, webview });
        id
    }

    /// Removes a tab, returning its `WebView` (dropping it closes the page).
    /// If it was active, the active tab becomes `None`; the caller picks a successor.
    pub fn unregister_tab(&self, tab: TabId) -> Option<WebView> {
        let mut tabs = self.tabs.borrow_mut();
        let index = tabs.list.iter().position(|t| t.id == tab)?;
        if tabs.active == Some(tab) {
            tabs.active = None;
        }
        Some(tabs.list.remove(index).webview)
    }

    pub fn tab_ids(&self) -> Vec<TabId> {
        self.tabs.borrow().list.iter().map(|t| t.id).collect()
    }

    pub fn tab_id_of(&self, webview: &WebView) -> Option<TabId> {
        let id = webview.id();
        self.tabs
            .borrow()
            .list
            .iter()
            .find(|t| t.webview.id() == id)
            .map(|t| t.id)
    }

    pub fn webview_of(&self, tab: TabId) -> Option<WebView> {
        self.tabs
            .borrow()
            .list
            .iter()
            .find(|t| t.id == tab)
            .map(|t| t.webview.clone())
    }

    pub fn active_tab(&self) -> Option<TabId> {
        self.tabs.borrow().active
    }

    pub fn set_active_tab(&self, tab: Option<TabId>) {
        self.tabs.borrow_mut().active = tab;
    }

    pub fn active_webview(&self) -> Option<WebView> {
        let active = self.active_tab()?;
        self.webview_of(active)
    }
}
