use gpui::{div, prelude::*, Context, IntoElement, Render, SharedString, Window};

use crate::{
    components::modal_shell::{modal_shell, ModalConfig, ModalSize},
    theme::RevoTheme,
};

use super::audio_mixer::MixerEvent;

pub const TRACK_IDS: [&str; 6] = ["1", "2", "3", "4", "5", "6"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Monitoring {
    Off,
    MonitorOnly,
    On,
}

impl Monitoring {
    pub fn value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::MonitorOnly => "monitor_only",
            Self::On => "on",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::MonitorOnly => "Monitor only (silent output)",
            Self::On => "On",
        }
    }

    pub fn all() -> [Monitoring; 3] {
        [Self::Off, Self::MonitorOnly, Self::On]
    }

    pub fn from_value(value: &str) -> Option<Self> {
        Monitoring::all()
            .iter()
            .find(|candidate| candidate.value() == value)
            .copied()
    }
}

pub fn clamp_balance_pan(pan: i32) -> i32 {
    pan.clamp(-100, 100)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdvancedDraft {
    pub monitoring: Monitoring,
    pub balance_left: i32,
    pub balance_right: i32,
    pub tracks: Vec<String>,
}

impl Default for AdvancedDraft {
    fn default() -> Self {
        Self {
            monitoring: Monitoring::Off,
            balance_left: 100,
            balance_right: 100,
            tracks: vec!["1".into(), "2".into()],
        }
    }
}

impl AdvancedDraft {
    pub fn balance_pan(&self) -> i32 {
        let left = clamp_balance_pan(self.balance_left).max(0);
        let right = clamp_balance_pan(self.balance_right).max(0);
        if right >= left {
            100 - left
        } else {
            -(100 - right)
        }
    }

    pub fn set_balance_pan(&mut self, pan: i32) {
        let pan = clamp_balance_pan(pan);
        if pan >= 0 {
            self.balance_left = 100 - pan;
            self.balance_right = 100;
        } else {
            self.balance_left = 100;
            self.balance_right = 100 + pan;
        }
    }

    pub fn toggle_track(&mut self, track: &str) {
        let mut tracks = if self.tracks.iter().any(|candidate| candidate == track) {
            self.tracks
                .iter()
                .filter(|candidate| candidate.as_str() != track)
                .cloned()
                .collect::<Vec<String>>()
        } else {
            let mut tracks = self.tracks.clone();
            tracks.push(track.to_owned());
            tracks
        };
        tracks.sort_by_key(|candidate| candidate.parse::<u32>().unwrap_or(u32::MAX));
        if tracks.is_empty() {
            tracks.push(track.to_owned());
        }
        self.tracks = tracks;
    }

    pub fn set_monitoring(&mut self, monitoring: Monitoring) {
        self.monitoring = monitoring;
    }
}

pub struct AudioMixerAdvancedModal {
    pub open_id: Option<String>,
    pub source_name: String,
    pub draft: Option<AdvancedDraft>,
    initial: Option<AdvancedDraft>,
    pub theme: RevoTheme,
}

impl AudioMixerAdvancedModal {
    pub fn new(theme: RevoTheme) -> Self {
        Self {
            open_id: None,
            source_name: String::new(),
            draft: None,
            initial: None,
            theme,
        }
    }

    pub fn open(
        &mut self,
        source_id: impl Into<String>,
        source_name: impl Into<String>,
        draft: AdvancedDraft,
    ) {
        self.open_id = Some(source_id.into());
        self.source_name = source_name.into();
        self.initial = Some(draft.clone());
        self.draft = Some(draft);
    }

    pub fn close(&mut self) {
        self.open_id = None;
        self.source_name.clear();
        self.initial = None;
        self.draft = None;
    }

    pub fn is_dirty(&self) -> bool {
        match (&self.draft, &self.initial) {
            (Some(draft), Some(initial)) => draft != initial,
            _ => false,
        }
    }

    pub fn set_monitoring(&mut self, monitoring: Monitoring) {
        if let Some(draft) = self.draft.as_mut() {
            draft.set_monitoring(monitoring);
        }
    }

    pub fn set_balance_pan(&mut self, pan: i32) {
        if let Some(draft) = self.draft.as_mut() {
            draft.set_balance_pan(pan);
        }
    }

    pub fn toggle_track(&mut self, track: &str) {
        if let Some(draft) = self.draft.as_mut() {
            draft.toggle_track(track);
        }
    }

    pub fn confirm(&mut self) -> Option<MixerEvent> {
        self.draft.as_ref()?;
        let id = self.open_id.take()?;
        let _ = self.draft.take();
        self.initial = None;
        Some(MixerEvent::AdvancedConfirmed { id })
    }

    pub fn cancel(&mut self) -> Option<MixerEvent> {
        self.open_id.as_ref()?;
        self.close();
        Some(MixerEvent::AdvancedCancelled)
    }
}

impl gpui::EventEmitter<MixerEvent> for AudioMixerAdvancedModal {}

impl Render for AudioMixerAdvancedModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if self.open_id.is_none() || self.draft.is_none() {
            return div();
        }
        let draft = self.draft.clone().expect("draft present");
        let mut rows = div().flex().flex_col().gap_2().p_4();
        for monitoring in Monitoring::all() {
            let active = draft.monitoring == monitoring;
            rows = rows.child(
                div()
                    .id(SharedString::from(format!(
                        "advanced-monitoring-{monitoring:?}"
                    )))
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(if active {
                        self.theme.accent
                    } else {
                        self.theme.surface_active
                    })
                    .text_color(if active {
                        self.theme.text_strong
                    } else {
                        self.theme.text
                    })
                    .child(monitoring.label())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_monitoring(monitoring);
                        cx.notify();
                    })),
            );
        }
        rows = rows.child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(
                    div()
                        .id("advanced-balance-left")
                        .cursor_pointer()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(self.theme.surface_active)
                        .child("-10")
                        .on_click(cx.listener(|this, _, _, cx| {
                            let pan = this.draft.as_ref().map_or(0, |draft| draft.balance_pan());
                            this.set_balance_pan(pan - 10);
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .id("advanced-balance-right")
                        .cursor_pointer()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(self.theme.surface_active)
                        .child("+10")
                        .on_click(cx.listener(|this, _, _, cx| {
                            let pan = this.draft.as_ref().map_or(0, |draft| draft.balance_pan());
                            this.set_balance_pan(pan + 10);
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .text_color(self.theme.text_secondary)
                        .child(format!("L/R pan: {}", draft.balance_pan())),
                ),
        );
        for track in TRACK_IDS {
            let active = draft.tracks.iter().any(|candidate| candidate == track);
            rows = rows.child(
                div()
                    .id(SharedString::from(format!("advanced-track-{track}")))
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
                        self.theme.accent
                    } else {
                        self.theme.surface_active
                    })
                    .text_color(if active {
                        self.theme.text_strong
                    } else {
                        self.theme.text
                    })
                    .child(format!("Track {track}"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_track(track);
                        cx.notify();
                    })),
            );
        }
        rows = rows.child(
            div()
                .flex()
                .flex_row()
                .justify_end()
                .gap_2()
                .child(
                    div()
                        .id("advanced-cancel")
                        .cursor_pointer()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(self.theme.surface_active)
                        .child("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(event) = this.cancel() {
                                cx.emit(event);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .id("advanced-confirm")
                        .cursor_pointer()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(self.theme.accent)
                        .child(if self.is_dirty() { "Confirm" } else { "Close" })
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(event) = this.confirm() {
                                cx.emit(event);
                            }
                            cx.notify();
                        })),
                ),
        );

        modal_shell(
            &ModalConfig::standard("Advanced Audio Properties", ModalSize::Medium),
            self.theme,
            [SharedString::from(self.source_name.clone())],
        )
        .child(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balance_pan_round_trips_through_left_right_fields() {
        let mut draft = AdvancedDraft::default();
        draft.set_balance_pan(40);
        assert_eq!(draft.balance_left, 60);
        assert_eq!(draft.balance_right, 100);
        assert_eq!(draft.balance_pan(), 40);

        draft.set_balance_pan(-30);
        assert_eq!(draft.balance_left, 100);
        assert_eq!(draft.balance_right, 70);
        assert_eq!(draft.balance_pan(), -30);

        draft.set_balance_pan(500);
        assert_eq!(draft.balance_pan(), 100);
        draft.set_balance_pan(-500);
        assert_eq!(draft.balance_pan(), -100);
    }

    #[test]
    fn track_toggle_never_leaves_an_empty_set() {
        let mut draft = AdvancedDraft {
            tracks: vec!["1".into()],
            ..AdvancedDraft::default()
        };
        draft.toggle_track("2");
        assert_eq!(draft.tracks, ["1".to_string(), "2".to_string()]);
        draft.toggle_track("1");
        draft.toggle_track("2");
        assert_eq!(draft.tracks, ["2".to_string()]);
        assert_eq!(draft.tracks.len(), 1);
        draft.toggle_track("5");
        assert_eq!(draft.tracks, ["2".to_string(), "5".to_string()]);
    }

    #[test]
    fn confirm_emits_only_for_an_open_source_and_closes() {
        let mut modal = AudioMixerAdvancedModal {
            open_id: None,
            source_name: String::new(),
            draft: None,
            initial: None,
            theme: RevoTheme::future(),
        };
        assert_eq!(modal.confirm(), None);

        modal.open("camera", "Camera", AdvancedDraft::default());
        assert!(!modal.is_dirty());
        modal.set_balance_pan(25);
        assert!(modal.is_dirty());
        assert_eq!(
            modal.confirm(),
            Some(MixerEvent::AdvancedConfirmed {
                id: "camera".into()
            })
        );
        assert!(modal.open_id.is_none());

        modal.open("camera", "Camera", AdvancedDraft::default());
        assert_eq!(modal.cancel(), Some(MixerEvent::AdvancedCancelled));
        assert!(!modal.is_dirty());
    }

    #[test]
    fn monitoring_values_match_mapped_svelte_options() {
        let labels: Vec<&str> = Monitoring::all().iter().map(|m| m.label()).collect();
        assert_eq!(labels, ["Off", "Monitor only (silent output)", "On"]);
        assert_eq!(
            Monitoring::from_value("monitor_only"),
            Some(Monitoring::MonitorOnly)
        );
        assert_eq!(Monitoring::from_value("bogus"), None);
    }
}
