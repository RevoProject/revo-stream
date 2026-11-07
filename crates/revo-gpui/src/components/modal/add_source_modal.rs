//! Functional native Add Source modal: a grid of available OBS source-type
//! tiles (seeded from the backend snapshot), a name field, and a
//! Create/Cancel footer. Creating dispatches `BackendCommand::AddSource`,
//! which creates the source and places it into the current scene.

use gpui::{
    div, prelude::*, px, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    MouseButton, Render, SharedString, Window,
};

use revo_core::models::SourceTypeItem;

use crate::{
    components::modal_shell::{modal_shell_with_body, ModalConfig, ModalSize},
    components::text_field::apply_line_keystroke,
    theme::RevoTheme,
};

#[derive(Clone, Debug, PartialEq)]
pub enum AddSourceEvent {
    /// The typed source name and the selected source type id.
    Created {
        name: String,
        source_type: String,
    },
    Cancelled,
}

pub struct AddSourceModal {
    pub theme: RevoTheme,
    pub source_types: Vec<SourceTypeItem>,
    pub selected_type: Option<String>,
    pub name: String,
    field_focus: FocusHandle,
    field_focused: bool,
    error: Option<String>,
}

impl EventEmitter<AddSourceEvent> for AddSourceModal {}

impl Focusable for AddSourceModal {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.field_focus.clone()
    }
}

impl AddSourceModal {
    pub fn new(source_types: Vec<SourceTypeItem>, cx: &mut Context<Self>) -> Self {
        Self {
            theme: RevoTheme::future(),
            source_types,
            selected_type: None,
            name: String::new(),
            field_focus: cx.focus_handle(),
            field_focused: false,
            error: None,
        }
    }

    /// Selects one source type tile; unknown ids are rejected so the modal
    /// never submits a type the engine cannot create.
    pub fn select(&mut self, source_type: &str) -> bool {
        if !self.source_types.iter().any(|item| item.id == source_type) {
            return false;
        }
        self.selected_type = Some(source_type.to_owned());
        true
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(source_type) = self.selected_type.clone() else {
            self.error = Some("Choose a source type first".into());
            cx.notify();
            return;
        };
        let name = self.name.trim();
        if name.is_empty() {
            self.error = Some("Source name required".into());
            cx.notify();
            return;
        }
        cx.emit(AddSourceEvent::Created {
            name: name.to_owned(),
            source_type,
        });
    }

    fn name_field(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_color(self.theme.text_secondary)
                    .text_size(gpui::px(12.))
                    .child("Source name"),
            )
            .child(
                div()
                    .id("add-source-field")
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(if self.field_focused {
                        self.theme.accent
                    } else {
                        self.theme.border_strong
                    })
                    .bg(self.theme.surface_elevated)
                    .text_color(if self.name.is_empty() {
                        self.theme.text_muted
                    } else {
                        self.theme.text
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.field_focused = true;
                            this.field_focus.focus(window);
                            cx.notify();
                        }),
                    )
                    .child(if self.name.is_empty() {
                        SharedString::from("Source name…")
                    } else {
                        SharedString::from(self.name.clone())
                    }),
            )
    }

    fn type_tiles(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        let mut grid = div().flex().flex_wrap().gap_1().min_w_0();
        for (index, item) in self.source_types.clone().into_iter().enumerate() {
            let selected = self.selected_type.as_deref() == Some(item.id.as_str());
            let type_id = item.id.clone();
            grid = grid.child(
                div()
                    .id(SharedString::from(format!("source-type-{index}")))
                    .cursor_pointer()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .w(px(148.))
                    .px_2()
                    .py_1p5()
                    .rounded_md()
                    .border_1()
                    .border_color(if selected {
                        self.theme.accent
                    } else {
                        self.theme.border_strong
                    })
                    .bg(if selected {
                        self.theme.surface_active
                    } else {
                        self.theme.surface_elevated
                    })
                    .text_color(if selected {
                        self.theme.text_strong
                    } else {
                        self.theme.text
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select(&type_id);
                        this.error = None;
                        cx.notify();
                    }))
                    .child(crate::components::icon::icon(
                        if selected {
                            self.theme.text_strong
                        } else {
                            self.theme.text_muted
                        },
                        crate::components::home::sources_panel::source_type_icon(
                            &item.id,
                            item.audio_capable,
                        ),
                        18.,
                    ))
                    .child(
                        div()
                            .text_size(px(11.2))
                            .font_weight(gpui::FontWeight(500.))
                            .truncate()
                            .child(item.label.clone()),
                    )
                    .child(
                        div()
                            .text_color(self.theme.text_muted)
                            .text_size(px(9.2))
                            .truncate()
                            .child(item.id.clone()),
                    ),
            );
        }
        if self.source_types.is_empty() {
            grid = grid.child(
                div()
                    .text_color(self.theme.text_muted)
                    .child("No source types available yet — start the engine to scan."),
            );
        }
        grid
    }
}

impl Render for AddSourceModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let error_note = self.error.clone().unwrap_or_default();

        let config = ModalConfig::standard("Add Source", ModalSize::Large);
        let theme = self.theme;
        let footer = div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .p_3()
            .child(
                div()
                    .text_color(theme.danger)
                    .child(SharedString::from(error_note)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id("add-source-cancel")
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(theme.surface_active)
                            .text_color(theme.text)
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(AddSourceEvent::Cancelled);
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("add-source-create")
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(theme.accent)
                            .text_color(theme.text_strong)
                            .on_click(cx.listener(|this, _, _, cx| this.confirm(cx)))
                            .child("Create"),
                    ),
            )
            .into_any_element();

        let on_key = cx.listener(|this, event: &KeyDownEvent, _, cx| {
            if this.field_focused {
                let keystroke = &event.keystroke;
                if let Some(next) = apply_line_keystroke(
                    &this.name,
                    &keystroke.key,
                    keystroke.key_char.as_deref(),
                    keystroke.modifiers.control,
                    keystroke.modifiers.alt,
                    keystroke.modifiers.platform,
                ) {
                    this.name = next;
                }
                cx.notify();
            }
        });

        let close = div()
            .id("add-source-close")
            .cursor_pointer()
            .text_color(self.theme.text_muted)
            .child("Close")
            .on_click(cx.listener(|_, _, _, cx| cx.emit(AddSourceEvent::Cancelled)))
            .into_any_element();
        modal_shell_with_body(
            &config,
            self.theme,
            div()
                .flex()
                .flex_col()
                .gap_2()
                .size_full()
                .track_focus(&self.field_focus)
                .child(self.type_tiles(cx))
                .child(self.name_field(cx))
                .on_key_down(on_key),
            Some(footer),
            Some(close),
        )
    }
}
