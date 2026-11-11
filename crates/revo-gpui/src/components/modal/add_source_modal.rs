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
    actions::{ColorField, ComponentEvent, FontField},
    components::editable_text::CaretField,
    components::modal_shell::{modal_shell_with_body, ModalConfig, ModalSize},
    components::naming::default_source_name,
    theme::RevoTheme,
};

/// Font size and left padding shared by the name and option value boxes.
const FIELD_FONT_PX: f32 = 13.;
const FIELD_PAD_PX: f32 = 8.;

#[derive(Clone, Debug, PartialEq)]
pub enum AddSourceEvent {
    /// The typed name, selected type and the basic per-type options.
    Created {
        name: String,
        source_type: String,
        options: std::collections::BTreeMap<String, String>,
    },
    Cancelled,
}

pub struct AddSourceModal {
    pub theme: RevoTheme,
    pub source_types: Vec<SourceTypeItem>,
    /// Existing source names; seeds the `source-{type}-{nr}` default.
    pub existing_names: Vec<String>,
    pub selected_type: Option<String>,
    pub name: String,
    /// Per-type basic option drafts (key → value) shown under the name
    /// field (Svelte "most important" options parity).
    pub option_values: std::collections::BTreeMap<String, String>,
    focused_option: Option<String>,
    field_focus: FocusHandle,
    field_focused: bool,
    error: Option<String>,
    name_caret: CaretField,
    option_carets: std::collections::BTreeMap<String, CaretField>,
}

impl EventEmitter<AddSourceEvent> for AddSourceModal {}

// Color/font picker requests flow through the shared ComponentEvent
// vocabulary; the Workspace opens the popup and routes the pick back to
// `set_option_value`.
impl EventEmitter<ComponentEvent> for AddSourceModal {}

impl Focusable for AddSourceModal {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.field_focus.clone()
    }
}

impl AddSourceModal {
    pub fn new(
        source_types: Vec<SourceTypeItem>,
        existing_names: Vec<String>,
        theme: RevoTheme,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            theme,
            source_types,
            existing_names,
            selected_type: None,
            name: String::new(),
            option_values: Default::default(),
            focused_option: None,
            field_focus: cx.focus_handle(),
            field_focused: false,
            error: None,
            name_caret: CaretField::default(),
            option_carets: Default::default(),
        }
    }

    /// Default name for the selected type: `source-{type}-{nr}` with the
    /// first free `{nr}`; `None` while no type is selected.
    pub fn default_name(&self) -> Option<String> {
        self.selected_type
            .as_deref()
            .map(|source_type| default_source_name(&self.existing_names, source_type))
    }

    /// Selects one source type tile; unknown ids are rejected so the modal
    /// never submits a type the engine cannot create.
    pub fn select(&mut self, source_type: &str) -> bool {
        if !self.source_types.iter().any(|item| item.id == source_type) {
            return false;
        }
        let changed = self.selected_type.as_deref() != Some(source_type);
        self.selected_type = Some(source_type.to_owned());
        if changed {
            self.sync_options();
        }
        true
    }

    /// The basic option rows for one source type: (key, label, placeholder).
    /// Text sources get the Svelte text/text-file mode selection plus color
    /// and font; captures get their device key.
    fn option_specs(source_type: &str) -> Vec<(&'static str, &'static str, &'static str)> {
        match source_type {
            "text_ft2_source" => vec![
                ("__mode", "Mode", "Text"),
                ("text", "Text", "Text to show…"),
                ("text_file", "Text file (UTF-8)", "/path/to/text.txt"),
                ("color", "Color", "#FFFFFF"),
                ("font_face", "Font", "Space Grotesk"),
                ("font_size", "Font size", "48"),
            ],
            "color_source" => vec![
                ("color", "Color", "#285CFC"),
                ("width", "Width", "1920"),
                ("height", "Height", "1080"),
            ],
            "image_source" | "slideshow" => {
                vec![("file", "Image file", "/path/to/image.png")]
            }
            "browser_source" => vec![
                ("url", "URL", "https://…"),
                ("width", "Width", "1280"),
                ("height", "Height", "720"),
            ],
            // Capture/other inputs: device selection stays a text key until
            // the device catalog lands in the snapshot.
            _ => vec![("device", "Device", "Default")],
        }
    }

    /// Re-seeds the option drafts when the selected type changes; text
    /// sources start in Text mode (the file field hidden).
    fn sync_options(&mut self) {
        let specs = self
            .selected_type
            .as_deref()
            .map(Self::option_specs)
            .unwrap_or_default();
        let mode = specs
            .iter()
            .find(|(key, _, _)| *key == "__mode")
            .map(|(_, _, default)| default.to_string());
        self.option_values = specs
            .into_iter()
            .map(|(key, _, default)| (key.to_string(), default.to_string()))
            .collect();
        self.focused_option = None;
        let _ = mode;
    }

    /// Which option keys are visible for the current selection: text
    /// sources show either the text field or the file field by mode.
    fn visible_option_keys(&self) -> Vec<String> {
        let mode = self
            .option_values
            .get("__mode")
            .cloned()
            .unwrap_or_else(|| "Text".into());
        self.option_values
            .keys()
            .filter(|key| match key.as_str() {
                "__mode" => false,
                "text" => mode == "Text",
                "text_file" => mode == "Text file (UTF-8)",
                _ => true,
            })
            .cloned()
            .collect()
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(source_type) = self.selected_type.clone() else {
            self.error = Some("Choose a source type first".into());
            cx.notify();
            return;
        };
        // An empty name falls back to the `source-{type}-{nr}` placeholder
        // shown in the field, so creating never blocks on naming.
        let name = match self.name.trim().is_empty() {
            true => self.default_name(),
            false => Some(self.name.trim().to_owned()),
        };
        let Some(name) = name else {
            self.error = Some("Choose a source type first".into());
            cx.notify();
            return;
        };
        let options: std::collections::BTreeMap<String, String> = self
            .option_values
            .iter()
            .filter(|(key, _)| key.as_str() != "__mode")
            .filter(|(key, _)| {
                self.visible_option_keys()
                    .iter()
                    .any(|visible| visible == *key)
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        cx.emit(AddSourceEvent::Created {
            name,
            source_type,
            options,
        });
    }

    /// Picker target: stores a confirmed color/font value into one option
    /// draft; unknown keys are ignored.
    pub fn set_option_value(&mut self, key: &str, value: &str, cx: &mut Context<Self>) {
        if !self.option_values.contains_key(key) {
            return;
        }
        self.option_values
            .insert(key.to_string(), value.to_string());
        cx.notify();
    }

    /// The per-type basic options section: text fields plus the text
    /// source's Text/Text-file mode cycle.
    fn options_section(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        if self.selected_type.is_none() {
            return div();
        }
        let mut section = div().flex().flex_col().gap_2();
        let Some(source_type) = self.selected_type.clone() else {
            return section;
        };
        let specs = Self::option_specs(&source_type);
        let visible = self.visible_option_keys();
        for (key, label, placeholder) in specs {
            if key == "__mode" {
                let mode = self
                    .option_values
                    .get("__mode")
                    .cloned()
                    .unwrap_or_else(|| "Text".into());
                section = section.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_color(self.theme.text_secondary)
                                .text_size(px(11.))
                                .child("Mode"),
                        )
                        .child(
                            div()
                                .id("add-source-text-mode")
                                .cursor_pointer()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(self.theme.surface_active)
                                .text_color(self.theme.text)
                                .text_size(px(11.2))
                                .child(SharedString::from(mode.clone()))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let next = if this
                                        .option_values
                                        .get("__mode")
                                        .map(|value| value == "Text")
                                        .unwrap_or(true)
                                    {
                                        "Text file (UTF-8)"
                                    } else {
                                        "Text"
                                    };
                                    this.option_values.insert("__mode".into(), next.into());
                                    cx.notify();
                                })),
                        ),
                );
                continue;
            }
            if !visible.iter().any(|visible_key| visible_key == key) {
                continue;
            }
            let value = self
                .option_values
                .get(key)
                .cloned()
                .unwrap_or_else(|| placeholder.to_string());
            let focused = self.focused_option.as_deref() == Some(key);
            let field_id: &'static str =
                Box::leak(format!("add-source-option-{key}").into_boxed_str());
            let key_owned = key.to_string();
            // Color and font options anchor the shared pickers; the field
            // itself stays a plain text draft either way.
            let picker_trigger = match key {
                "color" => {
                    let trigger_current = if value.is_empty() {
                        placeholder.to_string()
                    } else {
                        value.clone()
                    };
                    let trigger_key = key_owned.clone();
                    Some(("Pick…".to_string(), trigger_current, trigger_key))
                }
                "font_face" => {
                    let trigger_current = if value.is_empty() {
                        placeholder.to_string()
                    } else {
                        value.clone()
                    };
                    let trigger_key = key_owned.clone();
                    Some(("Fonts…".to_string(), trigger_current, trigger_key))
                }
                _ => None,
            };
            let option_field = div()
                .id(field_id)
                .cursor_pointer()
                .w_full()
                .px_2()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(if focused {
                    self.theme.accent
                } else {
                    self.theme.border_strong
                })
                .bg(self.theme.surface_elevated)
                .text_color(if value.is_empty() {
                    self.theme.text_muted
                } else {
                    self.theme.text
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        this.focused_option = Some(key_owned.clone());
                        this.field_focus.focus(window);
                        cx.notify();
                    }),
                )
                .child(if value.is_empty() {
                    SharedString::from(placeholder)
                } else {
                    SharedString::from(value)
                });
            let row = match picker_trigger {
                Some((trigger_label, trigger_current, trigger_key)) => {
                    div().flex().flex_row().gap_1().child(option_field).child(
                        div()
                            .id(SharedString::from(format!(
                                "add-source-picker-{}",
                                trigger_key.replace('_', "-")
                            )))
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(self.theme.border_strong)
                            .bg(self.theme.surface_active)
                            .text_color(self.theme.text)
                            .text_size(px(11.2))
                            .child(SharedString::from(trigger_label))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |_this, event: &gpui::MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    let event = match trigger_key.as_str() {
                                        "color" => ComponentEvent::OpenColorPicker {
                                            field: ColorField::AddSource {
                                                key: trigger_key.clone(),
                                            },
                                            current: trigger_current.clone(),
                                            position: (
                                                f32::from(event.position.x),
                                                f32::from(event.position.y),
                                            ),
                                        },
                                        _ => ComponentEvent::OpenFontPicker {
                                            field: FontField::AddSource,
                                            current: trigger_current.clone(),
                                            position: (
                                                f32::from(event.position.x),
                                                f32::from(event.position.y),
                                            ),
                                        },
                                    };
                                    cx.emit(event);
                                }),
                            ),
                    )
                }
                None => div().child(option_field),
            };
            section = section.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_color(self.theme.text_secondary)
                            .text_size(px(11.))
                            .child(SharedString::from(label)),
                    )
                    .child(row),
            );
        }
        section
    }

    fn name_field(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Div {
        self.name_caret.set_value(self.name.clone());
        let focused = self.field_focused;
        let theme = self.theme;
        let value_box = div()
            .id("add-source-field")
            .relative()
            .cursor_pointer()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(if focused {
                theme.accent
            } else {
                theme.border_strong
            })
            .bg(theme.surface_elevated)
            .text_color(if self.name.is_empty() {
                theme.text_muted
            } else {
                theme.text
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    this.field_focused = true;
                    this.name_caret.place_caret(
                        window,
                        event.position.x,
                        &this.name,
                        FIELD_FONT_PX,
                        FIELD_PAD_PX,
                    );
                    this.field_focus.focus(window);
                    cx.notify();
                }),
            );
        let value_box = if focused {
            value_box.child(self.name_caret.display(
                window,
                FIELD_FONT_PX,
                FIELD_PAD_PX,
                theme.text,
                theme.accent,
            ))
        } else {
            value_box.child(if self.name.is_empty() {
                SharedString::from(self.default_name().unwrap_or_else(|| "Source name…".into()))
            } else {
                SharedString::from(self.name.clone())
            })
        };
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
            .child(value_box.child(self.name_caret.bounds_probe()))
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
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
            // Option fields take priority when focused; otherwise the name.
            if let Some(option_key) = this.focused_option.clone() {
                let current = this
                    .option_values
                    .get(&option_key)
                    .cloned()
                    .unwrap_or_default();
                let changed = {
                    let caret = this.option_carets.entry(option_key.clone()).or_default();
                    caret.keystroke(event, &current)
                };
                if let Some(next) = changed {
                    this.option_values.insert(option_key, next);
                    cx.notify();
                    return;
                }
            }
            if this.field_focused {
                if let Some(next) = this.name_caret.keystroke(event, &this.name) {
                    this.name = next;
                }
                cx.notify();
            }
        });

        let close = crate::components::modal_shell::close_x_button("add-source-close", self.theme)
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
                .child(self.name_field(window, cx))
                .on_key_down(on_key),
            Some(footer),
            Some(close),
        )
    }
}
