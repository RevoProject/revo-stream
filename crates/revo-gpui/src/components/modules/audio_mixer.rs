use std::collections::HashMap;

use gpui::{div, prelude::*, px, Context, IntoElement, MouseButton, Render, SharedString, Window};

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
    pub theme: RevoTheme,
}

impl AudioMixerPanel {
    pub fn new(channels: Vec<AudioChannel>, theme: RevoTheme) -> Self {
        Self {
            channels,
            levels: HashMap::new(),
            theme,
        }
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

    pub fn open_context_menu(&mut self, id: &str) -> Option<MixerEvent> {
        self.channel(id)?;
        Some(MixerEvent::ContextMenuRequested { id: id.to_owned() })
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
                        move |this, _, _, cx| {
                            if let Some(event) = this.open_context_menu(&id) {
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
                                .text_color(self.theme.text_strong)
                                .child(channel.name.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_1()
                                .child(
                                    div()
                                        .id(SharedString::from(format!("mixer-mute-{id}")))
                                        .cursor_pointer()
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(self.theme.surface_active)
                                        .text_color(if channel.muted {
                                            self.theme.danger
                                        } else {
                                            self.theme.text
                                        })
                                        .child(if channel.muted { "Unmute" } else { "Mute" })
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
                                        .child(if channel.locked { "Unlock" } else { "Lock" })
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
                                        .child("Menu")
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, _, cx| {
                                                if let Some(event) = this.open_context_menu(&id) {
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
            panel.open_context_menu("camera"),
            Some(MixerEvent::ContextMenuRequested {
                id: "camera".into(),
            })
        );
        assert_eq!(panel.open_context_menu("missing"), None);
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
