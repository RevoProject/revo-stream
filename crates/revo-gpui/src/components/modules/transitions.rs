use std::collections::BTreeMap;

use gpui::{
    div, prelude::*, px, Context, FocusHandle, IntoElement, KeyDownEvent, MouseButton, Render,
    SharedString, Window,
};

use crate::{
    actions::{ColorField, ComponentEvent},
    components::editable_text::{CaretField, EditModifiers},
    theme::RevoTheme,
};

/// Font size and left padding the draft boxes render at.
const FIELD_FONT_PX: f32 = 13.;
const FIELD_PAD_PX: f32 = 8.;

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
    Apply {
        kind: String,
        duration_ms: u32,
        params: std::collections::BTreeMap<String, String>,
    },
    /// A transition was created in the draft list (Svelte parity: the
    /// add-transition section); persistence still happens through `Apply`.
    Added {
        name: String,
    },
    /// A transition draft was removed from the list.
    Removed,
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
    /// Lazily created focus handle for the keystroke-driven text fields, plus
    /// the slot currently focused.
    field_focus: Option<FocusHandle>,
    focused_field: Option<FieldSlot>,
    field_carets: std::collections::HashMap<FieldSlot, CaretField>,
    /// Whether the kind selector's expanding list is open.
    kind_dropdown_open: bool,
    /// The selected transition's param key receiving keystrokes.
    focused_param: Option<String>,
}

/// Identifies which draft text field currently owns keyboard input.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum FieldSlot {
    NewName,
    SelectedName,
    SelectedDuration,
    /// One of the selected transition's typed params, by key.
    SelectedParam(String),
}

impl gpui::EventEmitter<TransitionEvent> for TransitionsPanel {}

// Color-picker requests flow through the shared ComponentEvent vocabulary;
// the Workspace opens the popup and routes the pick back to `set_param_value`.
impl gpui::EventEmitter<ComponentEvent> for TransitionsPanel {}

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
            field_focus: None,
            focused_field: None,
            field_carets: Default::default(),
            kind_dropdown_open: false,
            focused_param: None,
        }
    }

    /// Lazily created focus handle backing the panel's keystroke fields.
    fn focus_handle_or_create(&mut self, cx: &mut Context<Self>) -> FocusHandle {
        self.field_focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone()
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

    /// Dropdown target: resolves the kind by its wire value and re-blends
    /// default params (no-op for an unknown value).
    pub fn set_transition_kind_value(&mut self, id: &str, value: &str, cx: &mut Context<Self>) {
        if let Some(kind) = TransitionKind::from_value(value) {
            self.set_transition_kind(id, kind);
        }
        cx.notify();
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
            params: active
                .params
                .iter()
                .filter(|(key, _)| key.as_str() != "duration_ms")
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
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

    /// Cycles the new-transition kind selector to the next kind.
    pub fn cycle_new_kind(&mut self) {
        let all = TransitionKind::all();
        let index = all
            .iter()
            .position(|kind| *kind == self.new_kind)
            .map_or(0, |index| (index + 1) % all.len());
        self.new_kind = all[index];
    }

    /// Cycles the selected transition's kind, blending its params.
    pub fn cycle_selected_kind(&mut self) {
        let Some(id) = self.selected_id.clone() else {
            return;
        };
        if let Some(item) = self.items.iter().find(|item| item.id == id) {
            let all = TransitionKind::all();
            let next = all
                .iter()
                .position(|kind| *kind == item.kind)
                .map_or(0, |index| (index + 1) % all.len());
            let kind = all[next];
            self.set_transition_kind(&id, kind);
        }
    }

    /// Current draft text for one slot (shared by routing and click focus).
    fn slot_text(&self, slot: &FieldSlot) -> String {
        match slot {
            FieldSlot::NewName => self.new_name.clone(),
            FieldSlot::SelectedName => self
                .selected_item()
                .map(|item| item.name.clone())
                .unwrap_or_default(),
            FieldSlot::SelectedDuration => self
                .selected_item()
                .and_then(|item| item.params.get("duration_ms").cloned())
                .unwrap_or_default(),
            FieldSlot::SelectedParam(key) => self
                .selected_item()
                .and_then(|item| item.params.get(key.as_str()).cloned())
                .unwrap_or_default(),
        }
    }

    /// Whether a slot only accepts ASCII digits (duration/numeric params).
    fn slot_digits_only(slot: &FieldSlot, key: &str) -> bool {
        matches!(slot, FieldSlot::SelectedDuration)
            || matches!(
                key,
                "softness_pct" | "sequence_fps" | "transition_point_ms" | "duration_ms"
            )
    }

    /// Writes an edited value back into its draft slot.
    fn set_slot_text(&mut self, slot: &FieldSlot, next: String) {
        match slot {
            FieldSlot::NewName => self.new_name = next,
            FieldSlot::SelectedName => {
                if let Some(id) = self.selected_id.clone() {
                    self.set_transition_name(&id, next);
                }
            }
            FieldSlot::SelectedDuration => {
                if let Some(id) = self.selected_id.clone() {
                    let parsed = next.trim().parse::<i64>().unwrap_or(0);
                    self.set_transition_duration_ms(&id, parsed);
                }
            }
            FieldSlot::SelectedParam(key) => {
                if let Some(id) = self.selected_id.clone() {
                    if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
                        item.params.insert(key.clone(), next);
                    }
                }
            }
        }
    }

    /// Applies one keystroke to a slot's caret session with the numeric
    /// filter the slot needs; returns the new value when it changed.
    fn caret_keystroke(
        &mut self,
        slot: &FieldSlot,
        digits_only: bool,
        keystroke: &gpui::Keystroke,
    ) -> Option<String> {
        let current = self.slot_text(slot);
        let key_char = keystroke
            .key_char
            .as_deref()
            .filter(|chars| !digits_only || chars.chars().all(|c| c.is_ascii_digit()));
        let caret = self.field_carets.entry(slot.clone()).or_default();
        caret.set_value(current.clone());
        let mods = EditModifiers {
            control: keystroke.modifiers.control,
            alt: keystroke.modifiers.alt,
            platform: keystroke.modifiers.platform,
            shift: keystroke.modifiers.shift,
        };
        if caret
            .session
            .apply_keystroke(&keystroke.key, key_char, mods)
            && caret.session.value != current
        {
            return Some(caret.session.value.clone());
        }
        None
    }

    /// Routes one keystroke into the focused draft field; digits-only for
    /// the duration field. Returns true when the value changed.
    pub fn apply_field_keystroke(&mut self, keystroke: &gpui::Keystroke) -> bool {
        let Some(slot) = self.focused_field.clone() else {
            return false;
        };
        let digits_only = matches!(slot, FieldSlot::SelectedDuration);
        match self.caret_keystroke(&slot, digits_only, keystroke) {
            Some(next) => {
                self.set_slot_text(&slot, next);
                true
            }
            _ => false,
        }
    }

    /// Routes one keystroke into the selected transition's focused param;
    /// numeric keys are digits-only.
    fn route_param_keystroke(&mut self, key: &str, keystroke: &gpui::Keystroke) -> bool {
        let Some(id) = self.selected_id.clone() else {
            return false;
        };
        let _ = id;
        let slot = FieldSlot::SelectedParam(key.to_string());
        let digits_only = Self::slot_digits_only(&slot, key);
        match self.caret_keystroke(&slot, digits_only, keystroke) {
            Some(next) => {
                self.set_slot_text(&slot, next);
                true
            }
            _ => false,
        }
    }

    fn draft_field(
        &mut self,
        id: &'static str,
        slot: &FieldSlot,
        placeholder: &str,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let focus = self.focus_handle_or_create(cx);
        let focused = self.focused_field.as_ref() == Some(slot);
        self.carets_for_slot(slot, &value);
        let probe = self
            .field_carets
            .get(slot)
            .map(|c| c.bounds_probe())
            .unwrap_or_else(|| div().into_any_element());
        let caret_box = if focused {
            self.field_carets.get_mut(slot).map(|caret| {
                caret.display(
                    window,
                    FIELD_FONT_PX,
                    FIELD_PAD_PX,
                    self.theme.text,
                    self.theme.accent,
                )
            })
        } else {
            None
        };
        let mut box_div = div()
            .id(id)
            .relative()
            .cursor_pointer()
            .min_w(px(120.))
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
            });
        box_div = if let Some(caret_box) = caret_box {
            box_div.child(caret_box)
        } else {
            box_div.child(if value.is_empty() {
                SharedString::from(placeholder.to_string())
            } else {
                SharedString::from(value)
            })
        };
        let slot_owned = slot.clone();
        box_div
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    this.focused_field = Some(slot_owned.clone());
                    this.field_carets.entry(slot_owned.clone()).or_default();
                    let current = this.slot_text(&slot_owned);
                    this.field_carets
                        .get_mut(&slot_owned)
                        .expect("caret")
                        .place_caret(
                            window,
                            event.position.x,
                            &current,
                            FIELD_FONT_PX,
                            FIELD_PAD_PX,
                        );
                    focus.focus(window);
                    cx.notify();
                }),
            )
            .child(probe)
    }

    /// One typed param row: caret-aware like the draft fields, with the
    /// param-focus routing (`focused_param`) the panel already uses.
    fn param_text_field(
        &mut self,
        key: String,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let slot = FieldSlot::SelectedParam(key.clone());
        let focus = self.focus_handle_or_create(cx);
        self.carets_for_slot(&slot, &value);
        let probe = self
            .field_carets
            .get(&slot)
            .map(|c| c.bounds_probe())
            .unwrap_or_else(|| div().into_any_element());
        let focused = self.focused_param.as_deref() == Some(key.as_str());
        let caret_display =
            if focused {
                Some(self.field_carets.get_mut(&slot).map(|caret| {
                    caret.display(window, 11.2, 8., self.theme.text, self.theme.accent)
                }))
            } else {
                None
            };
        let mut box_div = div()
            .id(SharedString::from(format!("transition-param-text-{key}")))
            .relative()
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
            .text_size(px(11.2));
        box_div = match caret_display.flatten() {
            Some(element) => box_div.child(element),
            None => box_div.child(if value.is_empty() {
                SharedString::from("…")
            } else {
                SharedString::from(value)
            }),
        };
        box_div
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    this.focused_param = Some(key.clone());
                    let slot = FieldSlot::SelectedParam(key.clone());
                    let current = this.slot_text(&slot);
                    this.field_carets.entry(slot.clone()).or_default();
                    this.field_carets
                        .get_mut(&slot)
                        .expect("caret")
                        .place_caret(window, event.position.x, &current, 11.2, 8.);
                    let focus = this.focus_handle_or_create(cx);
                    focus.focus(window);
                    cx.notify();
                }),
            )
            .child(probe)
    }

    /// Ensures the slot's caret session exists and mirrors the draft value.
    fn carets_for_slot(&mut self, slot: &FieldSlot, value: &str) {
        self.field_carets
            .entry(slot.clone())
            .or_default()
            .set_value(value.to_string());
    }
}

impl Render for TransitionsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let mut cards = div().flex().flex_col().gap_1();
        for (index, item) in self.items.clone().into_iter().enumerate() {
            let id = item.id.clone();
            let selected = self.selected_id.as_deref() == Some(&id);
            let active = self.active_id.as_deref() == Some(&id);
            let remove_id = id.clone();
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
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(item.kind.label())
                                    .child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "transition-remove-{id}"
                                            )))
                                            .cursor_pointer()
                                            .px_1()
                                            .text_color(self.theme.text_muted)
                                            .hover(|style| style.text_color(self.theme.danger))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.remove_transition(&remove_id);
                                                cx.emit(TransitionEvent::Removed);
                                                cx.notify();
                                            }))
                                            .child("✕"),
                                    ),
                            ),
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

        // Svelte-parity "Add transition" section: name field, kind selector,
        // and a create button appending a draft transition to the list.
        let new_kind_label = self.new_kind.label().to_string();
        let add_section = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .rounded_md()
            .bg(self.theme.surface_elevated)
            .border_1()
            .border_color(self.theme.border)
            .child(
                div()
                    .text_color(self.theme.text_secondary)
                    .text_size(px(11.))
                    .child("Add transition"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .flex_wrap()
                    .child(self.draft_field(
                        "transition-new-name",
                        &FieldSlot::NewName,
                        "Transition name…",
                        self.new_name.clone(),
                        window,
                        cx,
                    ))
                    .child(
                        div()
                            .id("transition-new-kind")
                            .cursor_pointer()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(self.theme.surface_active)
                            .text_color(self.theme.text)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.cycle_new_kind();
                                cx.notify();
                            }))
                            .child(SharedString::from(new_kind_label)),
                    )
                    .child(
                        div()
                            .id("transition-add")
                            .cursor_pointer()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(self.theme.accent)
                            .text_color(self.theme.text_strong)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(id) = this.add_transition(this.new_kind) {
                                    this.select(&id);
                                    cx.emit(TransitionEvent::Added {
                                        name: this
                                            .items
                                            .iter()
                                            .find(|item| item.id == id)
                                            .map(|item| item.name.clone())
                                            .unwrap_or_else(|| id.clone()),
                                    });
                                }
                                cx.notify();
                            }))
                            .child("Add transition"),
                    ),
            );

        // Selected-transition editor: rename, switch kind, set as the active
        // program transition, or remove the draft entirely.
        let selected_section = self
            .selected_id
            .clone()
            .and_then(|id| self.items.iter().find(|item| item.id == id).cloned())
            .map(|item| {
                let kind_label = item.kind.label().to_string();
                let duration = item
                    .params
                    .get("duration_ms")
                    .cloned()
                    .unwrap_or_else(|| "300".into());
                let active = self.active_id.as_deref() == Some(item.id.as_str());
                let item_id = item.id.clone();
                let set_active_id = item.id.clone();
                let is_active_id = item.id.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_2()
                    .rounded_md()
                    .bg(self.theme.surface_elevated)
                    .border_1()
                    .border_color(self.theme.border)
                    .child(
                        div()
                            .text_color(self.theme.text_secondary)
                            .text_size(px(11.))
                            .child(SharedString::from(format!(
                                "Editing: {}",
                                if item.name.is_empty() {
                                    "Transition".to_string()
                                } else {
                                    item.name.clone()
                                }
                            ))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .flex_wrap()
                            .child(self.draft_field(
                                "transition-selected-name",
                                &FieldSlot::SelectedName,
                                "Name…",
                                item.name.clone(),
                                window,
                                cx,
                            ))
                            .child(self.draft_field(
                                "transition-selected-duration",
                                &FieldSlot::SelectedDuration,
                                "Duration (ms)",
                                duration,
                                window,
                                cx,
                            ))
                            .child(
                                {
                                    let selected_id = self.selected_id.clone().unwrap_or_default();
                                    let current = self
                                        .selected_item()
                                        .map(|item| item.kind.value().to_string())
                                        .unwrap_or_default();
                                    div()
                                        .id("transition-selected-kind")
                                        .cursor_pointer()
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .bg(self.theme.surface_active)
                                        .text_color(self.theme.text)
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .child(SharedString::from(kind_label))
                                        .child(crate::components::icon::icon(
                                            self.theme.text_muted,
                                            "chevron-down",
                                            12.,
                                        ))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |_, event: &gpui::MouseDownEvent, _, cx| {
                                                cx.stop_propagation();
                                                let options = TransitionKind::all()
                                                    .into_iter()
                                                    .map(|kind| {
                                                        (
                                                            kind.value().to_string(),
                                                            kind.label().to_string(),
                                                        )
                                                    })
                                                    .collect();
                                                cx.emit(ComponentEvent::OpenDropdown {
                                                    field: crate::actions::DropdownField::TransitionKind {
                                                        transition_id: selected_id.clone(),
                                                    },
                                                    options,
                                                    current: current.clone(),
                                                    position: (
                                                        f32::from(event.position.x),
                                                        f32::from(event.position.y),
                                                    ),
                                                });
                                            }),
                                        )
                                },
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(
                                div()
                                    .id("transition-set-active")
                                    .cursor_pointer()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(if active {
                                        self.theme.border_selected
                                    } else {
                                        self.theme.surface_active
                                    })
                                    .text_color(self.theme.text)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_active(&set_active_id);
                                        cx.notify();
                                    }))
                                    .child(if active { "Active ✓" } else { "Set active" }),
                            )
                            .child(
                                div()
                                    .id("transition-selected-remove")
                                    .cursor_pointer()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(self.theme.surface_active)
                                    .text_color(self.theme.danger)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.remove_transition(&is_active_id);
                                        cx.emit(TransitionEvent::Removed);
                                        cx.notify();
                                    }))
                                    .child("Remove"),
                            ),
                    )
                    .child(
                        div()
                            .text_color(self.theme.text_muted)
                            .text_size(px(10.))
                            .child(SharedString::from(format!("id: {item_id}"))),
                    )
            });

        // The selected kind opens the shared dropdown overlay (the panel is
        // embedded in a scrollable modal body, so inline expansion would be
        // clipped); `kind_list` stays empty.
        let kind_list: Option<gpui::AnyElement> = None;

        // Per-kind typed params for the selected transition (duration lives
        // in the duration field above; direction/color/softness/stinger
        // fields follow).
        let params_block = self
            .selected_id
            .clone()
            .and_then(|id| self.items.iter().find(|item| item.id == id).cloned())
            .map(|item| {
                let mut rows = div().flex().flex_col().gap_2().mt_1();
                let mut keys: Vec<String> = item.params.keys().cloned().collect();
                keys.sort();
                for key in keys {
                    let value = item.params.get(&key).cloned().unwrap_or_default();
                    let label = match key.as_str() {
                        "direction" => "Direction".to_string(),
                        "color" => "Color".to_string(),
                        "softness_pct" => "Softness (%)".to_string(),
                        "source_mode" => "Stinger source".to_string(),
                        "media_file" => "Media file".to_string(),
                        "sequence_dir" => "Sequence directory".to_string(),
                        "sequence_fps" => "Sequence FPS".to_string(),
                        "transition_point_ms" => "Transition point (ms)".to_string(),
                        other => other.to_string(),
                    };
                    let field: gpui::AnyElement = match key.as_str() {
                        "direction" | "source_mode" => {
                            let options: Vec<(&'static str, &'static str)> = if key == "direction" {
                                vec![
                                    ("left", "left"),
                                    ("right", "right"),
                                    ("up", "up"),
                                    ("down", "down"),
                                ]
                            } else {
                                vec![("media", "media"), ("sequence", "sequence")]
                            };
                            let display = if value.is_empty() {
                                options.first().map(|(_, label)| *label).unwrap_or("")
                            } else {
                                value.as_str()
                            };
                            let transition_id = self.selected_id.clone().unwrap_or_default();
                            let param_key = key.clone();
                            let current = value.clone();
                            let options_owned: Vec<(String, String)> = options
                                .iter()
                                .map(|(value, label)| (value.to_string(), label.to_string()))
                                .collect();
                            div()
                                .id(SharedString::from(format!("transition-param-cycle-{key}")))
                                .cursor_pointer()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(self.theme.surface_active)
                                .text_color(self.theme.text)
                                .text_size(px(11.2))
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(SharedString::from(display.to_string()))
                                .child(crate::components::icon::icon(
                                    self.theme.text_muted,
                                    "chevron-down",
                                    11.,
                                ))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |_, event: &gpui::MouseDownEvent, _, cx| {
                                        cx.stop_propagation();
                                        cx.emit(ComponentEvent::OpenDropdown {
                                            field: crate::actions::DropdownField::TransitionParam {
                                                transition_id: transition_id.clone(),
                                                key: param_key.clone(),
                                            },
                                            options: options_owned.clone(),
                                            current: current.clone(),
                                            position: (
                                                f32::from(event.position.x),
                                                f32::from(event.position.y),
                                            ),
                                        });
                                    }),
                                )
                                .into_any_element()
                        }
                        "color" => {
                            let hex = value.trim_start_matches('#').to_string();
                            let swatch = u32::from_str_radix(&hex, 8)
                                .or_else(|_| u32::from_str_radix(&hex, 16))
                                .unwrap_or(0x000000);
                            let picker_key = key.clone();
                            let picker_current = if value.is_empty() {
                                "#000000".to_string()
                            } else {
                                value.clone()
                            };
                            let picker_transition = self.selected_id.clone().unwrap_or_default();
                            div()
                                .id(SharedString::from(format!("transition-param-color-{key}")))
                                .cursor_pointer()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(self.theme.border_strong)
                                .bg(self.theme.surface_elevated)
                                .text_color(self.theme.text)
                                .text_size(px(11.2))
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .w(px(14.))
                                        .h(px(14.))
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(self.theme.border_strong)
                                        .bg(gpui::rgb(swatch))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(
                                                move |_, event: &gpui::MouseDownEvent, _, cx| {
                                                    // The swatch opens the shared
                                                    // color picker; the rest of the
                                                    // row still focuses hex editing.
                                                    cx.stop_propagation();
                                                    cx.emit(ComponentEvent::OpenColorPicker {
                                                        field: ColorField::TransitionParam {
                                                            transition_id: picker_transition
                                                                .clone(),
                                                            key: picker_key.clone(),
                                                        },
                                                        current: picker_current.clone(),
                                                        position: (
                                                            f32::from(event.position.x),
                                                            f32::from(event.position.y),
                                                        ),
                                                    });
                                                },
                                            ),
                                        ),
                                )
                                .child(SharedString::from(value.clone()))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.focused_param = Some(key.clone());
                                    let focus = this.focus_handle_or_create(cx);
                                    focus.focus(window);
                                    cx.notify();
                                }))
                                .into_any_element()
                        }
                        _ => self
                            .param_text_field(key.clone(), value.clone(), window, cx)
                            .into_any_element(),
                    };
                    rows = rows.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .w(px(140.))
                                    .flex_none()
                                    .text_color(self.theme.text_secondary)
                                    .text_size(px(11.))
                                    .child(SharedString::from(label)),
                            )
                            .child(field),
                    );
                }
                rows
            });

        let focus = self.focus_handle_or_create(cx);
        let on_key = cx.listener(|this, event: &KeyDownEvent, _, cx| {
            if let Some(key) = this.focused_param.clone() {
                if this.route_param_keystroke(&key, &event.keystroke) {
                    cx.notify();
                    return;
                }
            }
            if this.apply_field_keystroke(&event.keystroke) {
                cx.notify();
            }
        });

        crate::components::panel(self.theme)
            .track_focus(&focus)
            .on_key_down(on_key)
            .child(div().child("Transitions"))
            .child(cards)
            .child(if self.items.is_empty() {
                div()
                    .text_color(self.theme.text_muted)
                    .child("No transitions configured.")
            } else {
                div()
            })
            .child(add_section)
            .children(selected_section)
            .children(kind_list)
            .children(params_block)
            .child(
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
                    ),
            )
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
                params: Default::default(),
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
                params: Default::default(),
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
                params: Default::default(),
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
