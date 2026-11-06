use std::collections::BTreeMap;

use gpui::{div, prelude::*, Context, IntoElement, Render, SharedString, Window};

use crate::theme::RevoTheme;

const DURATION_APPLY_MIN: i64 = 50;
const DURATION_APPLY_MAX: i64 = 20000;
const DURATION_FALLBACK: i64 = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionKind {
    Cut,
    Fade,
    Swipe,
    Slide,
    FadeToColor,
    LumaWipe,
    Stinger,
}

impl TransitionKind {
    pub fn value(self) -> &'static str {
        match self {
            Self::Cut => "cut",
            Self::Fade => "fade",
            Self::Swipe => "swipe",
            Self::Slide => "slide",
            Self::FadeToColor => "fade_to_color",
            Self::LumaWipe => "luma_wipe",
            Self::Stinger => "stinger",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Cut => "Cut",
            Self::Fade => "Fade",
            Self::Swipe => "Swipe",
            Self::Slide => "Slide",
            Self::FadeToColor => "Fade to Color",
            Self::LumaWipe => "Luma Wipe",
            Self::Stinger => "Stinger",
        }
    }

    pub fn all() -> [TransitionKind; 7] {
        [
            Self::Cut,
            Self::Fade,
            Self::Swipe,
            Self::Slide,
            Self::FadeToColor,
            Self::LumaWipe,
            Self::Stinger,
        ]
    }

    pub fn from_value(value: &str) -> Option<Self> {
        TransitionKind::all()
            .iter()
            .find(|candidate| candidate.value() == value)
            .copied()
    }
}

pub type TransitionParams = BTreeMap<String, String>;

pub fn preset_defaults(kind: TransitionKind) -> Vec<(&'static str, &'static str)> {
    match kind {
        TransitionKind::Cut => vec![],
        TransitionKind::Fade => vec![("duration_ms", "300")],
        TransitionKind::Swipe => vec![("duration_ms", "350"), ("direction", "left")],
        TransitionKind::Slide => vec![("duration_ms", "350"), ("direction", "left")],
        TransitionKind::FadeToColor => {
            vec![("duration_ms", "400"), ("color", "#000000")]
        }
        TransitionKind::LumaWipe => vec![
            ("duration_ms", "450"),
            ("direction", "left"),
            ("softness_pct", "35"),
        ],
        TransitionKind::Stinger => vec![
            ("source_mode", "media"),
            ("media_file", ""),
            ("sequence_dir", ""),
            ("sequence_fps", "30"),
            ("transition_point_ms", "1000"),
        ],
    }
}

pub fn blend_transition_params(
    previous: &TransitionParams,
    kind: TransitionKind,
) -> TransitionParams {
    let mut next = previous.clone();
    for (key, default) in preset_defaults(kind) {
        let value = previous
            .get(key)
            .cloned()
            .map_or_else(|| default.to_owned(), |value| value);
        next.insert(key.to_owned(), value);
    }
    next
}

pub fn transition_duration_ms(params: &TransitionParams) -> u32 {
    let parsed = params
        .get("duration_ms")
        .and_then(|raw| raw.trim().parse::<f64>().ok());
    match parsed {
        Some(value) if value.is_finite() => {
            value.round().clamp(0.0, i64::MAX as f64).max(0.0) as u32
        }
        _ => DURATION_FALLBACK as u32,
    }
}

pub fn apply_transition_duration_ms(params: &TransitionParams) -> u32 {
    let parsed = params
        .get("duration_ms")
        .and_then(|raw| raw.trim().parse::<f64>().ok());
    match parsed {
        Some(value) if value.is_finite() => value
            .round()
            .clamp(DURATION_APPLY_MIN as f64, DURATION_APPLY_MAX as f64)
            as u32,
        _ => DURATION_FALLBACK as u32,
    }
}

pub fn normalize_transition_softness(params: &TransitionParams) -> u32 {
    let parsed = params
        .get("softness_pct")
        .and_then(|raw| raw.trim().parse::<f64>().ok());
    match parsed {
        Some(value) if value.is_finite() => value.round().clamp(0.0, 100.0) as u32,
        _ => 35,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionItem {
    pub id: String,
    pub name: String,
    pub kind: TransitionKind,
    pub params: TransitionParams,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransitionEvent {
    Apply { kind: String, duration_ms: u32 },
    Discard,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TransitionsSnapshot {
    items: Vec<TransitionItem>,
    active_id: Option<String>,
}

pub struct TransitionsPanel {
    pub items: Vec<TransitionItem>,
    pub selected_id: Option<String>,
    pub active_id: Option<String>,
    pub new_name: String,
    pub new_kind: TransitionKind,
    snapshot: Option<TransitionsSnapshot>,
    next_id: u64,
    pub theme: RevoTheme,
}

impl gpui::EventEmitter<TransitionEvent> for TransitionsPanel {}

impl TransitionsPanel {
    pub fn new(items: Vec<TransitionItem>, theme: RevoTheme) -> Self {
        let next_id = (items.len() + 1) as u64;
        Self {
            items,
            selected_id: None,
            active_id: None,
            new_name: String::new(),
            new_kind: TransitionKind::Fade,
            snapshot: None,
            next_id,
            theme,
        }
    }

    pub fn begin_edit(&mut self) {
        self.snapshot = Some(TransitionsSnapshot {
            items: self.items.clone(),
            active_id: self.active_id.clone(),
        });
    }

    pub fn is_dirty(&self) -> bool {
        match &self.snapshot {
            Some(snapshot) => snapshot.items != self.items || snapshot.active_id != self.active_id,
            None => false,
        }
    }

    pub fn selected_item(&self) -> Option<&TransitionItem> {
        self.items
            .iter()
            .find(|item| Some(&item.id) == self.selected_id.as_ref())
    }

    pub fn select(&mut self, id: &str) -> bool {
        if !self.items.iter().any(|item| item.id == id) {
            return false;
        }
        self.selected_id = Some(id.to_owned());
        true
    }

    pub fn set_active(&mut self, id: &str) -> bool {
        if !self.items.iter().any(|item| item.id == id) {
            return false;
        }
        self.active_id = Some(id.to_owned());
        true
    }

    pub fn set_transition_name(&mut self, id: &str, name: impl Into<String>) {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
            item.name = name.into();
        }
    }

    pub fn set_transition_kind(&mut self, id: &str, kind: TransitionKind) {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
            item.kind = kind;
            item.params = blend_transition_params(&item.params, kind);
        }
    }

    pub fn update_transition_param(&mut self, id: &str, key: &str, value: impl Into<String>) {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
            item.params.insert(key.to_owned(), value.into());
        }
    }

    pub fn set_transition_duration_ms(&mut self, id: &str, raw_ms: i64) {
        let value = if raw_ms < 0 {
            0
        } else {
            raw_ms.min(i32::MAX as i64) as i32
        };
        self.update_transition_param(id, "duration_ms", value.to_string());
    }

    pub fn set_new_kind(&mut self, kind: TransitionKind) {
        self.new_kind = kind;
    }

    pub fn set_new_name(&mut self, name: impl Into<String>) {
        self.new_name = name.into();
    }

    pub fn add_transition(&mut self, kind: TransitionKind) -> Option<String> {
        self.next_id += 1;
        let id = format!("transition-{}", self.next_id);
        let count = self.items.len() + 1;
        let trimmed_name = self.new_name.trim().to_owned();
        let name = if trimmed_name.is_empty() {
            format!("Transition {count}")
        } else {
            trimmed_name
        };
        let item = TransitionItem {
            id: id.clone(),
            name,
            kind,
            params: blend_transition_params(&BTreeMap::new(), kind),
        };
        self.items.push(item);
        self.selected_id = Some(id.clone());
        self.new_name.clear();
        Some(id)
    }

    pub fn remove_transition(&mut self, id: &str) {
        self.items.retain(|item| item.id != id);
        if self.active_id.as_deref() == Some(id) {
            self.active_id = self.items.first().map(|item| item.id.clone());
        }
        if self.selected_id.as_deref() == Some(id) {
            self.selected_id = self.items.first().map(|item| item.id.clone());
        }
    }

    pub fn apply(&mut self) -> Option<TransitionEvent> {
        if self.items.is_empty() {
            return None;
        }
        let known_active = self
            .items
            .iter()
            .any(|item| Some(&item.id) == self.active_id.as_ref());
        let active_id = if known_active {
            self.active_id
                .clone()
                .unwrap_or_else(|| self.items[0].id.clone())
        } else {
            self.items[0].id.clone()
        };
        self.active_id = Some(active_id.clone());
        let active = self
            .items
            .iter()
            .find(|item| item.id == active_id)
            .expect("active id present");
        let event = TransitionEvent::Apply {
            kind: active.kind.value().to_owned(),
            duration_ms: apply_transition_duration_ms(&active.params),
        };
        self.snapshot = None;
        Some(event)
    }

    pub fn discard(&mut self) {
        if let Some(snapshot) = self.snapshot.take() {
            self.items = snapshot.items;
            self.active_id = snapshot.active_id;
        }
    }
}

impl Render for TransitionsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let mut cards = div().flex().flex_col().gap_1();
        for (index, item) in self.items.iter().enumerate() {
            let id = item.id.clone();
            let selected = self.selected_id.as_deref() == Some(&id);
            let active = self.active_id.as_deref() == Some(&id);
            cards = cards.child(
                div()
                    .id(SharedString::from(format!("transition-card-{id}")))
                    .cursor_pointer()
                    .p_2()
                    .rounded_md()
                    .bg(if selected {
                        self.theme.surface_selected
                    } else {
                        self.theme.surface_elevated
                    })
                    .border_1()
                    .border_color(if selected {
                        self.theme.border_selected
                    } else {
                        self.theme.border
                    })
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(if item.name.is_empty() {
                                format!("Transition {}", index + 1)
                            } else {
                                item.name.clone()
                            })
                            .child(item.kind.label()),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_color(self.theme.text_muted)
                            .child(if active { "Active" } else { "Inactive" })
                            .child(format!("#{}", index + 1)),
                    )
                    .on_click(cx.listener({
                        move |this, _, _, cx| {
                            this.select(&id);
                            cx.notify();
                        }
                    })),
            );
        }

        crate::components::panel(self.theme)
            .child(div().child("Transitions"))
            .child(cards)
            .child(if self.items.is_empty() {
                div()
                    .text_color(self.theme.text_muted)
                    .child("No transitions configured.")
            } else {
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        div()
                            .id("transition-discard")
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(self.theme.surface_active)
                            .child("Discard")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.is_dirty() {
                                    this.discard();
                                    cx.emit(TransitionEvent::Discard);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("transition-save")
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(self.theme.accent)
                            .text_color(self.theme.text_strong)
                            .child(if self.is_dirty() { "Save" } else { "Saved" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(event) = this.apply() {
                                    cx.emit(event);
                                }
                                cx.notify();
                            })),
                    )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel_with(items: Vec<TransitionItem>) -> TransitionsPanel {
        TransitionsPanel::new(items, RevoTheme::future())
    }

    fn fade_item(id: &str, duration: &str) -> TransitionItem {
        let mut params = blend_transition_params(&BTreeMap::new(), TransitionKind::Fade);
        params.insert("duration_ms".into(), duration.into());
        TransitionItem {
            id: id.into(),
            name: id.into(),
            kind: TransitionKind::Fade,
            params,
        }
    }

    #[test]
    fn kind_values_match_svelte_transition_kinds() {
        let values: Vec<&str> = TransitionKind::all().iter().map(|k| k.value()).collect();
        assert_eq!(
            values,
            [
                "cut",
                "fade",
                "swipe",
                "slide",
                "fade_to_color",
                "luma_wipe",
                "stinger"
            ]
        );
        let labels: Vec<&str> = TransitionKind::all().iter().map(|k| k.label()).collect();
        assert_eq!(labels[0], "Cut");
        assert_eq!(labels[4], "Fade to Color");
        assert_eq!(
            TransitionKind::from_value("stinger"),
            Some(TransitionKind::Stinger)
        );
        assert_eq!(TransitionKind::from_value("candy"), None);
    }

    #[test]
    fn preset_defaults_match_svelte_table() {
        assert!(preset_defaults(TransitionKind::Cut).is_empty());
        assert_eq!(
            preset_defaults(TransitionKind::Fade),
            vec![("duration_ms", "300")]
        );
        assert_eq!(
            preset_defaults(TransitionKind::LumaWipe),
            vec![
                ("duration_ms", "450"),
                ("direction", "left"),
                ("softness_pct", "35")
            ]
        );
    }

    #[test]
    fn kind_switch_blends_previous_params_with_new_presets() {
        let mut panel = panel_with(vec![fade_item("t1", "700")]);
        panel.set_transition_kind("t1", TransitionKind::LumaWipe);
        let item = &panel.items[0];
        assert_eq!(item.kind, TransitionKind::LumaWipe);
        assert_eq!(
            item.params.get("duration_ms").map(String::as_str),
            Some("700")
        );
        assert_eq!(
            item.params.get("direction").map(String::as_str),
            Some("left")
        );
        assert_eq!(
            item.params.get("softness_pct").map(String::as_str),
            Some("35")
        );
    }

    #[test]
    fn duration_clamps_follow_apply_rules() {
        let mut panel = panel_with(vec![fade_item("t1", "999999")]);
        let event = panel.apply();
        assert_eq!(
            event,
            Some(TransitionEvent::Apply {
                kind: "fade".into(),
                duration_ms: 20000,
            })
        );

        panel.begin_edit();
        panel.set_transition_duration_ms("t1", 0);
        panel.set_active("fallback");
        panel.items.clear();
        assert_eq!(panel.apply(), None);
    }

    #[test]
    fn duration_input_rules_match_svelte_defaults() {
        let mut params = BTreeMap::new();
        assert_eq!(transition_duration_ms(&params), 300);
        params.insert("duration_ms".into(), "not-a-number".into());
        assert_eq!(transition_duration_ms(&params), 300);
        params.insert("duration_ms".into(), "485.9".into());
        assert_eq!(transition_duration_ms(&params), 486);
        params.insert("duration_ms".into(), "0".into());
        assert_eq!(transition_duration_ms(&params), 0);

        params.insert("duration_ms".into(), "0".into());
        assert_eq!(apply_transition_duration_ms(&params), 50);
        params.insert("duration_ms".into(), "99999".into());
        assert_eq!(apply_transition_duration_ms(&params), 20000);
        params.insert("duration_ms".into(), "junk".into());
        assert_eq!(apply_transition_duration_ms(&params), 300);

        let mut softness = BTreeMap::new();
        assert_eq!(normalize_transition_softness(&softness), 35);
        softness.insert("softness_pct".into(), "140".into());
        assert_eq!(normalize_transition_softness(&softness), 100);
        softness.insert("softness_pct".into(), "-9".into());
        assert_eq!(normalize_transition_softness(&softness), 0);
    }

    #[test]
    fn dirty_state_tracks_snapshot_and_discard_restores() {
        let mut panel = panel_with(vec![fade_item("t1", "300")]);
        panel.set_active("t1");
        panel.begin_edit();
        assert!(!panel.is_dirty());

        panel.set_transition_name("t1", "Fancy fade");
        assert!(panel.is_dirty());
        panel.discard();
        assert_eq!(panel.items[0].name, "t1");
        assert_eq!(panel.active_id.as_deref(), Some("t1"));
        assert!(!panel.is_dirty());
    }

    #[test]
    fn apply_emits_typed_kind_and_clamped_duration_once() {
        let mut panel = panel_with(vec![fade_item("t1", "150"), fade_item("t2", "0")]);
        panel.set_active("t2");
        panel.begin_edit();
        panel.set_transition_name("t2", "Fast");
        let event = panel.apply();
        assert_eq!(
            event,
            Some(TransitionEvent::Apply {
                kind: "fade".into(),
                duration_ms: 50,
            })
        );
        assert!(panel.selected_item().is_none());
        assert!(!panel.is_dirty());
    }

    #[test]
    fn active_falls_back_to_first_item_when_unassigned() {
        let mut panel = panel_with(vec![fade_item("t1", "300"), fade_item("t2", "300")]);
        let event = panel.apply();
        assert_eq!(
            event,
            Some(TransitionEvent::Apply {
                kind: "fade".into(),
                duration_ms: 300,
            })
        );
        assert_eq!(panel.active_id.as_deref(), Some("t1"));
    }

    #[test]
    fn add_and_remove_maintain_selection_fallbacks() {
        let mut panel = panel_with(vec![]);
        panel.set_new_name("  Intro wipe  ");
        panel.set_new_kind(TransitionKind::LumaWipe);
        let added = panel.add_transition(TransitionKind::LumaWipe).unwrap();
        assert_eq!(panel.items[0].name, "Intro wipe");
        assert_eq!(panel.items[0].kind, TransitionKind::LumaWipe);
        assert!(panel.select(&added));

        panel.set_active(&added);
        panel.remove_transition(&added);
        assert!(panel.items.is_empty());
        assert_eq!(panel.active_id, None);
        assert_eq!(panel.selected_id, None);
    }
}
