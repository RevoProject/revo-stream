//! Shared color-picker popup, hosted by the Workspace as a full-window
//! overlay (the same layering the context menu uses) so it works above any
//! modal card — including scrollable bodies, which clip absolutely
//! positioned children.
//!
//! A field asks for the picker through
//! [`crate::actions::ComponentEvent::OpenColorPicker`]; choices come back as
//! typed [`ColorPickerEvent`]s and the Workspace routes the color to the
//! opening component by its [`crate::actions::ColorField`] key. The popup
//! edits an HSV draft in place (saturation/value area, hue strip, preset
//! swatches) and commits it with Apply; clicking the backdrop cancels.

use gpui::{
    div, prelude::*, px, Context, EventEmitter, IntoElement, MouseButton, MouseDownEvent, Render,
    SharedString, Window,
};

use crate::{actions::ColorField, theme::RevoTheme};

/// Fixed popup geometry; the click→color math depends on these constants
/// staying in sync with the rendered layout.
pub const PICKER_WIDTH: f32 = 190.0;
const PICKER_PADDING: f32 = 4.0;
const PICKER_GAP: f32 = 4.0;
pub const SV_HEIGHT: f32 = 140.0;
const HUE_HEIGHT: f32 = 12.0;
const PREVIEW_HEIGHT: f32 = 20.0;
const PRESETS_HEIGHT: f32 = 20.0;
const BUTTONS_HEIGHT: f32 = 24.0;
/// Total popup height (padding + SV + hue + preview + presets + buttons +
/// the four inter-section gaps).
pub const PICKER_HEIGHT: f32 = 2.0 * PICKER_PADDING
    + SV_HEIGHT
    + PICKER_GAP
    + HUE_HEIGHT
    + PICKER_GAP
    + PREVIEW_HEIGHT
    + PICKER_GAP
    + PRESETS_HEIGHT
    + PICKER_GAP
    + BUTTONS_HEIGHT;
/// Vertical offset of the saturation/value area inside the popup.
const SV_Y_OFFSET: f32 = PICKER_PADDING;
/// Vertical offset of the hue strip inside the popup.
const HUE_Y_OFFSET: f32 = PICKER_PADDING + SV_HEIGHT + PICKER_GAP;
/// The dropdown popups anchor this far below the cursor (settings parity).
const ANCHOR_DROP: f32 = 24.0;
const MARGIN: f32 = 8.0;

/// Preset swatches shown under the SV area; the quick-action modal's
/// recent-colors row stays the history mechanism for that field.
const DEFAULT_PRESETS: [&str; 8] = [
    "#FFFFFF", "#000000", "#FF3B30", "#FF9500", "#FFE100", "#28C840", "#285CFC", "#8B5CF6",
];

#[derive(Clone, Debug, PartialEq)]
pub enum ColorPickerEvent {
    /// The confirmed `#RRGGBB` color for the field that opened the picker.
    Picked {
        field: ColorField,
        color: String,
    },
    Cancelled,
}

/// Parses `#RRGGBB` (optional `#`) into 0..1 RGB; `None` for anything else.
pub fn hex_to_rgb(hex: &str) -> Option<(f32, f32, f32)> {
    let trimmed = hex.trim().trim_start_matches('#');
    if trimmed.len() != 6 || !trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |slice: &str| u8::from_str_radix(slice, 16).ok().map(|v| v as f32 / 255.0);
    Some((
        channel(&trimmed[0..2])?,
        channel(&trimmed[2..4])?,
        channel(&trimmed[4..6])?,
    ))
}

/// Formats 0..1 RGB channels as `#RRGGBB`.
pub fn rgb_to_hex(r: f32, g: f32, b: f32) -> String {
    let channel = |value: f32| {
        let byte = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("{:02X}", byte)
    };
    format!("#{}{}{}", channel(r), channel(g), channel(b))
}

/// `#RRGGBB` → `(hue degrees 0..360, saturation 0..1, value 0..1)`.
pub fn hex_to_hsv(hex: &str) -> (f32, f32, f32) {
    let Some((r, g, b)) = hex_to_rgb(hex) else {
        return (0.0, 0.0, 0.0);
    };
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let saturation = if max <= 0.0 { 0.0 } else { delta / max };
    let hue = if delta <= 0.0 {
        0.0
    } else if (max - r).abs() < f32::EPSILON {
        let fraction = (g - b) / delta;
        let fraction = if fraction < 0.0 {
            fraction + 6.0
        } else {
            fraction
        };
        fraction * 60.0
    } else if (max - g).abs() < f32::EPSILON {
        ((b - r) / delta + 2.0) * 60.0
    } else {
        ((r - g) / delta + 4.0) * 60.0
    };
    (hue, saturation, max)
}

/// `(hue degrees, saturation 0..1, value 0..1)` → `#RRGGBB`.
pub fn hsv_to_hex(h: f32, s: f32, v: f32) -> String {
    let hue = h.rem_euclid(360.0);
    let c = v * s;
    let sector = (hue / 60.0).floor();
    let remainder = hue / 60.0 - sector;
    let (r1, g1, b1) = match sector as i64 % 6 {
        0 => (c, c * remainder, 0.0),
        1 => (c * (1.0 - remainder), c, 0.0),
        2 => (0.0, c, c * remainder),
        3 => (0.0, c * (1.0 - remainder), c),
        4 => (c * remainder, 0.0, c),
        _ => (c, 0.0, c * (1.0 - remainder)),
    };
    let m = v - c;
    rgb_to_hex(r1 + m, g1 + m, b1 + m)
}

/// One interaction inside the picker popup.
#[derive(Clone, Debug, PartialEq)]
enum ColorPick {
    /// Saturation/value area position, both 0..1.
    Sv {
        s: f32,
        v: f32,
    },
    /// Hue strip position in degrees.
    Hue {
        h: f32,
    },
    /// A preset swatch value (`#RRGGBB`).
    Preset(String),
    Apply,
    Cancel,
}

pub struct ColorPickerPopup {
    pub open: bool,
    pub field: Option<ColorField>,
    /// Window-space anchor the popup opens under.
    pub position: (f32, f32),
    /// The draft color as `#RRGGBB`.
    pub draft: String,
    pub theme: RevoTheme,
}

impl Default for ColorPickerPopup {
    fn default() -> Self {
        Self {
            open: false,
            field: None,
            position: (0.0, 0.0),
            draft: "#FFFFFF".into(),
            theme: RevoTheme::future(),
        }
    }
}

impl EventEmitter<ColorPickerEvent> for ColorPickerPopup {}

impl ColorPickerPopup {
    pub fn open(&mut self, field: ColorField, current: String, position: (f32, f32)) {
        self.open = true;
        self.field = Some(field);
        self.position = position;
        self.draft = normalize(current);
    }

    pub fn close(&mut self) {
        self.open = false;
        self.field = None;
        self.position = (0.0, 0.0);
    }

    /// Applies one interaction; `Some` when the popup should emit (Apply or
    /// Cancel), `None` while the draft is still being edited.
    fn choose(&mut self, pick: ColorPick) -> Option<ColorPickerEvent> {
        let field = self.field.clone()?;
        match pick {
            ColorPick::Sv { s, v } => {
                let (h, _, _) = hex_to_hsv(&self.draft);
                self.draft = hsv_to_hex(h, s.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
                None
            }
            ColorPick::Hue { h } => {
                let (_, s, v) = hex_to_hsv(&self.draft);
                // An achromatic draft would look stuck on the hue wheel;
                // drag from full saturation/value instead.
                let (s, v) = if s <= 0.0 {
                    (1.0, if v <= 0.0 { 1.0 } else { v })
                } else {
                    (s, v)
                };
                self.draft = hsv_to_hex(h, s, v);
                None
            }
            ColorPick::Preset(color) => {
                self.draft = normalize(color);
                None
            }
            ColorPick::Apply => {
                let color = self.draft.clone();
                self.close();
                Some(ColorPickerEvent::Picked { field, color })
            }
            ColorPick::Cancel => {
                self.close();
                Some(ColorPickerEvent::Cancelled)
            }
        }
    }
}

/// Normalizes one accepted input form to `#RRGGBB`: either a hex string or
/// an OBS ABGR integer arriving un-normalized from settings payloads.
fn normalize(value: String) -> String {
    let trimmed = value.trim().trim_start_matches('#');
    if trimmed.len() == 6 && trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return format!("#{}", trimmed.to_ascii_uppercase());
    }
    if let Ok(abgr) = trimmed.parse::<u32>() {
        let r = (abgr & 0xFF) as u8;
        let g = ((abgr >> 8) & 0xFF) as u8;
        let b = ((abgr >> 16) & 0xFF) as u8;
        return format!("#{:02X}{:02X}{:02X}", r, g, b);
    }
    "#FFFFFF".into()
}

impl Render for ColorPickerPopup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if !self.open || self.field.is_none() {
            return div();
        }
        let theme = self.theme;
        let viewport = window.viewport_size();
        let (viewport_w, viewport_h) = (f32::from(viewport.width), f32::from(viewport.height));
        let x =
            (self.position.0 - 40.0).clamp(MARGIN, (viewport_w - PICKER_WIDTH - MARGIN).max(0.0));
        let y = (self.position.1 + ANCHOR_DROP)
            .clamp(MARGIN, (viewport_h - PICKER_HEIGHT - MARGIN).max(0.0));
        let draft = self.draft.clone();
        let (hue, saturation, value) = hex_to_hsv(&draft);
        let sv_width = PICKER_WIDTH - 2.0 * PICKER_PADDING;

        let sv_handler = cx.listener(move |this, event: &MouseDownEvent, _, cx| {
            let (x, y) = (this.position.0 - 40.0, this.position.1 + ANCHOR_DROP);
            let s =
                ((f32::from(event.position.x) - (x + PICKER_PADDING)) / sv_width).clamp(0.0, 1.0);
            let v = 1.0
                - ((f32::from(event.position.y) - (y + SV_Y_OFFSET)) / SV_HEIGHT).clamp(0.0, 1.0);
            if let Some(event) = this.choose(ColorPick::Sv { s, v }) {
                cx.emit(event);
            }
            cx.notify();
        });

        let hue_handler = cx.listener(move |this, event: &MouseDownEvent, _, cx| {
            let y = this.position.1 + ANCHOR_DROP;
            let h = ((f32::from(event.position.y) - (y + HUE_Y_OFFSET)) / HUE_HEIGHT * 360.0)
                .clamp(0.0, 359.99);
            if let Some(event) = this.choose(ColorPick::Hue { h }) {
                cx.emit(event);
            }
            cx.notify();
        });

        // Saturation/value area: hue base + white (left→right) + black
        // (top→bottom) overlay gradients; the cursor dot sits at the draft.
        let hue_base = gpui::rgb(
            u32::from_str_radix(hsv_to_hex(hue, 1.0, 1.0).trim_start_matches('#'), 16)
                .unwrap_or(0xffffff),
        );
        let sv_area = div()
            .relative()
            .w_full()
            .h(px(SV_HEIGHT))
            .rounded_sm()
            .border_1()
            .border_color(theme.border_strong)
            .on_mouse_down(MouseButton::Left, sv_handler)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .rounded_sm()
                    .bg(hue_base),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .rounded_sm()
                    .bg(gpui::linear_gradient(
                        90.0,
                        gpui::linear_color_stop(gpui::rgba(0xffffff_ff), 0.0),
                        gpui::linear_color_stop(gpui::rgba(0xffffff_00), 1.0),
                    )),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .rounded_sm()
                    .bg(gpui::linear_gradient(
                        180.0,
                        gpui::linear_color_stop(gpui::rgba(0x000000_00), 0.0),
                        gpui::linear_color_stop(gpui::rgba(0x000000_ff), 1.0),
                    )),
            )
            .child(
                div()
                    .absolute()
                    .left(px(saturation * sv_width - 5.0))
                    .top(px((1.0 - value) * SV_HEIGHT - 5.0))
                    .w(px(10.0))
                    .h(px(10.0))
                    .rounded_full()
                    .border_2()
                    .border_color(gpui::rgba(0xffffff_ff)),
            );

        // Hue strip: six 2-stop segments cover the wheel without multi-stop
        // gradients (gpui backgrounds carry exactly two color stops).
        let hue_stops: [(u32, u32); 6] = [
            (0xff0000, 0xffff00),
            (0xffff00, 0x00ff00),
            (0x00ff00, 0x00ffff),
            (0x00ffff, 0x0000ff),
            (0x0000ff, 0xff00ff),
            (0xff00ff, 0xff0000),
        ];
        let mut strip = div()
            .relative()
            .flex()
            .flex_row()
            .w_full()
            .h(px(HUE_HEIGHT))
            .rounded_sm()
            .border_1()
            .border_color(theme.border_strong)
            .on_mouse_down(MouseButton::Left, hue_handler);
        for (index, (from, to)) in hue_stops.iter().enumerate() {
            strip = strip.child(
                div()
                    .flex_1()
                    .h_full()
                    .bg(gpui::linear_gradient(
                        90.0,
                        gpui::linear_color_stop(gpui::rgb(*from), 0.0),
                        gpui::linear_color_stop(gpui::rgb(*to), 1.0),
                    ))
                    .when(index == 0, |edge| edge.rounded_l_sm())
                    .when(index == hue_stops.len() - 1, |edge| edge.rounded_r_sm()),
            );
        }
        let hue_x = hue / 360.0 * sv_width;
        strip = strip.child(
            div()
                .absolute()
                .left(px(hue_x - 1.0))
                .top(px(-2.0))
                .w(px(2.0))
                .h(px(HUE_HEIGHT + 4.0))
                .rounded_sm()
                .bg(gpui::rgba(0xffffff_ff))
                .border_1()
                .border_color(gpui::rgba(0x000000_88)),
        );

        // Draft preview row.
        let draft_swatch = hex_to_rgb(&draft)
            .map(|(r, g, b)| rgb_to_hex(r, g, b))
            .unwrap_or_else(|| draft.clone());
        let preview_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .h(px(PREVIEW_HEIGHT))
            .child(
                div()
                    .w(px(14.0))
                    .h(px(14.0))
                    .rounded_sm()
                    .border_1()
                    .border_color(theme.border_strong)
                    .bg(gpui::rgb(
                        u32::from_str_radix(draft_swatch.trim_start_matches('#'), 16)
                            .unwrap_or(0xffffff),
                    )),
            )
            .child(
                div()
                    .text_color(theme.text)
                    .text_size(px(11.2))
                    .child(SharedString::from(draft.clone())),
            );

        // Preset swatches.
        let mut preset_row = div().flex().flex_row().gap(px(2.0)).h(px(PRESETS_HEIGHT));
        for preset in DEFAULT_PRESETS {
            let color = preset.to_string();
            preset_row = preset_row.child(
                div()
                    .id(SharedString::from(format!("color-preset-{preset}")))
                    .cursor_pointer()
                    .w(px(18.0))
                    .h(px(18.0))
                    .rounded_sm()
                    .border_1()
                    .border_color(theme.border_strong)
                    .bg(gpui::rgb(
                        u32::from_str_radix(preset.trim_start_matches('#'), 16).unwrap_or(0xffffff),
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(event) = this.choose(ColorPick::Preset(color.clone())) {
                            cx.emit(event);
                        }
                        cx.notify();
                    })),
            );
        }

        // Apply / Cancel row.
        let buttons_row = div()
            .flex()
            .flex_row()
            .justify_end()
            .gap_2()
            .h(px(BUTTONS_HEIGHT))
            .child(
                div()
                    .id("color-picker-cancel")
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(theme.surface_active)
                    .text_color(theme.text)
                    .text_size(px(11.2))
                    .child("Cancel")
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(event) = this.choose(ColorPick::Cancel) {
                            cx.emit(event);
                        }
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("color-picker-apply")
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(theme.accent)
                    .text_color(theme.text_strong)
                    .text_size(px(11.2))
                    .child("Apply")
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(event) = this.choose(ColorPick::Apply) {
                            cx.emit(event);
                        }
                        cx.notify();
                    })),
            );

        let popup = div()
            .id("color-picker-popup")
            .absolute()
            .flex()
            .flex_col()
            .w(px(PICKER_WIDTH))
            .p(px(PICKER_PADDING))
            .gap(px(PICKER_GAP))
            .bg(theme.surface_elevated)
            .border_1()
            .border_color(theme.border_strong)
            .rounded_md()
            .shadow_md()
            .occlude()
            // Mouse-down inside the popup must not bubble to the backdrop
            // below, or the popup closes before the interaction completes.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .child(sv_area)
            .child(strip)
            .child(preview_row)
            .child(preset_row)
            .child(buttons_row);

        div()
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .left_0()
            .child(
                div()
                    .id("color-picker-backdrop")
                    .absolute()
                    .size_full()
                    .top_0()
                    .left_0()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if let Some(event) = this.choose(ColorPick::Cancel) {
                                cx.emit(event);
                            }
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, _, cx| {
                            if let Some(event) = this.choose(ColorPick::Cancel) {
                                cx.emit(event);
                            }
                            cx.notify();
                        }),
                    ),
            )
            .child(div().absolute().left(px(x)).top(px(y)).child(popup))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_rgb_roundtrip_and_rejection() {
        // Exact byte round trips: rgb_to_hex quantizes each channel back to
        // the source byte.
        assert_eq!(rgb_to_hex(0.15686275, 0.36078432, 0.9882353), "#285CFC");
        assert_eq!(rgb_to_hex(0.0, 0.0, 0.0), "#000000");
        assert_eq!(rgb_to_hex(1.0, 1.0, 1.0), "#FFFFFF");
        assert_eq!(rgb_to_hex(1.0, 0.5, 0.0), "#FF8000");
        assert_eq!(rgb_to_hex(-1.0, 2.0, 0.5), "#00FF80");
        // Round trip: parse what we format.
        for hex in ["#000000", "#285CFC", "#FFFFFF", "#F3C231"] {
            let (r, g, b) = hex_to_rgb(hex).expect("valid hex");
            assert_eq!(rgb_to_hex(r, g, b), hex.to_ascii_uppercase());
        }
        assert_eq!(hex_to_rgb("#285CF"), None);
        assert_eq!(hex_to_rgb("nope"), None);
        assert_eq!(hex_to_rgb("FFFFFF"), Some((1.0, 1.0, 1.0)));
    }

    #[test]
    fn hsv_conversions_round_trip_primary_colors() {
        assert_eq!(hex_to_hsv("#FF0000"), (0.0, 1.0, 1.0));
        assert_eq!(hex_to_hsv("#00FF00"), (120.0, 1.0, 1.0));
        assert_eq!(hex_to_hsv("#0000FF"), (240.0, 1.0, 1.0));
        assert_eq!(hex_to_hsv("#000000"), (0.0, 0.0, 0.0));
        assert_eq!(hsv_to_hex(0.0, 1.0, 1.0), "#FF0000");
        assert_eq!(hsv_to_hex(120.0, 1.0, 1.0), "#00FF00");
        assert_eq!(hsv_to_hex(240.0, 1.0, 1.0), "#0000FF");
        assert_eq!(hsv_to_hex(0.0, 0.0, 1.0), "#FFFFFF");
        // Lossless round trip for an arbitrary mid color.
        let (h, s, v) = hex_to_hsv("#285CFC");
        assert_eq!(hsv_to_hex(h, s, v), "#285CFC");
    }

    #[test]
    fn abgr_settings_values_normalize_to_hex() {
        // OBS color_source white (0xFFFFFFFF) and OBS green (0xFF00FF00 ABGR
        // = #00FF00) arrive as plain ints.
        assert_eq!(normalize("4294967295".into()), "#FFFFFF");
        assert_eq!(normalize("4278255360".into()), "#00FF00");
        assert_eq!(normalize("285cfc".into()), "#285CFC");
        assert_eq!(normalize("garbage".into()), "#FFFFFF");
    }

    #[test]
    fn popup_geometry_constants_match_the_layout() {
        // The click→color math reads these offsets; they must equal the
        // rendered column: padding, SV, gap, hue, gap, preview, gap,
        // presets, gap, buttons, padding.
        assert_eq!(SV_Y_OFFSET, PICKER_PADDING);
        assert_eq!(HUE_Y_OFFSET, PICKER_PADDING + SV_HEIGHT + PICKER_GAP);
        assert_eq!(
            PICKER_HEIGHT, 240.0,
            "preview/preset/button rows shifted; re-check the math constants"
        );
    }
}
