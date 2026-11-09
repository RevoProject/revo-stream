//! Functional native Settings modal (Svelte SettingsModal parity, GPUI
//! layout): a vertical section rail with icons on the left, section content
//! on the right, and the Output sub-tabs (Streaming / Recording / Audio) at
//! the top of the content area. Fields are backed by the flat `AppSettings`
//! keys where they exist and by the `ui_profile` document (string map) for
//! the streaming/recording/audio/accessibility options, so everything
//! persists through the existing Save pipeline.

use gpui::{
    div, prelude::*, px, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, Render, SharedString, Window,
};

use crate::{
    components::modal::settings_state::{
        OutputTab, SettingsDraft, SettingsEvent, SettingsSection, TextField,
    },
    components::modal_shell::{modal_shell_with_body, ModalConfig, ModalSize},
    theme::RevoTheme,
};

/// Sections with their rail icons (Svelte settings parity: icons next to the
/// section names).
const SECTION_LABELS: [(SettingsSection, &str, &str); 9] = [
    (SettingsSection::General, "General", "settings"),
    (SettingsSection::Look, "Look", "layers"),
    (SettingsSection::Broadcast, "Broadcast", "media"),
    (SettingsSection::Output, "Output", "record"),
    (SettingsSection::Audio, "Audio", "volume"),
    (SettingsSection::Video, "Video", "video"),
    (SettingsSection::Shortcuts, "Shortcuts", "sliders"),
    (SettingsSection::Accessibility, "Accessibility", "user"),
    (SettingsSection::Other, "Other", "file-text"),
];

/// Streaming encoder profiles (Svelte parity).
const H264_PROFILES: [&str; 3] = ["baseline", "main", "high"];
/// Rate control options (CBR/VBR/CQP).
const RATE_CONTROLS: [&str; 3] = ["CBR", "VBR", "CQP"];
/// x264 CPU presets, fastest first (higher preset = lower CPU load).
const CPU_PRESETS: [&str; 6] = [
    "ultrafast",
    "superfast",
    "veryfast",
    "faster",
    "fast",
    "medium",
];
/// x264 tuning profiles.
const TUNES: [&str; 3] = ["none", "zerolatency", "film"];
/// Recording container formats (libobs-provided set, common values).
const RECORD_FORMATS: [&str; 4] = ["mp4", "mkv", "mov", "flv"];
/// Per-channel audio bitrates (kbps).
const AUDIO_BITRATES: [&str; 5] = ["128", "160", "192", "256", "320"];
/// Sample rates.
const SAMPLE_RATES: [&str; 2] = ["44100", "48000"];
/// Channel layouts.
const CHANNEL_LAYOUTS: [&str; 2] = ["Mono", "Stereo"];
/// Common FPS values (Svelte parity).
const COMMON_FPS: [&str; 8] = ["10", "20", "24", "29.97", "30", "48", "59.94", "60"];
/// FPS type modes.
const FPS_TYPES: [&str; 3] = ["Common", "Integer", "Fractional"];
/// Downscale filters.
const DOWNSCALE_FILTERS: [&str; 3] = ["Bilinear", "Area", "Lanczos"];
/// UI scale percentages.
const UI_SCALES: [&str; 5] = ["90", "100", "110", "125", "150"];
/// Font size options.
const FONT_SIZES: [&str; 4] = ["Small", "Default", "Large", "Extra large"];
/// Font families.
const FONT_FAMILIES: [&str; 4] = ["Default", "Sans-serif", "Serif", "Monospace"];
/// Color vision modes.
const COLOR_VISION_MODES: [&str; 4] = ["None", "Protanopia", "Deuteranopia", "Tritanopia"];
/// Planner undo/redo limits (1..=50 per the Svelte spec).
const PLANNER_UNDO_LIMITS: [&str; 5] = ["1", "5", "10", "25", "50"];
/// Broadcast service presets (services.json supplies the full list in the
/// Svelte build; the native shell ships the common ones).
const BROADCAST_SERVICES: [(&str, &str); 4] = [
    ("Twitch", "rtmp://live.twitch.tv/live"),
    ("YouTube", "rtmp://a.rtmp.youtube.com/live2"),
    ("Facebook", "rtmps://live-api-s.facebook.com:443/rtmp"),
    ("Custom", ""),
];

/// An open `ui_profile` select popup: the target key, the anchor position
/// (window coordinates) and the option list.
#[derive(Clone, Debug)]
struct DropdownState {
    key: &'static str,
    x: f32,
    y: f32,
    options: &'static [&'static str],
}

pub struct SettingsModal {
    pub draft: SettingsDraft,
    pub theme: RevoTheme,
    /// The currently expanded select popup (Svelte `<select>` parity).
    open_dropdown: Option<DropdownState>,
    /// Live OBS status line for the General section (Svelte parity:
    /// Waiting for Connect / Running OBS / OBS stopped).
    pub obs_status: String,
    field_focus: FocusHandle,
    focused_field: Option<TextField>,
    /// Whether the stream key is revealed (Svelte shows it for 5 s; the
    /// native shell keeps an explicit toggle).
    key_visible: bool,
}

impl EventEmitter<SettingsEvent> for SettingsModal {}

impl Focusable for SettingsModal {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.field_focus.clone()
    }
}

impl SettingsModal {
    pub fn new(settings: revo_core::models::AppSettings, cx: &mut Context<Self>) -> Self {
        Self::with_status(settings, "Waiting for Connect".into(), cx)
    }

    pub fn with_status(
        settings: revo_core::models::AppSettings,
        obs_status: String,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            draft: SettingsDraft::load(settings),
            theme: RevoTheme::future(),
            open_dropdown: None,
            obs_status,
            field_focus: cx.focus_handle(),
            focused_field: None,
            key_visible: false,
        }
    }

    fn field_row(
        &self,
        label: &str,
        value: &str,
        field: TextField,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let focused = self.focused_field == Some(field);
        div()
            .flex()
            .flex_col()
            .gap_1()
            .min_h(px(50.))
            .child(
                div()
                    .text_color(self.theme.text_secondary)
                    .text_size(px(13.))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .id(SharedString::from(format!(
                        "settings-field-{}",
                        self.field_id(field)
                    )))
                    .cursor_pointer()
                    .w_full()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .track_focus(&self.field_focus)
                    .border_1()
                    .border_color(if focused {
                        self.theme.accent
                    } else {
                        self.theme.border_strong
                    })
                    .bg(self.theme.surface_elevated)
                    .text_size(px(13.))
                    .text_color(if value.is_empty() {
                        self.theme.text_muted
                    } else {
                        self.theme.text
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.focused_field = Some(field);
                            this.field_focus.focus(window);
                            cx.notify();
                        }),
                    )
                    .child(if value.is_empty() {
                        SharedString::from("…")
                    } else {
                        SharedString::from(value.to_owned())
                    }),
            )
    }

    /// Stable element id for one field slot.
    fn field_id(&self, field: TextField) -> String {
        match field {
            TextField::StreamUrl => "stream-url".into(),
            TextField::StreamKey => "stream-key".into(),
            TextField::RecordPath => "record-path".into(),
            TextField::SceneResolution => "scene-resolution".into(),
            TextField::ObsPluginsPath => "obs-plugins-path".into(),
            TextField::ObsWebsocketPassword => "obs-websocket-password".into(),
            TextField::ActiveProfile => "active-profile".into(),
            TextField::RootDir => "root-dir".into(),
            TextField::ImportExportPath => "import-export-path".into(),
            TextField::ProfileKey(key) => format!("profile-{key}"),
        }
    }

    fn toggle_row(
        &self,
        label: &str,
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
            .min_h(px(30.))
            .child(
                div()
                    .text_color(self.theme.text)
                    .text_size(px(14.))
                    .child(SharedString::from(label.to_string())),
            )
            .child(
                div()
                    .id(id)
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
                    .text_size(px(13.))
                    .child(if enabled { "On" } else { "Off" })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        toggle(&mut this.draft);
                        cx.notify();
                    })),
            )
    }

    /// `ui_profile`-backed boolean row.
    fn profile_toggle_row(
        &self,
        label: &str,
        key: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let enabled = self.draft.profile_value(key).eq_ignore_ascii_case("true");
        self.toggle_row(
            label,
            enabled,
            boxed_id(format!("ptoggle-{key}")),
            |draft| {
                draft.toggle_profile(key);
            },
            cx,
        )
    }

    fn cycle_row(
        &self,
        label: &str,
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
            .min_h(px(32.))
            .child(
                div()
                    .text_color(self.theme.text)
                    .text_size(px(14.))
                    .child(SharedString::from(label.to_string())),
            )
            .child(
                div()
                    .id(id)
                    .cursor_pointer()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(self.theme.border_strong)
                    .bg(self.theme.surface_active)
                    .text_color(self.theme.text)
                    .text_size(px(13.))
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(SharedString::from(value.to_string()))
                    .child(crate::components::icon::icon(
                        self.theme.text_muted,
                        "chevron-down",
                        12.,
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cycle(&mut this.draft);
                        cx.notify();
                    })),
            )
    }

    /// Select-style row: clicking opens a real popup list anchored under the
    /// control (not a cycling button).
    fn dropdown_row(
        &self,
        label: &str,
        key: &'static str,
        options: &'static [&'static str],
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let current = self.draft.profile_value(key);
        let display: String = if current.is_empty() {
            options.first().copied().unwrap_or("").to_string()
        } else {
            current
        };
        div()
            .id(boxed_id(format!("pdropdown-{key}")))
            .cursor_pointer()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .min_h(px(32.))
            .child(
                div()
                    .text_color(self.theme.text)
                    .text_size(px(14.))
                    .child(SharedString::from(label.to_string())),
            )
            .child(
                div()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(self.theme.border_strong)
                    .bg(self.theme.surface_active)
                    .text_color(self.theme.text)
                    .text_size(px(13.))
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(SharedString::from(display.clone()))
                    .child(crate::components::icon::icon(
                        self.theme.text_muted,
                        "chevron-down",
                        12.,
                    )),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    let position = (f32::from(event.position.x), f32::from(event.position.y));
                    this.open_dropdown = Some(DropdownState {
                        key,
                        x: position.0,
                        y: position.1,
                        options,
                    });
                    cx.notify();
                }),
            )
    }

    /// The expanded select popup; rendered inside the modal shell so window
    /// coordinates line up.
    fn dropdown_popup(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let state = self.open_dropdown.clone()?;
        let selected = {
            let current = self.draft.profile_value(state.key);
            if current.is_empty() {
                state.options.first().copied().unwrap_or("").to_string()
            } else {
                current
            }
        };
        let mut popup = div()
            .id("settings-dropdown-popup")
            .absolute()
            .flex()
            .flex_col()
            .w(px(190.))
            .p_1()
            .gap_y(px(1.0))
            .bg(self.theme.surface_elevated)
            .border_1()
            .border_color(self.theme.border_strong)
            .rounded_md()
            .shadow_md()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            );
        for (index, option) in state.options.iter().enumerate() {
            popup = popup.child(
                div()
                    .id(SharedString::from(format!(
                        "settings-dropdown-option-{index}"
                    )))
                    .cursor_pointer()
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .rounded_sm()
                    .bg(if *option == selected {
                        self.theme.surface_selected
                    } else {
                        self.theme.surface_elevated
                    })
                    .text_color(if *option == selected {
                        self.theme.text_strong
                    } else {
                        self.theme.text
                    })
                    .text_size(px(13.))
                    .child(SharedString::from((*option).to_string()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.draft
                            .set_profile_value(state.key, (*option).to_string());
                        this.open_dropdown = None;
                        cx.notify();
                    })),
            );
        }
        Some(
            div()
                .absolute()
                .size_full()
                .top_0()
                .left_0()
                .child(
                    div()
                        .id("settings-dropdown-backdrop")
                        .absolute()
                        .size_full()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.open_dropdown = None;
                                cx.notify();
                            }),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(state.x - 40.0))
                        .top(px(state.y + 24.0))
                        .child(popup),
                )
                .into_any_element(),
        )
    }

    /// `ui_profile`-backed text row.
    fn profile_field_row(
        &self,
        label: &str,
        key: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        self.field_row(
            label,
            &self.draft.profile_value(key),
            TextField::ProfileKey(key),
            cx,
        )
    }

    /// Section heading inside the content area.
    fn group_label(&self, label: &str) -> gpui::Div {
        div()
            .text_color(self.theme.text_strong)
            .text_size(px(14.))
            .child(label.to_string())
    }

    /// Import/Export-style button that reports into the modal notice.
    fn action_row(
        &self,
        label: &str,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .cursor_pointer()
            .px_3()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(self.theme.border)
            .bg(self.theme.surface_active)
            .text_color(self.theme.text)
            .text_size(px(11.))
            .child(label.to_string())
            .on_click(cx.listener(|this, _, _, cx| {
                this.draft.notice =
                    Some("Scene/profile transfer needs the native file dialog — pending".into());
                cx.notify();
            }))
    }

    fn section_body(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        let settings = self.draft.settings();
        let mut body = div().flex().flex_col().gap_2();
        match self.draft.active_section {
            SettingsSection::General => {
                body = body
                    .child(self.group_label("Import / Export"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_color(self.theme.text_secondary)
                                    .text_size(px(12.))
                                    .child("Ścieżka pliku kolekcji scen:"),
                            )
                            .child(
                                div()
                                    .id("settings-import-export-path")
                                    .cursor_pointer()
                                    .w_full()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(
                                        if self.focused_field == Some(TextField::ImportExportPath) {
                                            self.theme.accent
                                        } else {
                                            self.theme.border_strong
                                        },
                                    )
                                    .bg(self.theme.surface_elevated)
                                    .text_color(if self.draft.import_export_path.is_empty() {
                                        self.theme.text_muted
                                    } else {
                                        self.theme.text
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, window, cx| {
                                            this.focused_field = Some(TextField::ImportExportPath);
                                            this.field_focus.focus(window);
                                            cx.notify();
                                        }),
                                    )
                                    .child(if self.draft.import_export_path.is_empty() {
                                        SharedString::from("/path/to/collection.json")
                                    } else {
                                        SharedString::from(self.draft.import_export_path.clone())
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_2()
                            .child(self.action_row(
                                "RevoStream — Export scenes",
                                "general-export-revo",
                                cx,
                            ))
                            .child(self.action_row(
                                "RevoStream — Import scenes",
                                "general-import-revo",
                                cx,
                            ))
                            .child(self.action_row(
                                "OBS Studio — Export as OBS scenes",
                                "general-export-obs",
                                cx,
                            ))
                            .child(self.action_row(
                                "OBS Studio — Import scenes",
                                "general-import-obs",
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(
                                div()
                                    .id("general-export-revo")
                                    .cursor_pointer()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(self.theme.border)
                                    .bg(self.theme.surface_active)
                                    .text_color(self.theme.text)
                                    .text_size(px(11.))
                                    .child("RevoStream — Export scenes")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let path = this.draft.import_export_path.clone();
                                        if path.trim().is_empty() {
                                            this.draft.notice = Some("Podaj ścieżkę pliku".into());
                                        } else {
                                            cx.emit(SettingsEvent::ExportRevoSceneCollection {
                                                path,
                                            });
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("general-import-revo")
                                    .cursor_pointer()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(self.theme.border)
                                    .bg(self.theme.surface_active)
                                    .text_color(self.theme.text)
                                    .text_size(px(11.))
                                    .child("RevoStream — Import scenes")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let path = this.draft.import_export_path.clone();
                                        if path.trim().is_empty() {
                                            this.draft.notice = Some("Podaj ścieżkę pliku".into());
                                        } else {
                                            cx.emit(SettingsEvent::ImportRevoSceneCollection {
                                                path,
                                            });
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("general-export-obs")
                                    .cursor_pointer()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(self.theme.border)
                                    .bg(self.theme.surface_active)
                                    .text_color(self.theme.text)
                                    .text_size(px(11.))
                                    .child("OBS Studio — Export as OBS scenes")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let path = this.draft.import_export_path.clone();
                                        if path.trim().is_empty() {
                                            this.draft.notice = Some("Podaj ścieżkę pliku".into());
                                        } else {
                                            cx.emit(SettingsEvent::ExportObsSceneCollection {
                                                path,
                                            });
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("general-import-obs")
                                    .cursor_pointer()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(self.theme.border)
                                    .bg(self.theme.surface_active)
                                    .text_color(self.theme.text)
                                    .text_size(px(11.))
                                    .child("OBS Studio — Import scenes")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let path = this.draft.import_export_path.clone();
                                        if path.trim().is_empty() {
                                            this.draft.notice = Some("Podaj ścieżkę pliku".into());
                                        } else {
                                            cx.emit(SettingsEvent::ImportObsSceneCollection {
                                                path,
                                            });
                                        }
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(self.group_label("Confirmation alerts"))
                    .child(self.profile_toggle_row(
                        "Show dialog when starting stream",
                        "confirm_start_stream",
                        cx,
                    ))
                    .child(self.profile_toggle_row(
                        "Show dialog when stopping stream",
                        "confirm_stop_stream",
                        cx,
                    ))
                    .child(self.profile_toggle_row(
                        "Show dialog when starting recording",
                        "confirm_start_record",
                        cx,
                    ))
                    .child(self.profile_toggle_row(
                        "Show dialog when stopping recording",
                        "confirm_stop_record",
                        cx,
                    ))
                    .child(self.group_label("Other functions"))
                    .child(self.toggle_row(
                        "Minimize to system tray",
                        self.draft.bool_value(|s| s.minimize_to_tray),
                        "toggle-minimize-tray",
                        SettingsDraft::toggle_minimize_to_tray,
                        cx,
                    ))
                    .child(self.profile_toggle_row(
                        "Automatically record when start streaming",
                        "auto_record_on_stream",
                        cx,
                    ))
                    .child(self.dropdown_row(
                        "Graphic planner undo/redo actions",
                        "planner_undo_limit",
                        &PLANNER_UNDO_LIMITS,
                        cx,
                    ))
                    .child(
                        div()
                            .text_color(self.theme.text_muted)
                            .text_size(px(11.))
                            .child(SharedString::from(format!("OBS: {}", self.obs_status))),
                    );
            }
            SettingsSection::Output => match self.draft.output_tab {
                OutputTab::Streaming => {
                    body = body
                        .child(self.group_label("Basic settings"))
                        .child(self.profile_field_row(
                            "Audio encoder",
                            "streaming_audio_encoder",
                            cx,
                        ))
                        .child(self.profile_field_row(
                            "Video encoder",
                            "streaming_video_encoder",
                            cx,
                        ))
                        .child(self.profile_field_row("Bitrate (kbps)", "streaming_bitrate", cx))
                        .child(self.profile_field_row(
                            "Keyframe interval (s)",
                            "streaming_keyframe_interval",
                            cx,
                        ))
                        .child(self.dropdown_row(
                            "Profile",
                            "streaming_profile",
                            &H264_PROFILES,
                            cx,
                        ))
                        .child(self.profile_toggle_row(
                            "Advanced options",
                            "streaming_advanced",
                            cx,
                        ))
                        .child(self.group_label("Advanced encoder settings"))
                        .child(self.dropdown_row(
                            "Rate Control",
                            "streaming_rate_control",
                            &RATE_CONTROLS,
                            cx,
                        ))
                        .child(self.dropdown_row(
                            "CPU Usage Preset",
                            "streaming_cpu_preset",
                            &CPU_PRESETS,
                            cx,
                        ))
                        .child(self.dropdown_row("Tune", "streaming_tune", &TUNES, cx))
                        .child(self.profile_field_row(
                            "Extra Options",
                            "streaming_extra_options",
                            cx,
                        ));
                }
                OutputTab::Recording => {
                    body = body
                        .child(self.profile_toggle_row(
                            "Record to filename without spaces",
                            "record_no_spaces",
                            cx,
                        ))
                        .child(self.field_row(
                            "Recording path",
                            &settings.record_path,
                            TextField::RecordPath,
                            cx,
                        ))
                        .child(self.dropdown_row(
                            "Recording format",
                            "record_format",
                            &RECORD_FORMATS,
                            cx,
                        ))
                        .child(self.profile_toggle_row("Advanced", "record_advanced", cx))
                        .child(self.profile_toggle_row("Custom Output", "record_custom_output", cx))
                        .child(self.group_label("Advanced encoder settings"))
                        .child(self.profile_field_row("Video Encoder", "record_video_encoder", cx))
                        .child(self.profile_field_row("Audio Encoder", "record_audio_encoder", cx))
                        .child(self.profile_field_row(
                            "Custom muxer Settings",
                            "record_muxer_settings",
                            cx,
                        ))
                        .child(self.group_label("Custom Output"))
                        .child(self.dropdown_row(
                            "Type Output",
                            "record_custom_type",
                            &["file", "url"],
                            cx,
                        ))
                        .child(self.profile_field_row("Path", "record_custom_path", cx))
                        .child(self.dropdown_row(
                            "Container",
                            "record_custom_container",
                            &RECORD_FORMATS,
                            cx,
                        ))
                        .child(self.profile_field_row(
                            "Video bitrate (kbps)",
                            "record_custom_video_bitrate",
                            cx,
                        ))
                        .child(self.profile_field_row(
                            "Video encoder",
                            "record_custom_video_encoder",
                            cx,
                        ))
                        .child(self.profile_field_row(
                            "Video encoder settings",
                            "record_custom_video_settings",
                            cx,
                        ))
                        .child(self.profile_field_row(
                            "Audio bitrate (kbps)",
                            "record_custom_audio_bitrate",
                            cx,
                        ))
                        .child(self.profile_field_row(
                            "Audio encoder",
                            "record_custom_audio_encoder",
                            cx,
                        ))
                        .child(self.profile_field_row(
                            "Audio encoder settings",
                            "record_custom_audio_settings",
                            cx,
                        ));
                }
                OutputTab::Audio => {
                    for channel in 1..=5u8 {
                        body = body
                            .child(self.group_label(&format!("Channel {channel}")))
                            .child(self.dropdown_row(
                                "Audio bitrate",
                                boxed_key(format!("audio_channel_{channel}_bitrate")),
                                &AUDIO_BITRATES,
                                cx,
                            ))
                            .child(self.profile_field_row(
                                "Name",
                                boxed_key(format!("audio_channel_{channel}_name")),
                                cx,
                            ));
                    }
                }
            },
            SettingsSection::Broadcast => {
                body = body.child(self.group_label("Autocomplete services")).child(
                    div().flex().flex_row().flex_wrap().gap_2().children(
                        BROADCAST_SERVICES
                            .iter()
                            .map(|(name, url)| {
                                let (name, url) = (*name, (*url).to_string());
                                div()
                                    .id(SharedString::from(format!("broadcast-service-{name}")))
                                    .cursor_pointer()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(self.theme.border)
                                    .bg(self.theme.surface_active)
                                    .text_color(self.theme.text)
                                    .text_size(px(11.))
                                    .child(name)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.draft
                                            .set_field_text(TextField::StreamUrl, url.clone());
                                        this.draft.notice =
                                            Some("Service URL set — review the key".into());
                                        cx.notify();
                                    }))
                            })
                            .collect::<Vec<_>>(),
                    ),
                );
                body = body
                    .child(
                        div()
                            .text_color(self.theme.text_muted)
                            .text_size(px(10.))
                            .child(
                                "Protocols: RTMP · RTMPS · SRT · RIST · WHIP · WebRTC".to_string(),
                            ),
                    )
                    .child(self.field_row("URL", &settings.stream_url, TextField::StreamUrl, cx));
                let key_row = div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .flex_1()
                    .child(self.field_row(
                        "Key",
                        if self.key_visible {
                            settings.stream_key.as_str()
                        } else {
                            "••••••••••••"
                        },
                        TextField::StreamKey,
                        cx,
                    ));
                body = body.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_end()
                        .gap_2()
                        .child(key_row)
                        .child(
                            div()
                                .id("broadcast-show-key")
                                .cursor_pointer()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .bg(self.theme.surface_active)
                                .text_color(self.theme.text)
                                .text_size(px(11.))
                                .child(if self.key_visible {
                                    "Hide key"
                                } else {
                                    "Show key"
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.key_visible = !this.key_visible;
                                    cx.notify();
                                })),
                        ),
                );
            }
            SettingsSection::Audio => {
                body = body
                    .child(self.group_label("Basic"))
                    .child(self.dropdown_row("Sample Rate", "audio_sample_rate", &SAMPLE_RATES, cx))
                    .child(self.dropdown_row("Channels", "audio_channels", &CHANNEL_LAYOUTS, cx))
                    .child(self.group_label("Global Audio Devices"))
                    .child(self.profile_field_row("Desktop Audio", "audio_desktop_device", cx))
                    .child(self.profile_field_row("Mic Audio", "audio_mic_device", cx))
                    .child(self.group_label("Monitoring device"))
                    .child(self.profile_field_row("Monitor device", "audio_monitor_device", cx))
                    .child(self.profile_toggle_row(
                        "Low latency mode (for NDI outputs)",
                        "audio_low_latency",
                        cx,
                    ));
            }
            SettingsSection::Video => {
                body = body
                    .child(self.group_label("Basic"))
                    .child(self.field_row(
                        "Base (Canvas) Resolution",
                        settings.scene_resolution.as_deref().unwrap_or(""),
                        TextField::SceneResolution,
                        cx,
                    ))
                    .child(self.profile_field_row(
                        "Output (Scaled) Resolution",
                        "video_output_resolution",
                        cx,
                    ))
                    .child(self.dropdown_row(
                        "Downscale Filter",
                        "video_downscale_filter",
                        &DOWNSCALE_FILTERS,
                        cx,
                    ))
                    .child(self.group_label("FPS"))
                    .child(self.dropdown_row("FPS Type", "video_fps_type", &FPS_TYPES, cx))
                    .child(self.dropdown_row(
                        "Common FPS Values",
                        "video_fps_common",
                        &COMMON_FPS,
                        cx,
                    ))
                    .child(self.profile_field_row("Custom FPS value", "video_fps_custom", cx))
                    .child(self.profile_field_row("Integer FPS value", "video_fps_integer", cx))
                    .child(self.profile_field_row(
                        "Fractional numerator",
                        "video_fps_fraction_numerator",
                        cx,
                    ))
                    .child(self.profile_field_row(
                        "Fractional denominator",
                        "video_fps_fraction_denominator",
                        cx,
                    ));
            }
            SettingsSection::Look => {
                body = body.child(self.group_label("Interface style")).child(
                    div().flex().flex_row().gap_2().children(
                        [("RevoClassic", "classic"), ("RevoFuture", "future")]
                            .iter()
                            .map(|(label, value)| {
                                let value = (*value).to_string();
                                let active = self
                                    .draft
                                    .profile_value("interface_style")
                                    .eq_ignore_ascii_case(&value);
                                div()
                                    .id(SharedString::from(format!("look-style-{label}")))
                                    .cursor_pointer()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(if active {
                                        self.theme.accent
                                    } else {
                                        self.theme.border
                                    })
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
                                    .text_size(px(11.))
                                    .child(*label)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.draft
                                            .set_profile_value("interface_style", value.clone());
                                        this.draft.notice = Some(
                                            "Style applies when the settings are saved".into(),
                                        );
                                        cx.notify();
                                    }))
                            })
                            .collect::<Vec<_>>(),
                    ),
                );
                body = body
                    .child(self.group_label("Theme selection"))
                    .child(
                        div()
                            .text_color(self.theme.text_muted)
                            .text_size(px(11.))
                            .child(
                                "Default theme — built-in; custom .revotheme import needs the "
                                    .to_string()
                                    + "native file dialog.",
                            ),
                    )
                    .child(self.action_row("Extract default theme", "look-extract-theme", cx));
            }
            SettingsSection::Shortcuts => {
                body = body.child(
                    div()
                        .text_color(self.theme.text_muted)
                        .text_size(px(10.))
                        .child(
                            "Optional — type combinations like Ctrl+Shift+S. Keyboard capture "
                                .to_string()
                                + "comes with the bindings engine.",
                        ),
                );
                for (label, key) in [
                    ("Start streaming", "shortcut_start_stream"),
                    ("Stop streaming", "shortcut_stop_stream"),
                    ("Start recording", "shortcut_start_record"),
                    ("Stop recording", "shortcut_stop_record"),
                    ("Pause recording", "shortcut_pause_record"),
                    ("Resume recording", "shortcut_resume_record"),
                    ("Show all sources", "shortcut_show_sources"),
                    ("Hide all sources", "shortcut_hide_sources"),
                    ("Show source transform window", "shortcut_show_transform"),
                    ("Hide source transform window", "shortcut_hide_transform"),
                    ("Next scene", "shortcut_next_scene"),
                    ("Previous scene", "shortcut_prev_scene"),
                    ("Show all scene items", "shortcut_show_scene_items"),
                    ("Hide all scene items", "shortcut_hide_scene_items"),
                ] {
                    body = body.child(self.profile_field_row(label, key, cx));
                }
            }
            SettingsSection::Accessibility => {
                body = body
                    .child(self.group_label("Visual accessibility"))
                    .child(self.profile_toggle_row(
                        "High contrast mode (WCAG support)",
                        "a11y_high_contrast",
                        cx,
                    ))
                    .child(self.profile_toggle_row(
                        "Reduce motion and transitions",
                        "a11y_reduce_motion",
                        cx,
                    ))
                    .child(self.profile_toggle_row(
                        "Stronger keyboard focus indicators",
                        "a11y_strong_focus",
                        cx,
                    ))
                    .child(self.group_label("Text and interface size"))
                    .child(self.dropdown_row("UI scale (%)", "a11y_ui_scale", &UI_SCALES, cx))
                    .child(self.field_row(
                        "Font size",
                        &self.draft.profile_value("a11y_font_size"),
                        TextField::ProfileKey("a11y_font_size"),
                        cx,
                    ))
                    .child(self.field_row(
                        "Font family",
                        &self.draft.profile_value("a11y_font_family"),
                        TextField::ProfileKey("a11y_font_family"),
                        cx,
                    ))
                    .child(self.dropdown_row(
                        "Color vision mode",
                        "a11y_color_vision",
                        &COLOR_VISION_MODES,
                        cx,
                    ));
            }
            SettingsSection::Other => {
                body = body
                    .child(self.group_label("Language"))
                    .child(self.dropdown_row(
                        "Select language",
                        "language",
                        &["English", "Polski", "Deutsch", "Español"],
                        cx,
                    ))
                    .child(self.group_label("Profile Import / Export"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_2()
                            .child(self.action_row(
                                "OBS Studio profile — Export profile",
                                "other-export-profile",
                                cx,
                            ))
                            .child(self.action_row("Import profile", "other-import-profile", cx)),
                    )
                    .child(self.group_label("Profiles"))
                    .child(self.field_row(
                        "Active profile",
                        settings.active_profile.as_deref().unwrap_or("default"),
                        TextField::ActiveProfile,
                        cx,
                    ))
                    .child(
                        div()
                            .text_color(self.theme.text_muted)
                            .text_size(px(11.))
                            .child("Profiles persist in data/profiles."),
                    )
                    .child(self.group_label("Paths and OBS integration"))
                    .child(self.field_row(
                        "Project root (optional)",
                        settings.root_dir.as_deref().unwrap_or(""),
                        TextField::RootDir,
                        cx,
                    ))
                    .child(self.field_row(
                        "OBS Plugins path (optional)",
                        settings.obs_plugins_path.as_deref().unwrap_or(""),
                        TextField::ObsPluginsPath,
                        cx,
                    ))
                    .child(self.toggle_row(
                        "Load obs-vst module (VST 2.x audio filter)",
                        self.draft.bool_value(|s| s.obs_vst_enabled),
                        "toggle-vst",
                        SettingsDraft::toggle_obs_vst,
                        cx,
                    ))
                    .child(self.toggle_row(
                        "Load obs-websocket module (remote control API)",
                        self.draft.bool_value(|s| s.obs_websocket_enabled),
                        "toggle-websocket",
                        SettingsDraft::toggle_obs_websocket,
                        cx,
                    ))
                    .child(self.field_row(
                        "obs-websocket password (port 4455, applies on OBS restart)",
                        settings.obs_websocket_password.as_deref().unwrap_or(""),
                        TextField::ObsWebsocketPassword,
                        cx,
                    ))
                    .child(self.group_label("Preview and encoding"))
                    .child(self.cycle_row(
                        "Preview quality",
                        settings.preview_quality.as_deref().unwrap_or("medium"),
                        "cycle-quality",
                        SettingsDraft::cycle_preview_quality,
                        cx,
                    ))
                    .child(self.cycle_row(
                        "Video encoder",
                        settings.encoder_preference.as_deref().unwrap_or("hardware"),
                        "cycle-encoder",
                        SettingsDraft::cycle_encoder_preference,
                        cx,
                    ))
                    .child(
                        div()
                            .text_color(self.theme.text_muted)
                            .text_size(px(10.))
                            .child(
                                "nvenc_legacy uses the legacy H.264 NVENC path (Maxwell/Optimus); "
                                    .to_string()
                                    + "falls back to x264 when unavailable.",
                            ),
                    )
                    .child(
                        self.toggle_row(
                            "DEMO mode",
                            self.draft
                                .profile_value("demo_mode")
                                .eq_ignore_ascii_case("true"),
                            boxed_id("other-demo-mode".into()),
                            |draft| draft.toggle_profile("demo_mode"),
                            cx,
                        ),
                    )
                    .child(self.toggle_row(
                        "Autorescale inputs on change scene resolution",
                        self.draft.bool_value(|s| s.autorescale_inputs),
                        "toggle-autorescale",
                        SettingsDraft::toggle_autorescale_inputs,
                        cx,
                    ))
                    .child(self.toggle_row(
                        "Auto-retry preview",
                        self.draft.bool_value(|s| s.auto_retry_preview),
                        "toggle-auto-retry",
                        SettingsDraft::toggle_auto_retry_preview,
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

/// Heap-leaked stable element id for `profile`-backed rows.
fn boxed_id(id: String) -> &'static str {
    Box::leak(id.into_boxed_str())
}

/// Heap-leaked stable profile key for channel rows (the loop needs &'static).
fn boxed_key(key: String) -> &'static str {
    Box::leak(key.into_boxed_str())
}

impl Render for SettingsModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        // Vertical section rail (Svelte parity: icons above the labels, list
        // runs top to bottom).
        let mut rail = div()
            .flex()
            .flex_col()
            .gap_1()
            .w(gpui::px(176.))
            .flex_none();
        for (section, label, icon) in SECTION_LABELS {
            let active = self.draft.active_section == section;
            rail = rail.child(
                div()
                    .id(SharedString::from(format!("settings-section-{label}")))
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
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
                    .child(crate::components::icon::icon(
                        if active {
                            self.theme.accent
                        } else {
                            self.theme.text_muted
                        },
                        icon,
                        13.,
                    ))
                    .child(SharedString::from(label))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.draft.select_section(section);
                        this.focused_field = None;
                        cx.notify();
                    })),
            );
        }

        // Output sub-tabs sit at the top of the content area (Svelte keeps
        // them beside the section names, not at the bottom).
        let mut content = div().flex().flex_col().gap_2().flex_1().min_w_0();
        if self.draft.active_section == SettingsSection::Output {
            let mut tabs = div().flex().flex_row().gap_1();
            for (tab, label) in [
                (OutputTab::Streaming, "Streaming"),
                (OutputTab::Recording, "Recording"),
                (OutputTab::Audio, "Audio"),
            ] {
                let active = self.draft.output_tab == tab;
                tabs = tabs.child(
                    div()
                        .id(SharedString::from(format!(
                            "output-tab-{}",
                            label.to_ascii_lowercase()
                        )))
                        .cursor_pointer()
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(if active {
                            self.theme.accent
                        } else {
                            self.theme.surface_active
                        })
                        .text_color(if active {
                            self.theme.text_strong
                        } else {
                            self.theme.text_muted
                        })
                        .text_size(px(11.))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.select_output_tab(tab);
                            this.focused_field = None;
                            cx.notify();
                        }))
                        .child(label),
                );
            }
            content = content.child(tabs);
        }
        if let Some(notice) = self.draft.notice.clone() {
            content = content.child(
                div()
                    .text_color(self.theme.warning)
                    .text_size(px(11.))
                    .child(SharedString::from(notice)),
            );
        }
        content = content.child(self.section_body(cx));

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
                                this.focused_field = None;
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

        let shell = modal_shell_with_body(
            &config,
            self.theme,
            div()
                .flex()
                .flex_row()
                .gap_3()
                .size_full()
                .track_focus(&self.field_focus)
                .child(rail)
                .child(content)
                .on_key_down(on_key),
            Some(footer),
            None,
        );
        match self.dropdown_popup(cx) {
            Some(popup) => div().relative().size_full().child(shell).child(popup),
            None => shell,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn settings_modal_field_keystrokes_update_draft(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let _ = window;
            SettingsModal::new(revo_core::settings::default_settings(), cx)
        });
        // Focus the field exactly the way the field_row click handler does.
        cx.update(|window, cx| {
            entity.update(cx, |modal, cx| {
                modal.focused_field = Some(TextField::StreamUrl);
                modal.field_focus.focus(window);
                cx.notify();
            });
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            entity.update(cx, |modal, cx| {
                let _ = cx;
                assert!(
                    modal.field_focus.is_focused(window),
                    "field focus should be set on the window"
                );
            });
        });
        cx.simulate_keystrokes("a b c");
        cx.run_until_parked();
        entity.update(cx, |modal, cx| {
            assert_eq!(
                modal.draft.settings().stream_url,
                "abc",
                "typed text should reach the draft"
            );
            cx.notify();
        });
    }
}
