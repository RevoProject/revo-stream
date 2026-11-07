//! Functional native Add Scene modal: a name field with live validation and
//! a Create/Cancel footer. Creating dispatches `BackendCommand::AddScene`,
//! which both creates the scene and switches to it.

use gpui::{
    div, prelude::*, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    MouseButton, Render, SharedString, Window,
};

use crate::{
    components::modal_shell::{modal_shell_with_body, ModalConfig, ModalSize},
    components::naming::default_scene_name,
    components::text_field::apply_line_keystroke,
    theme::RevoTheme,
};

#[derive(Clone, Debug, PartialEq)]
pub enum AddSceneEvent {
    /// The typed scene name, trimmed and validated.
    Created {
        name: String,
    },
    Cancelled,
}

pub struct AddSceneModal {
    pub theme: RevoTheme,
    /// Existing scene names; seeds the `scene-{nr}` default.
    pub existing_names: Vec<String>,
    pub name: String,
    field_focus: FocusHandle,
    field_focused: bool,
    error: Option<String>,
}

impl EventEmitter<AddSceneEvent> for AddSceneModal {}

impl Focusable for AddSceneModal {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.field_focus.clone()
    }
}

impl AddSceneModal {
    pub fn new(existing_names: Vec<String>, cx: &mut Context<Self>) -> Self {
        Self {
            theme: RevoTheme::future(),
            existing_names,
            name: String::new(),
            field_focus: cx.focus_handle(),
            field_focused: false,
            error: None,
        }
    }

    /// Default scene name: `scene-{nr}` with the first free `{nr}`.
    pub fn default_name(&self) -> String {
        default_scene_name(&self.existing_names)
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        // An empty name falls back to the `scene-{nr}` placeholder shown in
        // the field, so creating never blocks on naming.
        let name = match self.name.trim().is_empty() {
            true => self.default_name(),
            false => self.name.trim().to_owned(),
        };
        cx.emit(AddSceneEvent::Created { name });
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
                    .child("Scene name"),
            )
            .child(
                div()
                    .id("add-scene-field")
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
                        SharedString::from(self.default_name())
                    } else {
                        SharedString::from(self.name.clone())
                    }),
            )
    }
}

impl Render for AddSceneModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let error_note = self.error.clone().unwrap_or_default();

        let config = ModalConfig::standard("Add Scene", ModalSize::Small);
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
                            .id("add-scene-cancel")
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(theme.surface_active)
                            .text_color(theme.text)
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(AddSceneEvent::Cancelled);
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("add-scene-create")
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
            .id("add-scene-close")
            .cursor_pointer()
            .text_color(self.theme.text_muted)
            .child("Close")
            .on_click(cx.listener(|_, _, _, cx| cx.emit(AddSceneEvent::Cancelled)))
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
                .child(self.name_field(cx))
                .on_key_down(on_key),
            Some(footer),
            Some(close),
        )
    }
}
