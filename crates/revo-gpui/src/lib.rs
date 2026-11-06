pub mod actions;
pub mod app;
pub mod assets;
pub mod components;
pub mod services;
pub mod theme;
pub mod windows;

pub use actions::{AuxiliaryWindowKind, ComponentEvent, ModalEvent};
pub use app::{run_native_app, ActiveModal, LayoutMode, Workspace};
pub use revo_core::service::BackendCommand;
pub use theme::{RevoTheme, ThemeKind};

// Keep the framework-neutral domain crate visible to native consumers.
pub use revo_core as core;
