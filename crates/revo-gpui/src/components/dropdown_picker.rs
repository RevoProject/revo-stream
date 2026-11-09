//! Shared dropdown-list popup, hosted by the Workspace as a full-window
//! overlay so it escapes scrollable modal bodies (the same reason the
//! color/font pickers live at workspace level). Replaces click-to-cycle
//! selectors with a real expanding list.

use gpui::{
    div, prelude::*, px, Context, EventEmitter, IntoElement, MouseButton, Render, SharedString,
    Window,
};

use crate::{actions::DropdownField, theme::RevoTheme};

pub const DROPDOWN_WIDTH: f32 = 220.0;
const ROW_HEIGHT: f32 = 26.0;
const LIST_MAX_HEIGHT: f32 = 260.0;
const ANCHOR_DROP: f32 = 24.0;
const MARGIN: f32 = 8.0;

#[derive(Clone, Debug, PartialEq)]
pub enum DropdownPickerEvent {
    Picked { field: DropdownField, value: String },
    Cancelled,
}

pub struct DropdownPickerPopup {
    pub open: bool,
    pub field: Option<DropdownField>,
    pub position: (f32, f32),
    /// `(value, label)` pairs shown in order.
    pub options: Vec<(String, String)>,
    pub current: String,
    pub theme: RevoTheme,
}

impl Default for DropdownPickerPopup {
    fn default() -> Self {
        Self {
            open: false,
            field: None,
            position: (0.0, 0.0),
            options: Vec::new(),
            current: String::new(),
            theme: RevoTheme::future(),
        }
    }
}

impl EventEmitter<DropdownPickerEvent> for DropdownPickerPopup {}

impl DropdownPickerPopup {
    pub fn open(
        &mut self,
        field: DropdownField,
        options: Vec<(String, String)>,
        current: String,
        position: (f32, f32),
    ) {
        self.open = true;
        self.field = Some(field);
        self.options = options;
        self.current = current;
        self.position = position;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.field = None;
        self.options.clear();
        self.current.clear();
        self.position = (0.0, 0.0);
    }

    fn pick(&mut self, value: String) -> Option<DropdownPickerEvent> {
        let field = self.field.clone()?;
        self.close();
        Some(DropdownPickerEvent::Picked { field, value })
    }
}

impl Render for DropdownPickerPopup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if !self.open || self.field.is_none() {
            return div();
        }
        let theme = self.theme;
        let viewport = window.viewport_size();
        let (viewport_w, viewport_h) = (f32::from(viewport.width), f32::from(viewport.height));
        let list_height = (self.options.len() as f32 * ROW_HEIGHT + 8.0).min(LIST_MAX_HEIGHT);
        let popup_height = list_height + 8.0;
        let x =
            (self.position.0 - 40.0).clamp(MARGIN, (viewport_w - DROPDOWN_WIDTH - MARGIN).max(0.0));
        let y = (self.position.1 + ANCHOR_DROP)
            .clamp(MARGIN, (viewport_h - popup_height - MARGIN).max(0.0));

        let mut list = div()
            .id("dropdown-picker-list")
            .flex()
            .flex_col()
            .w_full()
            .max_h(px(LIST_MAX_HEIGHT))
            .overflow_y_scroll()
            .scrollbar_width(px(6.));
        for (index, (value, label)) in self.options.iter().enumerate() {
            let selected = value == &self.current;
            let pick_value = value.clone();
            list = list.child(
                div()
                    .id(SharedString::from(format!("dropdown-picker-{index}")))
                    .cursor_pointer()
                    .h(px(ROW_HEIGHT))
                    .px_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .rounded_sm()
                    .bg(if selected {
                        theme.surface_selected
                    } else {
                        theme.surface_elevated
                    })
                    .hover(|style| style.bg(theme.surface_active))
                    .text_color(if selected {
                        theme.text_strong
                    } else {
                        theme.text
                    })
                    .text_size(px(11.2))
                    .child(SharedString::from(label.clone()))
                    .when(selected, |row| {
                        row.child(
                            div()
                                .text_color(theme.text_muted)
                                .text_size(px(10.))
                                .child("✓"),
                        )
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(event) = this.pick(pick_value.clone()) {
                            cx.emit(event);
                        }
                        cx.notify();
                    })),
            );
        }

        let popup = div()
            .id("dropdown-picker-popup")
            .absolute()
            .flex()
            .flex_col()
            .w(px(DROPDOWN_WIDTH))
            .p(px(4.))
            .bg(theme.surface_elevated)
            .border_1()
            .border_color(theme.border_strong)
            .rounded_md()
            .shadow_md()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .child(list);

        div()
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .left_0()
            .child(
                div()
                    .id("dropdown-picker-backdrop")
                    .absolute()
                    .size_full()
                    .top_0()
                    .left_0()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.close();
                            cx.emit(DropdownPickerEvent::Cancelled);
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, _, cx| {
                            this.close();
                            cx.emit(DropdownPickerEvent::Cancelled);
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
    use crate::actions::DropdownField;

    #[test]
    fn pick_emits_only_with_a_field_and_closes() {
        let mut picker = DropdownPickerPopup::default();
        assert_eq!(picker.pick("x".into()), None);
        picker.open(
            DropdownField::AddFilterKind {
                source_id: "s1".into(),
            },
            vec![("gain".into(), "gain".into())],
            "gain".into(),
            (1.0, 2.0),
        );
        assert_eq!(
            picker.pick("gain".into()),
            Some(DropdownPickerEvent::Picked {
                field: DropdownField::AddFilterKind {
                    source_id: "s1".into(),
                },
                value: "gain".into(),
            })
        );
        assert!(!picker.open);
    }

    #[test]
    fn open_clears_previous_state() {
        let mut picker = DropdownPickerPopup::default();
        picker.open(
            DropdownField::AddSourceOption { key: "a".into() },
            vec![("x".into(), "X".into())],
            "x".into(),
            (0.0, 0.0),
        );
        picker.close();
        assert!(picker.options.is_empty());
        assert!(picker.field.is_none());
    }
}
