//! Functional quick-action modal for the native shell, replacing the Svelte
//! `QuickTextEditModal` / `QuickColorModal` / `QuickDeviceModal` trio:
//! per-type editing (text, hex color with recents, image path, device pick
//! from the typed backend catalog) that applies through
//! `BackendCommand::UpdateSource`, plus the browser-interaction hand-off.

use gpui::{
    div, prelude::*, px, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    MouseButton, Render, SharedString, Window,
};

use crate::{
    actions::{ColorField, ComponentEvent, QuickSourceAction},
    components::modal_shell::{modal_shell_with_body, ModalConfig},
    components::text_field::apply_line_keystroke,
    theme::RevoTheme,
};

pub const QUICK_COLOR_HISTORY_LIMIT: usize = 5;

#[derive(Clone, Debug, PartialEq)]
pub enum QuickActionEvent {
    /// The validated draft ready for `UpdateSource`.
    Applied {
        id: String,
        action: QuickSourceAction,
        value: String,
    },
    /// Browser sources hand off to the interaction window instead.
    Interact {
        id: String,
    },
    Cancelled,
}

pub struct QuickActionModal {
    pub theme: RevoTheme,
    pub source_id: String,
    pub source_name: String,
    pub action: QuickSourceAction,
    pub value: String,
    pub recent_colors: Vec<String>,
    /// `(device id, display label)` pairs for the "Select device" picker.
    pub device_options: Vec<(String, String)>,
    pub error: Option<String>,
    field_focus: FocusHandle,
    field_focused: bool,
}

impl EventEmitter<QuickActionEvent> for QuickActionModal {}

// Color-picker requests flow through the shared ComponentEvent vocabulary;
// the Workspace opens the popup and routes the pick back to `apply_color`.
impl EventEmitter<ComponentEvent> for QuickActionModal {}

impl Focusable for QuickActionModal {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.field_focus.clone()
    }
}

/// Backend settings key for one quick action.
pub fn param_key(action: QuickSourceAction) -> Option<&'static str> {
    match action {
        QuickSourceAction::EditText => Some("text"),
        QuickSourceAction::ChangeColor => Some("color"),
        QuickSourceAction::SelectFile => Some("file"),
        QuickSourceAction::Interact | QuickSourceAction::SelectDevice => None,
    }
}

/// Normalizes color input to `#RRGGBB` (Svelte `normalizeQuickHexColor`
/// parity: optional `0x`/`#` prefix, non-hex characters dropped, longer
/// values keep the trailing six digits).
pub fn normalize_hex_color(value: &str) -> Option<String> {
    let trimmed = value
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches('#');
    // The input must be pure hex after the prefix; filtering stray hex
    // characters out of arbitrary text would accept garbage like "nope".
    if trimmed.is_empty() || !trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    let cleaned = trimmed.to_string();
    let tail: String = if cleaned.len() > 6 {
        cleaned[cleaned.len() - 6..].to_string()
    } else {
        format!("{:0>6}", cleaned)
    };
    Some(format!("#{}", tail.to_ascii_uppercase()))
}

/// Session-scoped recent-color history: deduplicated, newest first, capped.
pub fn push_recent_color(recent: &mut Vec<String>, color: String) {
    recent.retain(|entry| entry != &color);
    recent.insert(0, color);
    recent.truncate(QUICK_COLOR_HISTORY_LIMIT);
}

/// Device selection draft (Svelte `quickDeviceValue` parity): the stored
/// device wins, otherwise the first catalog entry, otherwise empty.
pub fn device_draft_value(current: &str, options: &[(String, String)]) -> String {
    if !current.is_empty() {
        return current.to_owned();
    }
    options
        .first()
        .map(|(value, _)| value.clone())
        .unwrap_or_default()
}

impl QuickActionModal {
    pub fn new(
        source_id: String,
        source_name: String,
        action: QuickSourceAction,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            theme: RevoTheme::future(),
            source_id,
            source_name,
            action,
            value: String::new(),
            recent_colors: Vec::new(),
            device_options: Vec::new(),
            error: None,
            field_focus: cx.focus_handle(),
            field_focused: false,
        }
    }

    /// Prefills the draft with the source's current value (Svelte parity:
    /// Edit text opens with the existing text, Change color with the
    /// current color; the color also seeds the recents).
    pub fn prefill(&mut self, current: String, cx: &mut Context<Self>) {
        if current.is_empty() {
            return;
        }
        match self.action {
            QuickSourceAction::ChangeColor => {
                if let Some(hex) = normalize_hex_color(&current) {
                    self.value = hex.clone();
                    push_recent_color(&mut self.recent_colors, hex);
                }
            }
            QuickSourceAction::EditText | QuickSourceAction::SelectFile => {
                self.value = current;
            }
            _ => {}
        }
        self.error = None;
        cx.notify();
    }

    /// Prefills the device picker with the backend catalog plus the source's
    /// current selection (Svelte parity: a stored device the catalog lacks
    /// is inserted as the first option so the current state is visible).
    pub fn prefill_devices(
        &mut self,
        current: String,
        mut options: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        if !current.is_empty() && !options.iter().any(|(value, _)| value == &current) {
            options.insert(0, (current.clone(), format!("{current} (current)")));
        }
        self.value = device_draft_value(&current, &options);
        self.device_options = options;
        self.error = None;
        cx.notify();
    }

    fn title(&self) -> &'static str {
        match self.action {
            QuickSourceAction::Interact => "Interact",
            QuickSourceAction::EditText => "Edit text",
            QuickSourceAction::ChangeColor => "Change color",
            QuickSourceAction::SelectFile => "Select image",
            QuickSourceAction::SelectDevice => "Select device",
        }
    }

    /// Validates and emits the draft; colors normalize first.
    fn confirm(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        match self.action {
            QuickSourceAction::ChangeColor => match normalize_hex_color(&self.value) {
                Some(color) => {
                    self.push_recent(color.clone());
                    cx.emit(QuickActionEvent::Applied {
                        id: self.source_id.clone(),
                        action: self.action,
                        value: color,
                    });
                }
                None => {
                    self.error = Some("Enter a hex color like #285CFC".into());
                }
            },
            QuickSourceAction::EditText => {
                if self.value.trim().is_empty() {
                    self.error = Some("Enter the text first".into());
                } else {
                    cx.emit(QuickActionEvent::Applied {
                        id: self.source_id.clone(),
                        action: self.action,
                        value: self.value.clone(),
                    });
                }
            }
            QuickSourceAction::SelectFile => {
                if self.value.trim().is_empty() {
                    self.error = Some("Enter the image file path".into());
                } else {
                    cx.emit(QuickActionEvent::Applied {
                        id: self.source_id.clone(),
                        action: self.action,
                        value: self.value.trim().to_owned(),
                    });
                }
            }
            QuickSourceAction::Interact => {
                cx.emit(QuickActionEvent::Interact {
                    id: self.source_id.clone(),
                });
            }
            QuickSourceAction::SelectDevice => {
                if self.device_options.is_empty() {
                    self.error = Some("No devices are available for this source type".into());
                } else if self.value.trim().is_empty() {
                    self.error = Some("Pick a device first".into());
                } else {
                    cx.emit(QuickActionEvent::Applied {
                        id: self.source_id.clone(),
                        action: self.action,
                        value: self.value.trim().to_owned(),
                    });
                }
            }
        }
        cx.notify();
    }

    fn push_recent(&mut self, color: String) {
        push_recent_color(&mut self.recent_colors, color);
    }

    /// Picker target: applies a confirmed popup color to the draft and the
    /// recent history; invalid values are ignored (the popup only emits
    /// normalized `#RRGGBB`).
    pub fn apply_color(&mut self, color: &str, cx: &mut Context<Self>) {
        if self.action != QuickSourceAction::ChangeColor {
            return;
        }
        if let Some(hex) = normalize_hex_color(color) {
            self.value = hex.clone();
            self.push_recent(hex);
            self.error = None;
            cx.notify();
        }
    }

    fn value_field(&mut self, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let focused = self.field_focused;
        let value = self.value.clone();
        div()
            .id("quick-action-field")
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
                cx.listener(|this, _, window, cx| {
                    this.field_focused = true;
                    this.field_focus.focus(window);
                    cx.notify();
                }),
            )
            .child(if value.is_empty() {
                SharedString::from(self.placeholder())
            } else {
                SharedString::from(value)
            })
    }

    fn placeholder(&self) -> &'static str {
        match self.action {
            QuickSourceAction::ChangeColor => "#285CFC",
            QuickSourceAction::SelectFile => "/path/to/image.png",
            _ => "…",
        }
    }

    fn on_field_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if !self.field_focused {
            return;
        }
        let keystroke = &event.keystroke;
        if let Some(next) = apply_line_keystroke(
            &self.value,
            &keystroke.key,
            keystroke.key_char.as_deref(),
            keystroke.modifiers.control,
            keystroke.modifiers.alt,
            keystroke.modifiers.platform,
        ) {
            self.value = next;
        }
        cx.notify();
    }
}

impl Render for QuickActionModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let config = ModalConfig {
            show_close: false,
            ..ModalConfig::quick(self.title())
        };
        let theme = self.theme;
        let error_note = self.error.clone().unwrap_or_default();

        let mut body = div().flex().flex_col().gap_2();
        body = body.child(
            div()
                .text_color(theme.text_secondary)
                .text_size(px(11.))
                .child(SharedString::from(self.source_name.clone())),
        );
        match self.action {
            QuickSourceAction::Interact => {
                body = body.child(
                    div()
                        .text_color(theme.text_secondary)
                        .text_size(px(11.))
                        .child(
                            "Opens the browser interaction window for this source (mouse and "
                                .to_string()
                                + "keyboard are forwarded to the headless browser).",
                        ),
                );
            }
            QuickSourceAction::SelectDevice => {
                if self.device_options.is_empty() {
                    body = body.child(
                        div()
                            .text_color(theme.text_secondary)
                            .text_size(px(11.))
                            .child(
                                "No device catalog was reported for this source type; capture "
                                    .to_string()
                                    + "sources keep their current device.",
                            ),
                    );
                } else {
                    // Scrollable option list (max height with a visible
                    // scrollbar) — long catalogs stay reachable.
                    let mut list = div()
                        .id("quick-device-list")
                        .flex()
                        .flex_col()
                        .gap_1()
                        .w_full()
                        .max_h(px(220.))
                        .overflow_y_scroll()
                        .scrollbar_width(px(8.));
                    let options = self.device_options.clone();
                    let current = self.value.clone();
                    for (index, (value, label)) in options.into_iter().enumerate() {
                        let selected = value == current;
                        list = list.child(
                            div()
                                .id(SharedString::from(format!("quick-device-{index}")))
                                .cursor_pointer()
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .border_1()
                                .border_color(if selected {
                                    theme.accent
                                } else {
                                    theme.border_strong
                                })
                                .bg(if selected {
                                    theme.surface_selected
                                } else {
                                    theme.surface_elevated
                                })
                                .text_color(if selected {
                                    theme.text_strong
                                } else {
                                    theme.text
                                })
                                .text_size(px(11.5))
                                .child(SharedString::from(label))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.value = value.clone();
                                    this.error = None;
                                    cx.notify();
                                })),
                        );
                    }
                    body = body.child(list);
                }
            }
            _ => {
                let field = self.value_field(cx);
                if self.action == QuickSourceAction::ChangeColor {
                    // The picker button anchors the shared color popup; the
                    // hex field stays directly editable either way.
                    let source_id = self.source_id.clone();
                    let current = if self.value.is_empty() {
                        self.recent_colors.first().cloned().unwrap_or_default()
                    } else {
                        self.value.clone()
                    };
                    body = body.child(
                        div().flex().flex_row().gap_1().child(field).child(
                            div()
                                .id("quick-color-picker")
                                .cursor_pointer()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border_strong)
                                .bg(theme.surface_active)
                                .text_color(theme.text)
                                .text_size(px(11.))
                                .child("Pick…")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |_, event: &gpui::MouseDownEvent, _, cx| {
                                        cx.emit(ComponentEvent::OpenColorPicker {
                                            field: ColorField::QuickAction {
                                                source_id: source_id.clone(),
                                            },
                                            current: current.clone(),
                                            position: (
                                                f32::from(event.position.x),
                                                f32::from(event.position.y),
                                            ),
                                        });
                                    }),
                                ),
                        ),
                    );
                } else {
                    body = body.child(field);
                }
            }
        }
        if self.action == QuickSourceAction::ChangeColor && !self.recent_colors.is_empty() {
            let recent: Vec<gpui::AnyElement> = self
                .recent_colors
                .clone()
                .into_iter()
                .map(|color| {
                    let hex = color.trim_start_matches('#').to_string();
                    let swatch = u32::from_str_radix(&hex, 8)
                        .or_else(|_| u32::from_str_radix(&hex, 16))
                        .unwrap_or(0xFFFFFF);
                    div()
                        .id(SharedString::from(format!("quick-color-{color}")))
                        .cursor_pointer()
                        .w(px(22.))
                        .h(px(22.))
                        .rounded_sm()
                        .border_1()
                        .border_color(theme.border_strong)
                        .bg(gpui::rgb(swatch))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.value = color.clone();
                            cx.notify();
                        }))
                        .into_any_element()
                })
                .collect();
            body = body.child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .child(
                        div()
                            .text_color(theme.text_muted)
                            .text_size(px(10.))
                            .child("Recent"),
                    )
                    .children(recent),
            );
        }
        if !error_note.is_empty() {
            body = body.child(
                div()
                    .text_color(theme.danger)
                    .text_size(px(11.))
                    .child(SharedString::from(error_note)),
            );
        }

        let footer = div()
            .flex()
            .flex_row()
            .justify_end()
            .gap_2()
            .p_3()
            .child(
                div()
                    .id("quick-action-cancel")
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(theme.surface_active)
                    .text_color(theme.text)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(QuickActionEvent::Cancelled)))
                    .child("Cancel"),
            )
            .child(
                div()
                    .id("quick-action-apply")
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(theme.accent)
                    .text_color(theme.text_strong)
                    .on_click(cx.listener(|this, _, _, cx| this.confirm(cx)))
                    .child(if self.action == QuickSourceAction::Interact {
                        "Open"
                    } else {
                        "Apply"
                    }),
            )
            .into_any_element();

        let on_key = cx.listener(|this, event: &KeyDownEvent, _, cx| {
            this.on_field_key(event, cx);
        });

        modal_shell_with_body(
            &config,
            theme,
            div()
                .flex()
                .flex_col()
                .gap_2()
                .size_full()
                .track_focus(&self.field_focus)
                .child(body)
                .on_key_down(on_key),
            Some(footer),
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colors_normalize_to_uppercase_rgb() {
        assert_eq!(normalize_hex_color(" 285cfc ").as_deref(), Some("#285CFC"));
        assert_eq!(normalize_hex_color("#FFAA00").as_deref(), Some("#FFAA00"));
        // Longer values keep the trailing six digits (Svelte parity; an
        // OBS ARGB value therefore keeps its RGB part).
        assert_eq!(normalize_hex_color("#FFAA00FF").as_deref(), Some("#AA00FF"));
        assert_eq!(normalize_hex_color("0x20e438").as_deref(), Some("#20E438"));
        assert_eq!(normalize_hex_color("nope"), None);
        assert_eq!(normalize_hex_color("f").as_deref(), Some("#00000F"));
    }

    #[test]
    fn param_keys_match_the_obs_source_settings() {
        assert_eq!(param_key(QuickSourceAction::EditText), Some("text"));
        assert_eq!(param_key(QuickSourceAction::ChangeColor), Some("color"));
        assert_eq!(param_key(QuickSourceAction::SelectFile), Some("file"));
        assert_eq!(param_key(QuickSourceAction::Interact), None);
    }

    #[test]
    fn device_draft_prefers_the_current_device_then_the_first_option() {
        let options = vec![
            ("/dev/video0".to_string(), "Cam 0".to_string()),
            ("/dev/video1".to_string(), "Cam 1".to_string()),
        ];
        assert_eq!(device_draft_value("/dev/video1", &options), "/dev/video1");
        assert_eq!(device_draft_value("", &options), "/dev/video0");
        assert_eq!(device_draft_value("", &[]), "");
    }

    #[test]
    fn recent_colors_deduplicate_and_cap() {
        let mut recent: Vec<String> = Vec::new();
        for color in [
            "#111111", "#222222", "#111111", "#333333", "#444444", "#555555", "#666666",
        ] {
            push_recent_color(&mut recent, color.into());
        }
        assert_eq!(
            recent,
            vec!["#666666", "#555555", "#444444", "#333333", "#111111"]
        );
    }
}
