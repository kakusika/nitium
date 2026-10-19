pub mod adapter;
pub mod controls;
pub mod delegate;
pub mod event;
pub mod events_utils;
pub mod rendering_context;
pub mod waker;
pub mod webview;

#[cfg(any(target_os = "linux", target_os = "android", target_os = "windows"))]
pub(crate) mod gl_bindings {
    #![allow(unsafe_op_in_unsafe_fn)]
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

pub use controls::{
    ControlId, DialogKind, MenuEntry, PageDialog, PageMenu, PageSelect, SelectChoice,
};
pub use event::{BrowserEvent, CompositionPhase, DeviceRect, TabId};
pub use webview::ServoBrowser;
