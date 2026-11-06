use gpui::{
    div, prelude::*, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    MouseButton, Render, SharedString, Window,
};

use crate::{
    components::modal::settings_state::{
        OutputTab, SettingsDraft, SettingsEvent, SettingsSection, TextField,
    },
    components::modal_shell::{modal_shell_with_body, ModalConfig, ModalSize},
    theme::RevoTheme,
};

/// Functional native Settings modal: a section rail, live draft fields, and
/// an interactive Save/Cancel footer. The draft state machine lives in
/// [`SettingsDraft`]; this view owns focus and routes keystrokes into it.
pub struct SettingsModal {
    pub draft: SettingsDraft,
    pub theme: RevoTheme,
    field_focus: FocusHandle,
    focused_field: Option<TextField>,
}

impl EventEmitter<SettingsEvent> for SettingsModal {}

impl Focusable for SettingsModal {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.field_focus.clone()
    }
}

const SECTION_LABELS: [(SettingsSection, &str); 9] = [
    (SettingsSection::General, "General"),
    (SettingsSection::Look, "Look"),
    (SettingsSection::Broadcast, "Broadcast"),
    (SettingsSection::Output, "Output"),
    (SettingsSection::Audio, "Audio"),
    (SettingsSection::Video, "Video"),
    (SettingsSection::Shortcuts, "Shortcuts"),
    (SettingsSection::Accessibility, "Accessibility"),
    (SettingsSection::Other, "Other"),
];

impl SettingsModal {
    pub fn new(settings: revo_core::models::AppSettings, cx: &mut Context<Self>) -> Self {
        Self {
            draft: SettingsDraft::load(settings),
            theme: RevoTheme::future(),
            field_focus: cx.focus_handle(),
            focused_field: None,
        }
    }

    fn field_row(
        &self,
        label: &'static str,
        value: &str,
        field: TextField,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let focused = self.focused_field == Some(field);
        let focus = self.field_focus.clone();
        let value = SharedString::from(value.to_string());
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_color(self.theme.text_secondary)
                    .text_size(gpui::px(12.))
                    .child(label),
            )
            .child(
                div()
                    .id(SharedString::from(format!("settings-field-{label}")))
                    .cursor_pointer()
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
                            this.focused_field = Some(field);
                            focus.focus(window);
                            cx.notify();
                        }),
                    )
                    .child(if value.is_empty() {
                        SharedString::from("…")
                    } else {
                        value
                    }),
            )
    }

    fn toggle_row(
        &self,
        label: &'static str,
        enabled: bool,
        id: &'static str,
        toggle: impl Fn(&mut SettingsDraft) + Copy + 'static,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .py_1()
            .child(
                div()
                    .text_color(self.theme.text)
                    .child(SharedString::from(label)),
            )
            .child(
                div()
                    .id(SharedString::from(id))
                    .cursor_pointer()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(if enabled {
                        self.theme.accent
                    } else {
                        self.theme.surface_active
                    })
                    .text_color(if enabled {
                        self.theme.text_strong
                    } else {
                        self.theme.text_muted
                    })
                    .child(if enabled { "On" } else { "Off" })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        toggle(&mut this.draft);
                        cx.notify();
                    })),
            )
    }

    fn cycle_row(
        &self,
        label: &'static str,
        value: &str,
        id: &'static str,
        cycle: impl Fn(&mut SettingsDraft) + Copy + 'static,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .py_1()
            .child(
                div()
                    .text_color(self.theme.text)
                    .child(SharedString::from(label)),
            )
            .child(
                div()
                    .id(SharedString::from(id))
                    .cursor_pointer()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(self.theme.border_strong)
                    .bg(self.theme.surface_active)
                    .text_color(self.theme.text)
                    .child(SharedString::from(value.to_string()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cycle(&mut this.draft);
                        cx.notify();
                    })),
            )
    }

    fn section_body(&self, cx: &mut Context<Self>) -> gpui::Div {
        let settings = self.draft.settings();
        let mut body = div().flex().flex_col().gap_2();
        match self.draft.active_section {
            SettingsSection::General => {
                body = body
                    .child(self.toggle_row(
                        "Auto retry preview",
                        self.draft.bool_value(|s| s.auto_retry_preview),
                        "toggle-auto-retry",
                        SettingsDraft::toggle_auto_retry_preview,
                        cx,
                    ))
                    .child(self.toggle_row(
                        "Auto rescale inputs",
                        self.draft.bool_value(|s| s.autorescale_inputs),
                        "toggle-autorescale",
                        SettingsDraft::toggle_autorescale_inputs,
                        cx,
                    ))
                    .child(self.toggle_row(
                        "Minimize to tray",
                        self.draft.bool_value(|s| s.minimize_to_tray),
                        "toggle-minimize-tray",
                        SettingsDraft::toggle_minimize_to_tray,
                        cx,
                    ))
                    .child(self.toggle_row(
                        "Built-in browser",
                        self.draft.bool_value(|s| s.built_in_browser_enabled),
                        "toggle-browser",
                        SettingsDraft::toggle_built_in_browser,
                        cx,
                    ));
            }
            SettingsSection::Look | SettingsSection::Shortcuts | SettingsSection::Accessibility => {
                // These sections persist through the ui_profile document, which
                // has no flat fields yet; they render read-only until the
                // profile editor migrates.
                body = body.child(div().text_color(self.theme.text_muted).child(
                    SharedString::from(format!(
                        "Stored in the UI profile document ({})",
                        settings.active_profile.as_deref().unwrap_or("default")
                    )),
                ));
            }
            SettingsSection::Broadcast => {
                body = body
                    .child(self.field_row(
                        "Stream URL",
                        &settings.stream_url,
                        TextField::StreamUrl,
                        cx,
                    ))
                    .child(self.field_row(
                        "Stream key",
                        &settings.stream_key,
                        TextField::StreamKey,
                        cx,
                    ));
            }
            SettingsSection::Output => {
                body = body
                    .child(self.field_row(
                        "Recording path",
                        &settings.record_path,
                        TextField::RecordPath,
                        cx,
                    ))
                    .child(self.cycle_row(
                        "Encoder preference",
                        settings.encoder_preference.as_deref().unwrap_or("hardware"),
                        "cycle-encoder",
                        SettingsDraft::cycle_encoder_preference,
                        cx,
                    ));
            }
            SettingsSection::Audio => {
                body = body
                    .child(self.toggle_row(
                        "VST plugins",
                        self.draft.bool_value(|s| s.obs_vst_enabled),
                        "toggle-vst",
                        SettingsDraft::toggle_obs_vst,
                        cx,
                    ))
                    .child(self.toggle_row(
                        "Websocket control",
                        self.draft.bool_value(|s| s.obs_websocket_enabled),
                        "toggle-websocket",
                        SettingsDraft::toggle_obs_websocket,
                        cx,
                    ));
            }
            SettingsSection::Video => {
                body = body
                    .child(self.field_row(
                        "Scene resolution",
                        settings.scene_resolution.as_deref().unwrap_or(""),
                        TextField::SceneResolution,
                        cx,
                    ))
                    .child(self.cycle_row(
                        "Preview quality",
                        settings.preview_quality.as_deref().unwrap_or("medium"),
                        "cycle-quality",
                        SettingsDraft::cycle_preview_quality,
                        cx,
                    ));
            }
            SettingsSection::Other => {
                body = body
                    .child(self.field_row(
                        "OBS plugins path",
                        settings.obs_plugins_path.as_deref().unwrap_or(""),
                        TextField::ObsPluginsPath,
                        cx,
                    ))
                    .child(self.field_row(
                        "Websocket password",
                        settings.obs_websocket_password.as_deref().unwrap_or(""),
                        TextField::ObsWebsocketPassword,
                        cx,
                    ))
                    .child(self.field_row(
                        "Active profile",
                        settings.active_profile.as_deref().unwrap_or("default"),
                        TextField::ActiveProfile,
                        cx,
                    ));
            }
        }
        body
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        match self.draft.commit() {
            Ok(settings) => cx.emit(SettingsEvent::Saved(settings)),
            Err(_) => cx.notify(),
        }
    }
}

impl Render for SettingsModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let mut rail = div().flex().flex_col().gap_1().w(gpui::px(160.));
        for (section, label) in SECTION_LABELS {
            let active = self.draft.active_section == section;
            rail = rail.child(
                div()
                    .id(SharedString::from(format!("settings-section-{label}")))
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(if active {
                        self.theme.surface_active
                    } else {
                        self.theme.surface
                    })
                    .text_color(if active {
                        self.theme.text_strong
                    } else {
                        self.theme.text_muted
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.draft.select_section(section);
                        this.focused_field = None;
                        cx.notify();
                    }))
                    .child(SharedString::from(label)),
            );
        }

        if self.draft.active_section == SettingsSection::Output {
            rail = rail.child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        div()
                            .id("output-tab-streaming")
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .text_color(if self.draft.output_tab == OutputTab::Streaming {
                                self.theme.text_strong
                            } else {
                                self.theme.text_muted
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.draft.select_output_tab(OutputTab::Streaming);
                                cx.notify();
                            }))
                            .child("Streaming"),
                    )
                    .child(
                        div()
                            .id("output-tab-recording")
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .text_color(if self.draft.output_tab == OutputTab::Recording {
                                self.theme.text_strong
                            } else {
                                self.theme.text_muted
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.draft.select_output_tab(OutputTab::Recording);
                                cx.notify();
                            }))
                            .child("Recording"),
                    )
                    .child(
                        div()
                            .id("output-tab-audio")
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .text_color(if self.draft.output_tab == OutputTab::Audio {
                                self.theme.text_strong
                            } else {
                                self.theme.text_muted
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.draft.select_output_tab(OutputTab::Audio);
                                cx.notify();
                            }))
                            .child("Audio"),
                    ),
            );
        }

        let dirty_note = if self.draft.is_dirty() {
            SharedString::from("Unsaved changes")
        } else {
            SharedString::from("Saved")
        };
        let error_note = self
            .draft
            .field_error
            .clone()
            .map(SharedString::from)
            .unwrap_or_else(|| SharedString::from(""));

        let config = ModalConfig::standard("Settings", ModalSize::Workspace);
        let theme = self.theme;
        let footer = div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .p_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_color(if self.draft.is_dirty() {
                                theme.warning
                            } else {
                                theme.text_muted
                            })
                            .child(dirty_note),
                    )
                    .child(div().text_color(theme.danger).child(error_note)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id("settings-reset")
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(theme.surface_active)
                            .text_color(theme.text_muted)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.draft.reset_defaults();
                                cx.notify();
                            }))
                            .child("Reset defaults"),
                    )
                    .child(
                        div()
                            .id("settings-cancel")
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(theme.surface_active)
                            .text_color(theme.text)
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(SettingsEvent::Cancelled);
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("settings-save")
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(theme.accent)
                            .text_color(theme.text_strong)
                            .on_click(cx.listener(|this, _, _, cx| this.confirm(cx)))
                            .child("Save"),
                    ),
            )
            .into_any_element();

        let on_key = cx.listener(|this, event: &KeyDownEvent, _, cx| {
            if let Some(field) = this.focused_field {
                let keystroke = &event.keystroke;
                this.draft.field_keystroke(
                    field,
                    &keystroke.key,
                    keystroke.key_char.as_deref(),
                    keystroke.modifiers.control,
                    keystroke.modifiers.alt,
                    keystroke.modifiers.platform,
                );
                cx.notify();
            }
        });

        modal_shell_with_body(
            &config,
            self.theme,
            div()
                .flex()
                .flex_row()
                .gap_3()
                .size_full()
                .child(rail)
                .child(self.section_body(cx))
                .on_key_down(on_key),
            Some(footer),
        )
    }
}
