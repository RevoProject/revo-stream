//! Shared system-font-picker popup, hosted by the Workspace as a full-window
//! overlay (the same layering the context menu uses). The font list comes
//! from the engine (`fc-list`) over IPC through
//! [`crate::services::fetch_system_fonts`]; mock builds get a static list.
//!
//! A field asks for the picker through
//! [`crate::actions::ComponentEvent::OpenFontPicker`]; choices come back as
//! typed [`FontPickerEvent`]s and the Workspace routes the font family name
//! to the opening component by its [`crate::actions::FontField`] key. Rows
//! pick immediately; the filter field narrows the list.

use gpui::{
    div, prelude::*, px, Context, EventEmitter, FocusHandle, IntoElement, KeyDownEvent,
    MouseButton, Render, SharedString, Window,
};

use crate::{actions::FontField, components::editable_text::CaretField, theme::RevoTheme};

/// Font size and left padding the filter box renders at.
const FILTER_FONT_PX: f32 = 11.2;
const FILTER_PAD_PX: f32 = 8.;

/// Fixed popup geometry used for the viewport clamp.
pub const FONT_PICKER_WIDTH: f32 = 260.0;
const PICKER_PADDING: f32 = 4.0;
const PICKER_GAP: f32 = 4.0;
const ROW_HEIGHT: f32 = 22.0;
const LIST_MAX_HEIGHT: f32 = 240.0;
const ANCHOR_DROP: f32 = 24.0;
const MARGIN: f32 = 8.0;
/// Rough popup height for the viewport clamp: padding + filter row + gap +
/// the clamped list.
const PICKER_MAX_HEIGHT: f32 =
    2.0 * PICKER_PADDING + ROW_HEIGHT + 2.0 * PICKER_GAP + LIST_MAX_HEIGHT;

#[derive(Clone, Debug, PartialEq)]
pub enum FontPickerEvent {
    /// The chosen font family name for the field that opened the picker.
    Picked {
        field: FontField,
        font: String,
    },
    Cancelled,
}

pub struct FontPickerPopup {
    pub open: bool,
    pub field: Option<FontField>,
    /// Window-space anchor the popup opens under.
    pub position: (f32, f32),
    /// Family names from the engine, sorted case-insensitively.
    pub fonts: Vec<String>,
    /// Whether `fonts` came from a completed fetch (mock fetches resolve
    /// immediately; the engine path fills in later).
    pub fonts_loaded: bool,
    /// Case-insensitive filter draft.
    pub filter: String,
    caret: CaretField,
    filter_focused: bool,
    main_focus: Option<FocusHandle>,
    pub theme: RevoTheme,
}

impl FontPickerPopup {
    /// Creates the workspace-level entity; the focus handle must come from
    /// the constructor context.
    pub fn new(theme: RevoTheme, cx: &mut Context<Self>) -> Self {
        Self {
            open: false,
            field: None,
            position: (0.0, 0.0),
            fonts: Vec::new(),
            fonts_loaded: false,
            filter: String::new(),
            caret: CaretField::default(),
            filter_focused: false,
            main_focus: Some(cx.focus_handle()),
            theme,
        }
    }

    /// Opens the picker for one field. `fonts_loaded` survives closes so the
    /// engine enumeration (one IPC round trip) is reused across opens.
    pub fn open(&mut self, field: FontField, current: String, position: (f32, f32)) {
        self.open = true;
        self.field = Some(field);
        self.position = position;
        self.filter = current;
        self.filter_focused = false;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.field = None;
        self.position = (0.0, 0.0);
        self.filter.clear();
        self.filter_focused = false;
    }

    /// Stores a completed font enumeration (deduplicated, sorted).
    pub fn load_fonts(&mut self, mut fonts: Vec<String>) {
        fonts.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        fonts.dedup();
        self.fonts = fonts;
        self.fonts_loaded = true;
    }

    /// The visible families: the filter draft narrows the list
    /// case-insensitively.
    fn visible_fonts(&self) -> Vec<String> {
        let needle = self.filter.trim().to_lowercase();
        if needle.is_empty() {
            return self.fonts.clone();
        }
        self.fonts
            .iter()
            .filter(|font| font.to_lowercase().contains(&needle))
            .cloned()
            .collect()
    }

    fn pick(&mut self, font: String) -> Option<FontPickerEvent> {
        let field = self.field.clone()?;
        self.close();
        Some(FontPickerEvent::Picked { field, font })
    }
}

impl EventEmitter<FontPickerEvent> for FontPickerPopup {}

impl Render for FontPickerPopup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if !self.open || self.field.is_none() {
            return div();
        }
        let theme = self.theme;
        let viewport = window.viewport_size();
        let (viewport_w, viewport_h) = (f32::from(viewport.width), f32::from(viewport.height));
        let x = (self.position.0 - 40.0)
            .clamp(MARGIN, (viewport_w - FONT_PICKER_WIDTH - MARGIN).max(0.0));
        let y = (self.position.1 + ANCHOR_DROP)
            .clamp(MARGIN, (viewport_h - PICKER_MAX_HEIGHT - MARGIN).max(0.0));

        self.caret.set_value(self.filter.clone());
        let filter_focused = self.filter_focused;
        let mut filter_field = div()
            .id("font-picker-filter")
            .relative()
            .cursor_pointer()
            .w_full()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(if filter_focused {
                theme.accent
            } else {
                theme.border_strong
            })
            .bg(theme.surface_elevated)
            .text_color(if self.filter.is_empty() {
                theme.text_muted
            } else {
                theme.text
            })
            .text_size(px(11.2))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    this.filter_focused = true;
                    this.caret.place_caret(
                        window,
                        event.position.x,
                        &this.filter,
                        FILTER_FONT_PX,
                        FILTER_PAD_PX,
                    );
                    if let Some(focus) = this.main_focus.clone() {
                        focus.focus(window);
                    }
                    cx.notify();
                }),
            );
        filter_field = if filter_focused {
            filter_field.child(self.caret.display(
                window,
                FILTER_FONT_PX,
                FILTER_PAD_PX,
                theme.text,
                theme.accent,
            ))
        } else {
            filter_field.child(if self.filter.is_empty() {
                SharedString::from("Filter fonts…")
            } else {
                SharedString::from(self.filter.clone())
            })
        };
        let filter_field = filter_field.child(self.caret.bounds_probe());

        let mut list = div()
            .id("font-picker-list")
            .flex()
            .flex_col()
            .gap_y(px(1.0))
            .w_full()
            .max_h(px(LIST_MAX_HEIGHT))
            .overflow_y_scroll()
            .scrollbar_width(px(6.));
        if !self.fonts_loaded {
            list = list.child(
                div()
                    .px_2()
                    .py_1()
                    .text_color(theme.text_muted)
                    .text_size(px(11.2))
                    .child("Loading system fonts…"),
            );
        } else {
            let fonts = self.visible_fonts();
            if fonts.is_empty() {
                list = list.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_color(theme.text_muted)
                        .text_size(px(11.2))
                        .child("No fonts match the filter"),
                );
            }
            for font in fonts {
                let font_name = font.clone();
                list = list.child(
                    div()
                        .id(SharedString::from(format!(
                            "font-picker-row-{}",
                            font.replace(' ', "-")
                        )))
                        .cursor_pointer()
                        .h(px(ROW_HEIGHT))
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded_sm()
                        .bg(theme.surface_elevated)
                        .hover(|style| style.bg(theme.surface_selected))
                        .text_color(theme.text)
                        .text_size(px(11.2))
                        .child(SharedString::from(font_name.clone()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(event) = this.pick(font_name.clone()) {
                                cx.emit(event);
                            }
                            cx.notify();
                        })),
                );
            }
        }

        let mut popup = div()
            .id("font-picker-popup")
            .absolute()
            .flex()
            .flex_col()
            .w(px(FONT_PICKER_WIDTH))
            .p(px(PICKER_PADDING))
            .gap(px(PICKER_GAP))
            .bg(theme.surface_elevated)
            .border_1()
            .border_color(theme.border_strong)
            .rounded_md()
            .shadow_md()
            .occlude()
            // Mouse-down inside the popup must not bubble to the backdrop.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if !this.filter_focused {
                    return;
                }
                if let Some(next) = this.caret.keystroke(event, &this.filter) {
                    this.filter = next;
                }
                cx.notify();
            }));
        if let Some(focus) = self.main_focus.clone() {
            popup = popup.track_focus(&focus);
        }
        let popup = popup.child(filter_field).child(list);

        div()
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .left_0()
            .child(
                div()
                    .id("font-picker-backdrop")
                    .absolute()
                    .size_full()
                    .top_0()
                    .left_0()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.close();
                            cx.emit(FontPickerEvent::Cancelled);
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, _, cx| {
                            this.close();
                            cx.emit(FontPickerEvent::Cancelled);
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

    fn picker() -> FontPickerPopup {
        let mut picker = FontPickerPopup {
            open: false,
            field: None,
            position: (0.0, 0.0),
            fonts: Vec::new(),
            fonts_loaded: false,
            filter: String::new(),
            caret: CaretField::default(),
            filter_focused: false,
            main_focus: None,
            theme: RevoTheme::future(),
        };
        picker.load_fonts(vec![
            "Noto Sans".into(),
            "DejaVu Sans".into(),
            "DejaVu Serif".into(),
            "Liberation Sans".into(),
            "Noto Sans".into(),
        ]);
        picker
    }

    #[test]
    fn load_fonts_sorts_case_insensitively_and_deduplicates() {
        let picker = picker();
        assert_eq!(
            picker.fonts,
            vec![
                "DejaVu Sans",
                "DejaVu Serif",
                "Liberation Sans",
                "Noto Sans"
            ]
        );
        assert!(picker.fonts_loaded);
    }

    #[test]
    fn visible_fonts_narrows_case_insensitively() {
        let mut picker = picker();
        assert_eq!(picker.visible_fonts().len(), 4);
        picker.filter = "dejavu".into();
        assert_eq!(picker.visible_fonts(), vec!["DejaVu Sans", "DejaVu Serif"]);
        picker.filter = "zzz".into();
        assert!(picker.visible_fonts().is_empty());
    }

    #[test]
    fn pick_emits_only_with_a_field_and_closes() {
        let mut picker = picker();
        assert_eq!(picker.pick("DejaVu Sans".into()), None);
        picker.open(FontField::AddSource, "DejaVu Sans".into(), (10.0, 10.0));
        assert!(picker.open);
        assert_eq!(
            picker.pick("Liberation Sans".into()),
            Some(FontPickerEvent::Picked {
                field: FontField::AddSource,
                font: "Liberation Sans".into(),
            })
        );
        assert!(!picker.open);
        assert!(picker.field.is_none());
    }
}
