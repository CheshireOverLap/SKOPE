//! Application module for skope_ui
//!
//! Provides window and event loop integration using winit

mod slate_app;

pub use slate_app::{SlateApp, SlateAppConfig, SlateAppHandler, FloatingWindowRequest, RedockRequest};
