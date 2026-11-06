//! Native ports of the Tauri auxiliary-window routes
//! (`src/routes/{preview,debug-console,interact}`) as standalone GPUI
//! windows. Each root view owns a clone of the shared
//! [`crate::services::AnyBackend`]; in mock mode every surface is
//! structurally live with documented placeholders where a real libobs frame
//! or CDP session is required.

pub mod debug_console;
pub mod interact;
pub mod preview;
