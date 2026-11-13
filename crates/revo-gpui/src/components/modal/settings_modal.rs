//! Functional native Settings modal (Svelte SettingsModal parity, GPUI
//! layout): a vertical section rail with icons on the left, section content
//! on the right, and the Output sub-tabs (Streaming / Recording / Audio) at
//! the top of the content area. Fields are backed by the flat `AppSettings`
//! keys where they exist and by the `ui_profile` document (string map) for
//! the streaming/recording/audio/accessibility options, so everything
//! persists through the existing Save pipeline.

use gpui::{
    div, prelude::*, px, Bounds, Context, EventEmitter, FocusHandle, Focusable, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, PathPromptOptions, Pixels, Render, SharedString,
    Window,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::{
    components::editable_text::{
        caret_index_at_x, render_caret_value, CaretSession, EditModifiers,
    },
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
    ("Facebook", "rtmps://live-api-s.facebook.com:441/rtmp"),
    ("Custom", ""),
];

/// Languages with their translation completion (plan §11.1): only English
/// ships strings for now, so the rest advertise `(0%)` until i18n lands.
const LANGUAGE_VALUES: [&str; 4] = ["English", "Polski", "Deutsch", "Español"];
const LANGUAGE_LABELS: [&str; 4] = [
    "English (100%)",
    "Polski (0%)",
    "Deutsch (0%)",
    "Español (0%)",
];

/// Where a popup writes its choice: an `ui_profile` key or a typed draft
/// field (e.g. the base-resolution `TextField`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DropdownTarget {
    Profile(&'static str),
    Text(TextField),
}

/// An open select popup: the write target, the anchor position (window
/// coordinates) and the (stored value, displayed label) pairs. Static rows
/// use value == label; engine catalogs and language rows provide their own
/// labels (plan §6–8, §11.1).
#[derive(Clone, Debug)]
struct DropdownState {
    target: DropdownTarget,
    x: f32,
    y: f32,
    options: Vec<(String, String)>,
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
    /// Engine-published Settings catalog (plan §6–8); `None` falls back to
    /// the static lists/fields.
    pub select_options: Option<revo_core::service::SnapshotSelectOptions>,
    /// Caret/selection session per focused field key (plan §5.1).
    sessions: HashMap<String, CaretSession>,
    /// Per-field bounds probes used for click-to-caret mapping.
    field_bounds: HashMap<String, Rc<RefCell<Option<Bounds<Pixels>>>>>,
}

/// Font size and left padding the settings value boxes render at; caret
/// geometry must be measured with the same numbers.
const FIELD_FONT_PX: f32 = 13.;
const FIELD_PAD_PX: f32 = 8.;

impl EventEmitter<SettingsEvent> for SettingsModal {}

impl Focusable for SettingsModal {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.field_focus.clone()
    }
}

impl SettingsModal {
    pub fn new(
        settings: revo_core::models::AppSettings,
        theme: RevoTheme,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::with_status(settings, "Waiting for Connect".into(), theme, cx)
    }

    pub fn with_status(
        settings: revo_core::models::AppSettings,
        obs_status: String,
        theme: RevoTheme,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            draft: SettingsDraft::load(settings),
            theme,
            open_dropdown: None,
            obs_status,
            field_focus: cx.focus_handle(),
            focused_field: None,
            key_visible: false,
            select_options: None,
            sessions: HashMap::new(),
            field_bounds: HashMap::new(),
        }
    }

    /// Pushes the engine catalog (or clears it). The workspace calls this on
    /// open and on every snapshot apply so an open modal upgrades its
    /// dropdowns the moment OBS reports its modules (plan §6–8).
    pub fn set_select_options(
        &mut self,
        options: Option<revo_core::service::SnapshotSelectOptions>,
        cx: &mut Context<Self>,
    ) {
        if self.select_options != options {
            self.select_options = options;
            cx.notify();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn field_row(
        &mut self,
        window: &mut Window,
        label: &str,
        value: &str,
        field: TextField,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let focused = self.focused_field == Some(field);
        let id = self.field_id(field);
        if focused {
            // Keep the live draft value in the session (external edits such
            // as Reset defaults win; the caret is clamped, never dropped).
            let mut session = self
                .sessions
                .remove(&id)
                .unwrap_or_else(|| CaretSession::new(value.to_owned()));
            session.sync_value(value.to_owned());
            self.sessions.insert(id.clone(), session);
        }
        let bounds_cell = self
            .field_bounds
            .entry(id.clone())
            .or_insert_with(|| Rc::new(RefCell::new(None)))
            .clone();
        let value_box = div()
            .id(SharedString::from(format!(
                "settings-field-{}",
                self.field_id(field)
            )))
            .cursor_text()
            .w_full()
            .px_2()
            .py_1()
            .rounded_md()
            .relative()
            .track_focus(&self.field_focus)
            .border_1()
            .border_color(if focused {
                self.theme.accent
            } else {
                self.theme.border_strong
            })
            .bg(self.theme.surface_elevated)
            .text_size(px(FIELD_FONT_PX))
            .text_color(if value.is_empty() {
                self.theme.text_muted
            } else {
                self.theme.text
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    let was_focused = this.focused_field == Some(field);
                    this.focused_field = Some(field);
                    this.field_focus.focus(window);
                    let id = this.field_id(field);
                    let draft_value = this.draft.field_text(field);
                    let mut session = this
                        .sessions
                        .remove(&id)
                        .unwrap_or_else(|| CaretSession::new(draft_value.clone()));
                    session.sync_value(draft_value.clone());
                    if !was_focused && session.cursor > session.len() {
                        session.cursor = session.len();
                    }
                    // Browser-like click: 1 = caret/drag, 2 = word, 3 = all; Shift extends.
                    let bounds = this.field_bounds.get(&id).and_then(|c| *c.borrow());
                    let origin_x = bounds
                        .map(|b| f32::from(b.origin.x))
                        .unwrap_or(f32::MIN / 4.);
                    let local_x = f32::from(event.position.x) - origin_x - FIELD_PAD_PX;
                    let index =
                        caret_index_at_x(window, &session.value, local_x.max(0.), FIELD_FONT_PX);
                    let clicks = event.click_count;
                    let shift = event.modifiers.shift;
                    if shift && clicks <= 1 {
                        let anchor = session.anchor.unwrap_or(session.cursor);
                        session.anchor = Some(anchor);
                        session.cursor = index;
                    } else {
                        match clicks {
                            2 => {
                                let (s, e) = crate::components::editable_text::word_bounds(&session.value, index);
                                session.anchor = Some(s);
                                session.cursor = e;
                            }
                            3.. => {
                                session.anchor = Some(0);
                                session.cursor = session.len();
                            }
                            _ => {
                                session.anchor = None;
                                session.cursor = index;
                            }
                        }
                    }
                    this.sessions.insert(id, session);
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(move |this, event: &gpui::MouseMoveEvent, window, cx| {
                if !event.pressed_button.is_some_and(|b| b == MouseButton::Left) {
                    return;
                }
                if this.focused_field != Some(field) {
                    return;
                }
                let id = this.field_id(field);
                let bounds = this.field_bounds.get(&id).and_then(|c| *c.borrow());
                let Some(bounds) = bounds else { return };
                let local_x = f32::from(event.position.x) - f32::from(bounds.origin.x) - FIELD_PAD_PX;
                let mut session = match this.sessions.remove(&id) {
                    Some(s) => s,
                    None => return,
                };
                let index = caret_index_at_x(window, &session.value, local_x.max(0.), FIELD_FONT_PX);
                if session.anchor.is_none() {
                    session.anchor = Some(session.cursor);
                }
                session.cursor = index;
                this.sessions.insert(id, session);
                cx.notify();
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let id = this.field_id(field);
                    if let Some(session) = this.sessions.get_mut(&id) {
                        if session.selection_chars().is_none() {
                            session.anchor = None;
                        }
                    }
                    cx.notify();
                }),
            );
        let value_box = if focused {
            let session = self.sessions.get(&id).cloned().unwrap_or_default();
            let selection_bg = gpui::Rgba {
                a: 0.3,
                ..self.theme.accent
            };
            value_box.child(render_caret_value(
                window,
                &session,
                FIELD_FONT_PX,
                FIELD_PAD_PX,
                self.theme.text,
                self.theme.accent,
                selection_bg,
            ))
        } else if field == TextField::StreamKey && !self.key_visible {
            // Masked display only: the editing session always holds the real
            // key value.
            value_box.child(SharedString::from("••••••••••••"))
        } else {
            value_box.child(if value.is_empty() {
                SharedString::from("…")
            } else {
                SharedString::from(value.to_owned())
            })
        };
        div()
            .relative()
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
                value_box.child(
                    // Bounds probe: records where the value box actually painted
                    // so the next click can map pointer-x to a caret index.
                    gpui::canvas(
                        move |bounds, _, _| *bounds_cell.borrow_mut() = Some(bounds),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                ),
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

    /// The shared value box + popup opener used by every select row. The
    /// popup opens only from the box itself (the label is not a click trap,
    /// plan §3.2) and carries the full (value, label) option list.
    fn dropdown_box(
        &self,
        label: &str,
        id_key: &str,
        display: String,
        target: DropdownTarget,
        options: Vec<(String, String)>,
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
                    .id(boxed_id(id_key.to_string()))
                    .cursor_pointer()
                    .max_w(gpui::relative(0.5))
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
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .child(SharedString::from(display)),
                    )
                    .child(crate::components::icon::icon(
                        self.theme.text_muted,
                        "chevron-down",
                        12.,
                    ))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.open_dropdown = Some(DropdownState {
                                target,
                                x: f32::from(event.position.x),
                                y: f32::from(event.position.y),
                                options: options.clone(),
                            });
                            cx.notify();
                        }),
                    ),
            )
    }

    fn target_value(&self, target: DropdownTarget) -> String {
        match target {
            DropdownTarget::Profile(key) => self.draft.profile_value(key),
            DropdownTarget::Text(field) => self.draft.field_text(field),
        }
    }

    fn write_target(&mut self, target: DropdownTarget, value: String) {
        match target {
            DropdownTarget::Profile(key) => self.draft.set_profile_value(key, value),
            DropdownTarget::Text(field) => self.draft.set_field_text(field, value),
        }
    }

    /// Select row backed by a `ui_profile` key with a static value list.
    fn dropdown_row(
        &self,
        label: &str,
        key: &'static str,
        options: &'static [&'static str],
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let pairs: Vec<(String, String)> = options
            .iter()
            .map(|value| (value.to_string(), value.to_string()))
            .collect();
        self.dropdown_row_owned(label, key, pairs, cx)
    }

    /// Select row backed by a `ui_profile` key with (value, label) pairs
    /// from the engine catalog (plan §6–8) or language completion labels.
    fn dropdown_row_owned(
        &self,
        label: &str,
        key: &'static str,
        pairs: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let current = self.draft.profile_value(key);
        let display = pairs
            .iter()
            .find(|(value, _)| *value == current)
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| {
                if current.is_empty() {
                    pairs
                        .first()
                        .map(|(_, label)| label.clone())
                        .unwrap_or_default()
                } else {
                    current.clone()
                }
            });
        self.dropdown_box(
            label,
            &format!("pdropdown-{key}"),
            display,
            DropdownTarget::Profile(key),
            pairs,
            cx,
        )
    }

    /// The expanded select popup; rendered inside the modal shell so window
    /// coordinates line up.
    fn dropdown_popup(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let state = self.open_dropdown.clone()?;
        let selected = {
            let current = self.target_value(state.target);
            if current.is_empty() {
                state
                    .options
                    .first()
                    .map(|(value, _)| value.clone())
                    .unwrap_or_default()
            } else {
                current
            }
        };
        let mut popup = div()
            .id("settings-dropdown-popup")
            .absolute()
            .flex()
            .flex_col()
            .w(px(220.))
            .p_1()
            .gap_y(px(1.0))
            // Long option lists (profiles, encoders, services) stay
            // reachable with a visible scrollbar instead of overflowing.
            .max_h(px(260.))
            .overflow_y_scroll()
            .scrollbar_width(px(8.))
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
        for (index, (value, label)) in state.options.iter().enumerate() {
            let selected_row = *value == selected;
            let click_value = value.clone();
            let click_target = state.target;
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
                    .bg(if selected_row {
                        self.theme.surface_selected
                    } else {
                        self.theme.surface_elevated
                    })
                    .text_color(if selected_row {
                        self.theme.text_strong
                    } else {
                        self.theme.text
                    })
                    .text_size(px(13.))
                    .child(SharedString::from(label.clone()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.write_target(click_target, click_value.clone());
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

    /// Select row whose stored value differs from the displayed label
    /// (languages show their translation completion next to the name).
    fn dropdown_row_pairs(
        &self,
        label: &str,
        key: &'static str,
        options: &'static [&'static str],
        labels: &'static [&'static str],
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let pairs: Vec<(String, String)> = options
            .iter()
            .zip(labels.iter())
            .map(|(value, label)| (value.to_string(), label.to_string()))
            .collect();
        self.dropdown_row_owned(label, key, pairs, cx)
    }

    /// Engine-catalog pairs for one option list; empty when the engine has
    /// not published `select_options` yet.
    fn catalog_options(
        &self,
        pick: impl Fn(
            &revo_core::service::SnapshotSelectOptions,
        ) -> &[revo_core::service::SnapshotOption],
    ) -> Vec<(String, String)> {
        let Some(options) = self.select_options.as_ref() else {
            return Vec::new();
        };
        pick(options)
            .iter()
            .map(|option| (option.value.clone(), option.label.clone()))
            .collect()
    }

    /// Catalog dropdown; falls back to the static list when the engine has
    /// not published this catalog yet.
    fn option_row(
        &mut self,
        label: &str,
        key: &'static str,
        pick: impl Fn(
            &revo_core::service::SnapshotSelectOptions,
        ) -> &[revo_core::service::SnapshotOption],
        fallback: &'static [&'static str],
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let pairs = self.catalog_options(pick);
        if pairs.is_empty() {
            self.dropdown_row(label, key, fallback, cx)
        } else {
            self.dropdown_row_owned(label, key, pairs, cx)
        }
    }

    /// Catalog dropdown that also keeps the static list AND the free-text
    /// value reachable: when a catalog exists, its pairs are merged over the
    /// fallback values and a trailing "Custom (typed)" entry preserves the
    /// current non-catalog text (plan §6–8, devices/encoders).
    fn option_field_row(
        &mut self,
        window: &mut Window,
        label: &str,
        key: &'static str,
        pick: impl Fn(
            &revo_core::service::SnapshotSelectOptions,
        ) -> &[revo_core::service::SnapshotOption],
        fallback: &'static [&'static str],
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let current = self.draft.profile_value(key);
        let catalog = self.catalog_options(pick);
        if catalog.is_empty() {
            if fallback.is_empty() {
                return self.profile_field_row(window, label, key, cx);
            }
            return self.dropdown_row(label, key, fallback, cx);
        }
        let mut pairs = catalog;
        for value in fallback {
            if !pairs.iter().any(|(existing, _)| existing == value) {
                pairs.push((value.to_string(), value.to_string()));
            }
        }
        if !current.is_empty() && !pairs.iter().any(|(value, _)| *value == current) {
            pairs.push((current.clone(), format!("Custom ({current})")));
        }
        self.dropdown_row_owned(label, key, pairs, cx)
    }

    /// Base/output resolution dropdown from the catalog (or the shared
    /// planner presets), always keeping the current custom value visible
    /// (plan §8.1). `field` is the write target for the typed base row; the
    /// output row passes `None` and writes the profile key.
    fn resolution_row(
        &mut self,
        window: &mut Window,
        label: &str,
        field: Option<TextField>,
        key: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let mut pairs = self.catalog_options(|options| &options.video_resolutions);
        if pairs.is_empty() {
            pairs = revo_core::planner::resolution_preset_options();
        }
        if let Some(field) = field {
            let current = self.draft.field_text(field);
            if !current.is_empty() && !pairs.iter().any(|(value, _)| *value == current) {
                pairs.insert(0, (current.clone(), format!("Custom ({current})")));
            }
            let display = pairs
                .iter()
                .find(|(value, _)| *value == current)
                .map(|(_, label)| label.clone())
                .unwrap_or_else(|| current.clone());
            let id_key = format!("fdropdown-{}", self.field_id(field));
            return self.dropdown_box(
                label,
                &id_key,
                display,
                DropdownTarget::Text(field),
                pairs,
                cx,
            );
        }
        let current = self.draft.profile_value(key);
        if !current.is_empty() && !pairs.iter().any(|(value, _)| *value == current) {
            pairs.insert(0, (current.clone(), format!("Custom ({current})")));
        }
        self.dropdown_row_owned(label, key, pairs, cx)
    }

    /// `ui_profile`-backed text row.
    fn profile_field_row(
        &mut self,
        window: &mut Window,
        label: &str,
        key: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let value = self.draft.profile_value(key);
        self.field_row(window, label, &value, TextField::ProfileKey(key), cx)
    }

    /// Section heading inside the content area.
    fn group_label(&self, label: &str) -> gpui::Div {
        div()
            .text_color(self.theme.text_strong)
            .text_size(px(14.))
            .child(label.to_string())
    }

    /// Emits the chosen scene-transfer event for a concrete path.
    fn emit_transfer(&mut self, intent: TransferIntent, path: String, cx: &mut Context<Self>) {
        let event = match intent {
            TransferIntent::ExportRevo => SettingsEvent::ExportRevoSceneCollection { path },
            TransferIntent::ImportRevo => SettingsEvent::ImportRevoSceneCollection { path },
            TransferIntent::ExportObs => SettingsEvent::ExportObsSceneCollection { path },
            TransferIntent::ImportObs => SettingsEvent::ImportObsSceneCollection { path },
        };
        cx.emit(event);
    }

    /// Transfer button press: typed path wins; otherwise the native file
    /// dialog opens ("Save as…" for exports, picker for imports), exactly
    /// the browser-download behavior of plan §3.1. The picked path also
    /// lands in the path box so the next press reuses it.
    fn run_transfer(&mut self, intent: TransferIntent, cx: &mut Context<Self>) {
        let typed = self.draft.import_export_path.trim().to_owned();
        if !typed.is_empty() {
            self.emit_transfer(intent, typed, cx);
            cx.notify();
            return;
        }
        let suggested: &'static str = match intent {
            TransferIntent::ExportRevo | TransferIntent::ImportRevo => "revo-scenes.json",
            TransferIntent::ExportObs | TransferIntent::ImportObs => "obs-scenes.json",
        };
        let starting_dir =
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
        let result = if matches!(
            intent,
            TransferIntent::ExportRevo | TransferIntent::ExportObs
        ) {
            let receiver = cx.prompt_for_new_path(&starting_dir, Some(suggested));
            cx.spawn(async move |modal, cx| {
                let path = receiver.await.ok().and_then(|r| r.ok().flatten());
                let Some(path) = path else { return };
                let _ = modal.update(cx, move |this, cx| {
                    this.draft.import_export_path = path.to_string_lossy().into_owned();
                    this.emit_transfer(intent, this.draft.import_export_path.clone(), cx);
                    cx.notify();
                });
            })
        } else {
            let receiver = cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some("Scene collection file".into()),
            });
            cx.spawn(async move |modal, cx| {
                let picked = receiver
                    .await
                    .ok()
                    .and_then(|r| r.ok().flatten())
                    .and_then(|paths| paths.into_iter().next());
                let Some(path) = picked else { return };
                let _ = modal.update(cx, move |this, cx| {
                    this.draft.import_export_path = path.to_string_lossy().into_owned();
                    this.emit_transfer(intent, this.draft.import_export_path.clone(), cx);
                    cx.notify();
                });
            })
        };
        result.detach();
    }

    /// Plain "pick an existing collection file" for the path box.
    fn browse_collection_path(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Scene collection file".into()),
        });
        cx.spawn(async move |modal, cx| {
            let picked = receiver
                .await
                .ok()
                .and_then(|r| r.ok().flatten())
                .and_then(|paths| paths.into_iter().next());
            let Some(path) = picked else { return };
            let _ = modal.update(cx, move |this, cx| {
                this.draft.import_export_path = path.to_string_lossy().into_owned();
                cx.notify();
            });
        })
        .detach();
    }
    fn section_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Div {
        let settings = self.draft.settings().clone();
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
                            )
                            .child(
                                // Native file dialog (plan §3.1): fills the
                                // path box from the platform picker.
                                div()
                                    .id("general-browse-collection")
                                    .cursor_pointer()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(self.theme.border)
                                    .bg(self.theme.surface_active)
                                    .text_color(self.theme.text)
                                    .text_size(px(11.))
                                    .child("Browse…")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.browse_collection_path(cx);
                                    })),
                            ),
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
                                        this.run_transfer(TransferIntent::ExportRevo, cx);
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
                                        this.run_transfer(TransferIntent::ImportRevo, cx);
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
                                        this.run_transfer(TransferIntent::ExportObs, cx);
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
                                        this.run_transfer(TransferIntent::ImportObs, cx);
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
                    .child(self.profile_toggle_row(
                        // Plan §15.4: workspace gates RemoveScene/RemoveSource.
                        "Confirm before deleting scenes and sources",
                        "confirm_delete",
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
                        .child(self.option_field_row(
                            window,
                            "Audio encoder",
                            "streaming_audio_encoder",
                            |options| &options.streaming_audio_encoders,
                            &["aac", "opus"],
                            cx,
                        ))
                        .child(self.option_field_row(
                            window,
                            "Video encoder",
                            "streaming_video_encoder",
                            |options| &options.streaming_video_encoders,
                            &["x264", "h264_nvenc"],
                            cx,
                        ))
                        .child(self.profile_field_row(
                            window,
                            "Bitrate (kbps)",
                            "streaming_bitrate",
                            cx,
                        ))
                        .child(self.profile_field_row(
                            window,
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
                            window,
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
                            window,
                            "Recording path",
                            &settings.record_path,
                            TextField::RecordPath,
                            cx,
                        ))
                        .child(self.option_row(
                            "Recording format",
                            "record_format",
                            |options| &options.recording_formats,
                            &RECORD_FORMATS,
                            cx,
                        ))
                        .child(self.profile_toggle_row("Advanced", "record_advanced", cx))
                        .child(self.profile_toggle_row("Custom Output", "record_custom_output", cx))
                        .child(self.group_label("Advanced encoder settings"))
                        .child(self.option_field_row(
                            window,
                            "Video Encoder",
                            "record_video_encoder",
                            |options| &options.recording_video_encoders,
                            &["x264", "h264_nvenc"],
                            cx,
                        ))
                        .child(self.option_field_row(
                            window,
                            "Audio Encoder",
                            "record_audio_encoder",
                            |options| &options.recording_audio_encoders,
                            &["aac", "mp4_aac"],
                            cx,
                        ))
                        .child(self.profile_field_row(
                            window,
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
                        .child(self.profile_field_row(window, "Path", "record_custom_path", cx))
                        .child(self.dropdown_row(
                            "Container",
                            "record_custom_container",
                            &RECORD_FORMATS,
                            cx,
                        ))
                        .child(self.profile_field_row(
                            window,
                            "Video bitrate (kbps)",
                            "record_custom_video_bitrate",
                            cx,
                        ))
                        .child(self.profile_field_row(
                            window,
                            "Video encoder",
                            "record_custom_video_encoder",
                            cx,
                        ))
                        .child(self.profile_field_row(
                            window,
                            "Video encoder settings",
                            "record_custom_video_settings",
                            cx,
                        ))
                        .child(self.profile_field_row(
                            window,
                            "Audio bitrate (kbps)",
                            "record_custom_audio_bitrate",
                            cx,
                        ))
                        .child(self.profile_field_row(
                            window,
                            "Audio encoder",
                            "record_custom_audio_encoder",
                            cx,
                        ))
                        .child(self.profile_field_row(
                            window,
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
                                window,
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
                    .child(self.field_row(
                        window,
                        "URL",
                        &settings.stream_url,
                        TextField::StreamUrl,
                        cx,
                    ));
                let key_row = div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .flex_1()
                    .min_w_0()
                    // The key box stretches to touch the visibility button
                    // (plan §5.2): flex column around the field row.
                    .child(div().flex_1().min_w_0().child(self.field_row(
                        window,
                        "Key",
                        settings.stream_key.as_str(),
                        TextField::StreamKey,
                        cx,
                    )));
                body = body.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_end()
                        // 15px between the field and the Hide/Show button.
                        .gap(px(15.))
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
                    .child(self.option_row(
                        "Sample Rate",
                        "audio_sample_rate",
                        |options| &options.audio_sample_rates,
                        &SAMPLE_RATES,
                        cx,
                    ))
                    .child(self.option_row(
                        "Channels",
                        "audio_channels",
                        |options| &options.audio_channels,
                        &CHANNEL_LAYOUTS,
                        cx,
                    ))
                    .child(self.group_label("Global Audio Devices"))
                    .child(self.option_field_row(
                        window,
                        "Desktop Audio",
                        "audio_desktop_device",
                        |options| &options.audio_output_devices,
                        &[],
                        cx,
                    ))
                    .child(self.option_field_row(
                        window,
                        "Mic Audio",
                        "audio_mic_device",
                        |options| &options.audio_input_devices,
                        &[],
                        cx,
                    ))
                    .child(self.group_label("Monitoring device"))
                    .child(self.option_field_row(
                        window,
                        "Monitor device",
                        "audio_monitor_device",
                        |options| &options.audio_output_devices,
                        &[],
                        cx,
                    ))
                    .child(self.profile_toggle_row(
                        "Low latency mode (for NDI outputs)",
                        "audio_low_latency",
                        cx,
                    ));
            }
            SettingsSection::Video => {
                body = body
                    .child(self.group_label("Basic"))
                    .child(self.resolution_row(
                        window,
                        "Base (Canvas) Resolution",
                        Some(TextField::SceneResolution),
                        "video_base_resolution",
                        cx,
                    ))
                    .child(self.resolution_row(
                        window,
                        "Output (Scaled) Resolution",
                        None,
                        "video_output_resolution",
                        cx,
                    ))
                    .child(self.option_row(
                        "Downscale Filter",
                        "video_downscale_filter",
                        |options| &options.video_downscale_filters,
                        &DOWNSCALE_FILTERS,
                        cx,
                    ));
                // Plan §8.2: only the fields belonging to the selected FPS
                // type are shown (Svelte `v-if` parity).
                let fps_type = self.draft.profile_value("video_fps_type");
                body = body.child(self.group_label("FPS")).child(self.dropdown_row(
                    "FPS Type",
                    "video_fps_type",
                    &FPS_TYPES,
                    cx,
                ));
                match fps_type.as_str() {
                    "Integer" => {
                        body = body.child(self.profile_field_row(
                            window,
                            "Integer FPS value",
                            "video_fps_integer",
                            cx,
                        ));
                    }
                    "Fractional" => {
                        body = body
                            .child(self.profile_field_row(
                                window,
                                "Fractional numerator",
                                "video_fps_fraction_numerator",
                                cx,
                            ))
                            .child(self.profile_field_row(
                                window,
                                "Fractional denominator",
                                "video_fps_fraction_denominator",
                                cx,
                            ));
                    }
                    _ => {
                        body = body
                            .child(self.option_row(
                                "Common FPS Values",
                                "video_fps_common",
                                |options| &options.video_common_fps,
                                &COMMON_FPS,
                                cx,
                            ))
                            .child(self.profile_field_row(
                                window,
                                "Custom FPS value",
                                "video_fps_custom",
                                cx,
                            ));
                    }
                }
            }
            SettingsSection::Look => {
                body = body
                    .child(self.group_label("Interface style"))
                    .child(
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
                                            this.draft.set_profile_value(
                                                "interface_style",
                                                value.clone(),
                                            );
                                            this.draft.notice = Some(
                                                "Style applies when the settings are saved".into(),
                                            );
                                            cx.notify();
                                        }))
                                })
                                .collect::<Vec<_>>(),
                        ),
                    )
                    .child(self.group_label("Theme extraction"))
                    .child(
                        div()
                            .id("look-extract-theme")
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .border_1()
                            .border_color(self.theme.border)
                            .bg(self.theme.surface_active)
                            .text_color(self.theme.text)
                            .text_size(px(11.))
                            .child("Extract default theme…")
                            .on_click(cx.listener(|_, _, _, cx| {
                                // Plan §4.2: pick the root directory (themes/ will be
                                // created inside it) via the native folder dialog.
                                let receiver = cx.prompt_for_paths(PathPromptOptions {
                                    files: false,
                                    directories: true,
                                    multiple: false,
                                    prompt: Some("Select theme root directory".into()),
                                });
                                cx.spawn(async move |modal, cx| {
                                    let picked = receiver
                                        .await
                                        .ok()
                                        .and_then(|r| r.ok().flatten())
                                        .and_then(|paths| paths.into_iter().next());
                                    let Some(root_dir) = picked else { return };
                                    let root_dir = root_dir.to_string_lossy().into_owned();
                                    let _ = modal.update(cx, move |_, cx| {
                                        cx.emit(SettingsEvent::ExtractDefaultTheme { root_dir });
                                    });
                                })
                                .detach();
                            })),
                    );
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
                    body = body.child(self.profile_field_row(window, label, key, cx));
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
                    .child(self.dropdown_row("Font size", "a11y_font_size", &FONT_SIZES, cx))
                    .child(self.dropdown_row("Font family", "a11y_font_family", &FONT_FAMILIES, cx))
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
                    .child(self.dropdown_row_pairs(
                        "Select language",
                        "language",
                        &LANGUAGE_VALUES,
                        &LANGUAGE_LABELS,
                        cx,
                    ))
                    .child(self.group_label("Profile Import / Export"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                div()
                                    .id("other-export-profile")
                                    .cursor_pointer()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(self.theme.border)
                                    .bg(self.theme.surface_active)
                                    .text_color(self.theme.text)
                                    .text_size(px(11.))
                                    .child("OBS Studio profile — Export profile")
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        let starting_dir = std::env::current_dir()
                                            .unwrap_or_else(|_| std::path::PathBuf::from("/"));
                                        let receiver = cx.prompt_for_new_path(
                                            &starting_dir,
                                            Some("revo-profile.json"),
                                        );
                                        cx.spawn(async move |modal, cx| {
                                            let path =
                                                receiver.await.ok().and_then(|r| r.ok().flatten());
                                            let Some(path) = path else { return };
                                            let path = path.to_string_lossy().into_owned();
                                            let _ = modal.update(cx, move |_, cx| {
                                                cx.emit(SettingsEvent::ExportProfile { path });
                                            });
                                        })
                                        .detach();
                                    })),
                            )
                            .child(
                                div()
                                    .id("other-import-profile")
                                    .cursor_pointer()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(self.theme.border)
                                    .bg(self.theme.surface_active)
                                    .text_color(self.theme.text)
                                    .text_size(px(11.))
                                    .child("Import profile")
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        let receiver = cx.prompt_for_paths(PathPromptOptions {
                                            files: true,
                                            directories: false,
                                            multiple: false,
                                            prompt: Some("OBS profile JSON".into()),
                                        });
                                        cx.spawn(async move |modal, cx| {
                                            let picked = receiver
                                                .await
                                                .ok()
                                                .and_then(|r| r.ok().flatten())
                                                .and_then(|paths| paths.into_iter().next());
                                            let Some(path) = picked else { return };
                                            let path = path.to_string_lossy().into_owned();
                                            let _ = modal.update(cx, move |_, cx| {
                                                cx.emit(SettingsEvent::ImportProfile { path });
                                            });
                                        })
                                        .detach();
                                    })),
                            ),
                    )
                    .child(self.group_label("Profiles"))
                    .child(self.field_row(
                        window,
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
                        window,
                        "Project root (optional)",
                        settings.root_dir.as_deref().unwrap_or(""),
                        TextField::RootDir,
                        cx,
                    ))
                    .child(self.field_row(
                        window,
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
                        window,
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
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
        let mut content = div().flex().flex_col().gap_2().flex_1().min_w_0().min_h_0();
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
        content = content.child(self.section_body(window, cx));

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

        // Caret-aware editing (plan §5.1): the session owns insertion and
        // selection; the draft mirrors every value change.
        let on_key = cx.listener(|this, event: &KeyDownEvent, _, cx| {
            if let Some(field) = this.focused_field {
                let keystroke = &event.keystroke;
                let mods = EditModifiers {
                    control: keystroke.modifiers.control,
                    alt: keystroke.modifiers.alt,
                    platform: keystroke.modifiers.platform,
                    shift: keystroke.modifiers.shift,
                };
                let id = this.field_id(field);
                let draft_value = this.draft.field_text(field);
                let mut session = this
                    .sessions
                    .remove(&id)
                    .unwrap_or_else(|| CaretSession::new(draft_value.clone()));
                session.sync_value(draft_value);
                if session.apply_keystroke(&keystroke.key, keystroke.key_char.as_deref(), mods) {
                    this.draft.set_field_text(field, session.value.clone());
                }
                this.sessions.insert(id, session);
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
                // Natural height lets the modal-card body own scrolling when
                // sections exceed the viewport; min_h_0 keeps flex children
                // shrinkable so overflow is not clipped.
                .w_full()
                .min_h_0()
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

/// Which scene-collection transfer a button press stands for (plan §3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferIntent {
    ExportRevo,
    ImportRevo,
    ExportObs,
    ImportObs,
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn settings_modal_field_keystrokes_update_draft(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let _ = window;
            SettingsModal::new(
                revo_core::settings::default_settings(),
                RevoTheme::future(),
                cx,
            )
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

    #[gpui::test]
    fn run_transfer_with_typed_path_emits_without_dialog(cx: &mut TestAppContext) {
        // Plan §3.1: a typed path transfers directly; the native dialog is
        // only the fallback, and the events are the workspace's existing
        // BackendCommand bridge.
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let _ = window;
            SettingsModal::new(
                revo_core::settings::default_settings(),
                RevoTheme::future(),
                cx,
            )
        });
        let mut events = cx.events(&entity);
        entity.update(cx, |modal, cx| {
            modal.draft.import_export_path = "/tmp/coll.json".into();
            modal.run_transfer(TransferIntent::ExportRevo, cx);
        });
        cx.run_until_parked();
        let first = events
            .try_next()
            .expect("event")
            .expect("transfer event emitted");
        assert_eq!(
            first,
            SettingsEvent::ExportRevoSceneCollection {
                path: "/tmp/coll.json".into()
            }
        );
    }

    #[gpui::test]
    fn dropdown_targets_write_profile_and_typed_fields(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let _ = window;
            SettingsModal::new(
                revo_core::settings::default_settings(),
                RevoTheme::future(),
                cx,
            )
        });
        entity.update(cx, |modal, cx| {
            modal.write_target(
                DropdownTarget::Profile("record_video_encoder"),
                "h264_nvenc".into(),
            );
            modal.write_target(
                DropdownTarget::Text(TextField::SceneResolution),
                "1280x720".into(),
            );
            assert_eq!(
                modal.draft.profile_value("record_video_encoder"),
                "h264_nvenc"
            );
            assert_eq!(
                modal.draft.field_text(TextField::SceneResolution),
                "1280x720"
            );
            assert_eq!(
                modal.target_value(DropdownTarget::Profile("record_video_encoder")),
                "h264_nvenc"
            );
        });
    }

    #[gpui::test]
    fn catalog_options_map_engine_pairs(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let _ = window;
            SettingsModal::new(
                revo_core::settings::default_settings(),
                RevoTheme::future(),
                cx,
            )
        });
        // Without a catalog the picker yields nothing and rows fall back.
        entity.update(cx, |modal, _| {
            assert!(modal
                .catalog_options(|options| &options.recording_video_encoders)
                .is_empty());
        });
        entity.update(cx, |modal, cx| {
            modal.set_select_options(
                Some(revo_core::service::SnapshotSelectOptions {
                    recording_video_encoders: vec![revo_core::service::SnapshotOption {
                        value: "x264".into(),
                        label: "x264 (very fast)".into(),
                    }],
                    ..Default::default()
                }),
                cx,
            );
            assert_eq!(
                modal.catalog_options(|options| &options.recording_video_encoders),
                vec![("x264".to_string(), "x264 (very fast)".to_string())]
            );
        });
    }

    #[gpui::test]
    fn settings_modal_edits_at_the_caret_after_navigation(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let _ = window;
            SettingsModal::new(
                revo_core::settings::default_settings(),
                RevoTheme::future(),
                cx,
            )
        });
        cx.update(|window, cx| {
            entity.update(cx, |modal, cx| {
                modal.focused_field = Some(TextField::StreamUrl);
                modal.field_focus.focus(window);
                cx.notify();
            });
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("a b c");
        cx.simulate_keystrokes("left left");
        cx.simulate_keystrokes("x");
        cx.run_until_parked();
        entity.update(cx, |modal, _| {
            assert_eq!(
                modal.draft.settings().stream_url,
                "axbc",
                "navigation then insert must edit inside the value, not append"
            );
        });
    }

    #[gpui::test]
    fn settings_modal_shift_selection_replaces_range(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let _ = window;
            SettingsModal::new(
                revo_core::settings::default_settings(),
                RevoTheme::future(),
                cx,
            )
        });
        cx.update(|window, cx| {
            entity.update(cx, |modal, cx| {
                modal.focused_field = Some(TextField::StreamUrl);
                modal.field_focus.focus(window);
                cx.notify();
            });
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("a b c d");
        cx.simulate_keystrokes("home");
        cx.simulate_keystrokes("shift-right shift-right");
        cx.simulate_keystrokes("z");
        cx.run_until_parked();
        entity.update(cx, |modal, _| {
            assert_eq!(
                modal.draft.settings().stream_url,
                "zcd",
                "typing over a selection must replace the selected range"
            );
        });
    }
}
