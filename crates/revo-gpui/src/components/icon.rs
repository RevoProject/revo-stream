//! Lucide-style icon rendering for the native shell.
//!
//! Icons are embedded SVGs (see [`crate::assets`]) rendered through GPUI's
//! `svg()` element, which masks the path geometry with the given text color.

use gpui::{prelude::*, svg};

/// Renders one of the embedded `icons/*.svg` assets at `size`, colored with
/// `color`.
pub fn icon(
    theme_color: impl Into<gpui::Hsla>,
    name: &'static str,
    size: f32,
) -> gpui::Svg {
    svg()
        .path(format!("icons/{name}.svg"))
        .size(gpui::px(size), gpui::px(size))
        .flex_none()
        .text_color(theme_color)
}
