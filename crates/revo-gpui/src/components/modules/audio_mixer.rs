use std::collections::HashMap;

use gpui::{
    div, prelude::*, px, ClickEvent, Context, IntoElement, MouseButton, MouseDownEvent, Render,
    SharedString, Window,
};

use crate::theme::RevoTheme;

pub const AUDIO_MIXER_DB_MIN: f32 = -60.0;
pub const AUDIO_MIXER_DB_MAX: f32 = 12.0;
pub const VOLUME_PERCENT_MIN: f32 = 0.0;
pub const VOLUME_PERCENT_MAX: f32 = 200.0;
pub const VOLUME_STEP: f32 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeMode {
    Percent,
    Db,
}

impl VolumeMode {
    pub fn value(self) -> &'static str {
        match self {
            Self::Percent => "percent",
            Self::Db => "db",
        }
    }
}

pub fn clamp_percent(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(VOLUME_PERCENT_MIN, VOLUME_PERCENT_MAX)
    } else {
        100.0
    }
}

pub fn clamp_db(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(AUDIO_MIXER_DB_MIN, AUDIO_MIXER_DB_MAX)
    } else {
        0.0
    }
}

pub fn percent_to_db(percent: f32) -> f32 {
    let normalized = clamp_percent(percent) / 100.0;
    if normalized <= 0.0001 {
        AUDIO_MIXER_DB_MIN
    } else {
        clamp_db(20.0 * normalized.log10())
    }
}

pub fn db_to_percent(db: f32) -> f32 {
    clamp_percent(10f32.powf(clamp_db(db) / 20.0) * 100.0)
}

pub fn format_db(value: f32) -> String {
    let value = clamp_db(value);
    format!("{}{:.1} dB", if value >= 0.0 { "+" } else { "" }, value)
}

#[derive(Clone, Debug, PartialEq)]
pub enum MixerEvent {
    VolumeChanged {
        id: String,
        percent: f32,
        mode: VolumeMode,
    },
    VolumeCommitted {
        id: String,
        percent: f32,
    },
    MuteToggled {
        id: String,
        muted: bool,
    },
    LockToggled {
        id: String,
        locked: bool,
    },
    ContextMenuRequested {
        id: String,
        /// Click point the menu anchors to (plan §17.1).
        position: (f32, f32),
    },
    OpenFilters {
        id: String,
    },
    OpenAdvanced {
        id: String,
    },
    AdvancedConfirmed {
        id: String,
    },
    AdvancedCancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioChannel {
    pub id: String,
    pub name: String,
    pub volume_percent: f32,
    pub volume_db: f32,
    pub volume_mode: VolumeMode,
    pub muted: bool,
    pub locked: bool,
}

impl AudioChannel {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            volume_percent: 100.0,
            volume_db: percent_to_db(100.0),
            volume_mode: VolumeMode::Percent,
            muted: false,
            locked: false,
        }
    }
}

pub struct AudioMixerPanel {
    pub channels: Vec<AudioChannel>,
    levels: HashMap<String, f32>,
    /// Short labels of the filters attached to each channel (plan §17.3);
    /// filled by the workspace's slow filter poll and rendered as badges.
    pub filter_badges: HashMap<String, Vec<String>>,
    pub theme: RevoTheme,
}

/// Short badge label for one libobs filter kind (§17.3).
pub fn filter_badge_label(kind: &str) -> String {
    match kind.trim().to_ascii_lowercase().as_str() {
        "noise_gate" => "gate".to_string(),
        "noise_suppression" => "denoise".to_string(),
        "compressor" => "comp".to_string(),
        "limiter" => "limit".to_string(),
        "expander" => "expand".to_string(),
        "gain" => "gain".to_string(),
        "eq" | "equalizer" => "eq".to_string(),
        "vst" | "vst_plugin" => "vst".to_string(),
        other if other.is_empty() => "filter".to_string(),
        other => other.to_string(),
    }
}

impl AudioMixerPanel {
    pub fn new(channels: Vec<AudioChannel>, theme: RevoTheme) -> Self {
        Self {
            channels,
            levels: HashMap::new(),
            filter_badges: HashMap::new(),
            theme,
        }
    }

    /// Rebuilds the channel list from the backend snapshot: one channel per
    /// audio-capable source, with volumes/mutes/locks applied. The mock
    /// seed only serves mock builds.
    pub fn sync_from_snapshot(
        &mut self,
        items: &[revo_core::service::SourceSnapshotItem],
        volumes: &[(String, u32)],
        muted: &[String],
        locked: &[String],
        levels: &[(String, u32)],
    ) {
        let volume_of = |id: &str| {
            volumes
                .iter()
                .find(|(entry, _)| entry == id)
                .map(|(_, percent)| *percent as f32)
                .unwrap_or(100.0)
        };
        self.channels = items
            .iter()
            .filter(|item| item.audio_capable)
            .map(|item| {
                let mut channel = AudioChannel::new(item.id.clone(), item.name.clone());
                channel.volume_percent = clamp_percent(volume_of(&item.id));
                channel.volume_db = percent_to_db(channel.volume_percent);
                channel.muted = muted.iter().any(|entry| entry == &item.id);
                channel.locked = locked.iter().any(|entry| entry == &item.id);
                channel
            })
            .collect();
        for (id, level) in levels {
            self.set_level(id, *level as f32);
        }
    }

    /// Whether the mixer has any channel to show.
    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    pub fn set_level(&mut self, id: &str, level: f32) {
        if !level.is_finite() {
            return;
        }
        self.levels.insert(id.to_owned(), level.clamp(0.0, 100.0));
    }

    pub fn level(&self, id: &str) -> f32 {
        self.levels
            .get(id)
            .copied()
            .map_or(0.0, |level| level.clamp(0.0, 100.0))
    }

    pub fn set_volume_percent(&mut self, id: &str, value: f32) -> Option<MixerEvent> {
        let channel = self.channel_mut(id)?;
        if channel.locked {
            return None;
        }
        channel.volume_percent = clamp_percent(value);
        channel.volume_db = percent_to_db(channel.volume_percent);
        Some(MixerEvent::VolumeChanged {
            id: id.to_owned(),
            percent: channel.volume_percent,
            mode: channel.volume_mode,
        })
    }

    pub fn set_volume_db(&mut self, id: &str, value: f32) -> Option<MixerEvent> {
        let channel = self.channel_mut(id)?;
        if channel.locked {
            return None;
        }
        channel.volume_db = clamp_db(value);
        channel.volume_percent = db_to_percent(channel.volume_db);
        Some(MixerEvent::VolumeChanged {
            id: id.to_owned(),
            percent: channel.volume_percent,
            mode: channel.volume_mode,
        })
    }

    pub fn commit_volume(&mut self, id: &str) -> Option<MixerEvent> {
        let channel = self.channel_mut(id)?;
        if channel.locked {
            return None;
        }
        let percent = clamp_percent(channel.volume_percent);
        channel.volume_percent = percent;
        channel.volume_db = percent_to_db(channel.volume_db);
        Some(MixerEvent::VolumeCommitted {
            id: id.to_owned(),
            percent,
        })
    }

    pub fn set_volume_mode(&mut self, id: &str, mode: VolumeMode) -> Option<MixerEvent> {
        let channel = self.channel_mut(id)?;
        if channel.volume_mode == mode {
            return None;
        }
        channel.volume_mode = mode;
        match mode {
            VolumeMode::Db => channel.volume_db = percent_to_db(channel.volume_percent),
            VolumeMode::Percent => channel.volume_percent = db_to_percent(channel.volume_db),
        }
        Some(MixerEvent::VolumeChanged {
            id: id.to_owned(),
            percent: channel.volume_percent,
            mode,
        })
    }

    pub fn toggle_mute(&mut self, id: &str) -> Option<MixerEvent> {
        let channel = self.channel_mut(id)?;
        channel.muted = !channel.muted;
        Some(MixerEvent::MuteToggled {
            id: id.to_owned(),
            muted: channel.muted,
        })
    }

    pub fn toggle_lock(&mut self, id: &str) -> Option<MixerEvent> {
        let channel = self.channel_mut(id)?;
        channel.locked = !channel.locked;
        Some(MixerEvent::LockToggled {
            id: id.to_owned(),
            locked: channel.locked,
        })
    }

    pub fn open_context_menu(&mut self, id: &str, position: (f32, f32)) -> Option<MixerEvent> {
        self.channel(id)?;
        Some(MixerEvent::ContextMenuRequested {
            id: id.to_owned(),
            position,
        })
    }

    pub fn channel(&self, id: &str) -> Option<&AudioChannel> {
        self.channels.iter().find(|channel| channel.id == id)
    }

    fn channel_mut(&mut self, id: &str) -> Option<&mut AudioChannel> {
        self.channels.iter_mut().find(|channel| channel.id == id)
    }
}

impl gpui::EventEmitter<MixerEvent> for AudioMixerPanel {}

impl Render for AudioMixerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if self.channels.is_empty() {
            // Empty state (Svelte parity): crossed-speaker icon over a note
            // that no audio sources are available.
            return div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .py_6()
                .text_color(self.theme.text_muted)
                .child(crate::components::icon::icon(
                    self.theme.text_muted,
                    "volume-x",
                    28.,
                ))
                .child(div().text_size(px(12.)).child("No audio sources available"));
        }
        let mut list = div().flex().flex_col().gap_2();
        for channel_zero in 0..self.channels.len() {
            let channel = self.channels[channel_zero].clone();
            let id = channel.id.clone();
            let level = self.level(&id);
            let percent_active = channel.volume_mode == VolumeMode::Percent;
            let volume_label = match channel.volume_mode {
                VolumeMode::Percent => format!("{}%", channel.volume_percent.round()),
                VolumeMode::Db => format_db(channel.volume_db),
            };
            let mut row = div()
                .id(SharedString::from(format!("mixer-row-{id}")))
                .flex()
                .flex_col()
                .gap_2()
                .p_2()
                .bg(self.theme.surface_elevated)
                .border_1()
                .border_color(self.theme.border)
                .rounded_md()
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener({
                        let id = id.clone();
                        move |this, event: &MouseDownEvent, _, cx| {
                            let position =
                                (f32::from(event.position.x), f32::from(event.position.y));
                            if let Some(event) = this.open_context_menu(&id, position) {
                                cx.emit(event);
                            }
                            cx.notify();
                        }
                    }),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_color(self.theme.text_strong)
                                        .child(channel.name.clone()),
                                )
                                .children(self.filter_badges_row(&id)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_1()
                                // Icon controls per the Svelte mixer strip
                                // (`volume-x`/`volume` mute button, lock
                                // icon, `⋯` menu).
                                .child(
                                    div()
                                        .id(SharedString::from(format!("mixer-mute-{id}")))
                                        .cursor_pointer()
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(self.theme.surface_active)
                                        .child(crate::components::icon::icon(
                                            if channel.muted {
                                                self.theme.danger
                                            } else {
                                                self.theme.text
                                            },
                                            if channel.muted { "volume-x" } else { "volume" },
                                            14.,
                                        ))
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, _, cx| {
                                                if let Some(event) = this.toggle_mute(&id) {
                                                    cx.emit(event);
                                                }
                                                cx.notify();
                                            }
                                        })),
                                )
                                .child(
                                    div()
                                        .id(SharedString::from(format!("mixer-lock-{id}")))
                                        .cursor_pointer()
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(self.theme.surface_active)
                                        .child(crate::components::icon::icon(
                                            if channel.locked {
                                                self.theme.accent
                                            } else {
                                                self.theme.text_muted
                                            },
                                            if channel.locked { "lock" } else { "unlock" },
                                            13.,
                                        ))
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, _, cx| {
                                                if let Some(event) = this.toggle_lock(&id) {
                                                    cx.emit(event);
                                                }
                                                cx.notify();
                                            }
                                        })),
                                )
                                .child(
                                    div()
                                        .id(SharedString::from(format!("mixer-menu-{id}")))
                                        .cursor_pointer()
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(self.theme.surface_active)
                                        .child(crate::components::icon::icon(
                                            self.theme.text,
                                            "dots",
                                            14.,
                                        ))
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, event: &ClickEvent, _, cx| {
                                                let position = event.position();
                                                let point =
                                                    (f32::from(position.x), f32::from(position.y));
                                                if let Some(event) =
                                                    this.open_context_menu(&id, point)
                                                {
                                                    cx.emit(event);
                                                }
                                                cx.notify();
                                            }
                                        })),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .child(self.meter(level))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_1()
                                .child(
                                    div()
                                        .id(SharedString::from(format!("mixer-percent-{id}")))
                                        .cursor_pointer()
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(if percent_active {
                                            self.theme.accent
                                        } else {
                                            self.theme.surface_active
                                        })
                                        .text_color(if percent_active {
                                            self.theme.text_strong
                                        } else {
                                            self.theme.text
                                        })
                                        .child("%")
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, _, cx| {
                                                if let Some(event) =
                                                    this.set_volume_mode(&id, VolumeMode::Percent)
                                                {
                                                    cx.emit(event);
                                                }
                                                cx.notify();
                                            }
                                        })),
                                )
                                .child(
                                    div()
                                        .id(SharedString::from(format!("mixer-db-{id}")))
                                        .cursor_pointer()
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(if percent_active {
                                            self.theme.surface_active
                                        } else {
                                            self.theme.accent
                                        })
                                        .text_color(if percent_active {
                                            self.theme.text
                                        } else {
                                            self.theme.text_strong
                                        })
                                        .child("dB")
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, _, cx| {
                                                if let Some(event) =
                                                    this.set_volume_mode(&id, VolumeMode::Db)
                                                {
                                                    cx.emit(event);
                                                }
                                                cx.notify();
                                            }
                                        })),
                                )
                                .child(
                                    div()
                                        .id(SharedString::from(format!("mixer-volume-down-{id}")))
                                        .when(!channel.locked, |control| control.cursor_pointer())
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(self.theme.surface_active)
                                        .text_color(if channel.locked {
                                            self.theme.text_disabled
                                        } else {
                                            self.theme.text
                                        })
                                        .child("-5")
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, _, cx| {
                                                Self::adjust_volume(this, &id, -VOLUME_STEP, cx);
                                            }
                                        })),
                                )
                                .child(
                                    div()
                                        .id(SharedString::from(format!("mixer-volume-up-{id}")))
                                        .when(!channel.locked, |control| control.cursor_pointer())
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(self.theme.surface_active)
                                        .text_color(if channel.locked {
                                            self.theme.text_disabled
                                        } else {
                                            self.theme.text
                                        })
                                        .child("+5")
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, _, cx| {
                                                Self::adjust_volume(this, &id, VOLUME_STEP, cx);
                                            }
                                        })),
                                )
                                .child(
                                    div()
                                        .text_color(self.theme.text_secondary)
                                        .child(volume_label.clone()),
                                ),
                        ),
                );
            if channel.locked {
                row = row
                    .border_color(self.theme.border_subtle)
                    .text_color(self.theme.text_disabled);
            }
            list = list.child(row);
        }
        crate::components::panel(self.theme)
            .child(div().child("Audio Mixer"))
            .child(list)
    }
}

impl AudioMixerPanel {
    fn adjust_volume(&mut self, id: &str, delta: f32, cx: &mut Context<'_, Self>) {
        let current = self
            .channel(id)
            .map_or(100.0, |channel| channel.volume_percent);
        if let Some(event) = self.set_volume_percent(id, current + delta) {
            cx.emit(event);
        }
        if let Some(event) = self.commit_volume(id) {
            cx.emit(event);
        }
        cx.notify();
    }

    /// Stores the short filter labels for one channel (§17.3). Disabled
    /// filters are omitted; an empty list clears the badges.
    pub fn set_filter_badges(&mut self, id: &str, kinds: &[String]) {
        let labels: Vec<String> = kinds.iter().map(|kind| filter_badge_label(kind)).collect();
        if labels.is_empty() {
            self.filter_badges.remove(id);
        } else {
            self.filter_badges.insert(id.to_owned(), labels);
        }
    }

    /// The badges for one channel, empty when it has no filters.
    pub fn badges_for(&self, id: &str) -> Vec<String> {
        self.filter_badges.get(id).cloned().unwrap_or_default()
    }

    /// One channel's filter badges under its name: a muted green chip per
    /// filter, with the full list on hover (§17.3).
    fn filter_badges_row(&self, id: &str) -> Option<gpui::Stateful<gpui::Div>> {
        let labels = self.filter_badges.get(id)?;
        if labels.is_empty() {
            return None;
        }
        let summary = labels.join(" · ");
        Some(
            div()
                .id(SharedString::from(format!("mixer-filters-{id}")))
                .group(SharedString::from(format!("mixer-filters-{id}")))
                .relative()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .flex_wrap()
                .children(labels.iter().cloned().map(|label| {
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .border_1()
                        .border_color(gpui::Rgba {
                            a: 0.55,
                            ..self.theme.success
                        })
                        .bg(gpui::Rgba {
                            a: 0.35,
                            ..self.theme.success
                        })
                        .text_color(self.theme.text)
                        .text_size(px(9.5))
                        .child(SharedString::from(label))
                }))
                // Hover reveals the full filter list as a small overlay.
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_full()
                        .mt_1()
                        .w(px(180.))
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(self.theme.border_strong)
                        .bg(self.theme.surface_elevated)
                        .text_color(self.theme.text)
                        .text_size(px(10.))
                        .opacity(0.)
                        .group_hover(SharedString::from(format!("mixer-filters-{id}")), |style| {
                            style.opacity(1.)
                        })
                        .child(SharedString::from(format!("Filters: {summary}"))),
                ),
        )
    }

    fn meter(&self, level: f32) -> gpui::Div {
        div()
            .flex_1()
            .h(px(12.0))
            .rounded_full()
            .bg(self.theme.border_subtle)
            .border_1()
            .border_color(self.theme.border)
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .w(gpui::relative(level.clamp(0.0, 100.0) / 100.0))
                    .bg(if level > 85.0 {
                        self.theme.danger
                    } else if level > 65.0 {
                        self.theme.warning
                    } else {
                        self.theme.success
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel() -> AudioMixerPanel {
        AudioMixerPanel::new(
            vec![AudioChannel::new("camera", "Camera")],
            RevoTheme::future(),
        )
    }

    #[test]
    fn filter_badges_use_short_labels_and_clear_when_empty() {
        // §17.3: the badge is a short marker, not the raw libobs kind.
        assert_eq!(filter_badge_label("noise_gate"), "gate");
        assert_eq!(filter_badge_label("noise_suppression"), "denoise");
        assert_eq!(filter_badge_label("compressor"), "comp");
        assert_eq!(filter_badge_label("limiter"), "limit");
        assert_eq!(filter_badge_label("gain"), "gain");
        // Unknown and empty kinds still produce a chip.
        assert_eq!(filter_badge_label("future_kind"), "future_kind");
        assert_eq!(filter_badge_label(""), "filter");

        let mut panel = panel();
        assert!(panel.badges_for("camera").is_empty());
        panel.set_filter_badges("camera", &["noise_gate".to_string(), "gain".to_string()]);
        assert_eq!(panel.badges_for("camera"), vec!["gate", "gain"]);
        // An empty list removes the row instead of leaving stale chips.
        panel.set_filter_badges("camera", &[]);
        assert!(panel.badges_for("camera").is_empty());
    }

    #[test]
    fn slider_clamping_follows_svelte_rules() {
        assert_eq!(clamp_percent(250.0), 200.0);
        assert_eq!(clamp_percent(-1.0), 0.0);
        assert_eq!(clamp_percent(f32::NAN), 100.0);
        assert_eq!(clamp_percent(f32::INFINITY), 100.0);
        assert_eq!(clamp_db(f32::INFINITY), 0.0);
        assert_eq!(clamp_db(-90.0), AUDIO_MIXER_DB_MIN);
        assert_eq!(clamp_db(f32::NAN), 0.0);
    }

    #[test]
    fn percent_db_conversion_round_trips() {
        assert_eq!(percent_to_db(0.0), AUDIO_MIXER_DB_MIN);
        assert!((db_to_percent(AUDIO_MIXER_DB_MIN) - 0.1).abs() < 0.01);
        assert_eq!(percent_to_db(100.0), 0.0);
        assert_eq!(db_to_percent(0.0), 100.0);
        assert!(percent_to_db(200.0) > 6.0);
        assert!(percent_to_db(200.0) < 6.1);
    }

    #[test]
    fn mute_toggle_updates_state_and_emits_event() {
        let mut panel = panel();
        assert_eq!(
            panel.toggle_mute("camera"),
            Some(MixerEvent::MuteToggled {
                id: "camera".into(),
                muted: true
            })
        );
        assert!(panel.channel("camera").unwrap().muted);
        assert_eq!(
            panel.toggle_mute("camera"),
            Some(MixerEvent::MuteToggled {
                id: "camera".into(),
                muted: false
            })
        );
        assert!(!panel.channel("camera").unwrap().muted);
        assert_eq!(panel.toggle_mute("missing"), None);
    }

    #[test]
    fn volume_change_clamps_mirrors_db_and_commit_emits() {
        let mut panel = panel();
        let event = panel
            .set_volume_percent("camera", 999.0)
            .expect("volume change emits");
        assert_eq!(
            event,
            MixerEvent::VolumeChanged {
                id: "camera".into(),
                percent: 200.0,
                mode: VolumeMode::Percent,
            }
        );
        assert_eq!(
            panel.channel("camera").unwrap().volume_db,
            percent_to_db(200.0)
        );
        assert_eq!(
            panel.commit_volume("camera"),
            Some(MixerEvent::VolumeCommitted {
                id: "camera".into(),
                percent: 200.0,
            })
        );
    }

    #[test]
    fn locked_channels_ignore_volume_changes() {
        let mut panel = panel();
        panel.toggle_lock("camera");
        assert!(panel.channel("camera").unwrap().locked);
        assert_eq!(panel.set_volume_percent("camera", 123.0), None);
        assert_eq!(panel.set_volume_db("camera", 3.0), None);
        assert_eq!(panel.commit_volume("camera"), None);
        assert_eq!(panel.channel("camera").unwrap().volume_percent, 100.0,);
    }

    #[test]
    fn db_mode_switch_converts_both_directions() {
        let mut panel = panel();
        panel.set_volume_percent("camera", 50.0);
        let switch = panel
            .set_volume_mode("camera", VolumeMode::Db)
            .expect("mode switch emits");
        match switch {
            MixerEvent::VolumeChanged { id, percent, mode } => {
                assert_eq!(id, "camera");
                assert_eq!(mode, VolumeMode::Db);
                assert_eq!(percent, panel.channel("camera").unwrap().volume_percent);
            }
            other => panic!("unexpected event: {other:?}"),
        }
        assert_eq!(
            panel.channel("camera").unwrap().volume_db,
            percent_to_db(50.0)
        );
        panel.set_volume_mode("camera", VolumeMode::Percent);
        assert!((panel.channel("camera").unwrap().volume_percent - 50.0).abs() < 0.05);
    }

    #[test]
    fn context_menu_open_is_typed_per_channel() {
        let mut panel = panel();
        assert_eq!(
            panel.open_context_menu("camera", (12.0, 30.0)),
            Some(MixerEvent::ContextMenuRequested {
                id: "camera".into(),
                position: (12.0, 30.0),
            })
        );
        assert_eq!(panel.open_context_menu("missing", (0.0, 0.0)), None);
    }

    #[test]
    fn meter_keeps_only_the_latest_bounded_value() {
        let mut panel = panel();
        panel.set_level("camera", 42.0);
        panel.set_level("camera", 130.0);
        assert_eq!(panel.level("camera"), 100.0);
        panel.set_level("camera", f32::NAN);
        assert_eq!(panel.level("camera"), 100.0);
        assert_eq!(panel.level("missing"), 0.0);
    }
}
