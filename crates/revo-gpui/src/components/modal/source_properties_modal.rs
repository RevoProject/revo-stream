//! Functional native Source Properties / Edit Source modal, matching the
//! Svelte `SourcePropertiesModal` / `EditSourceModal`: transform fields
//! (size, position) seeded from the backend snapshot, Fit/Stretch-to-canvas
//! helpers computed with `revo-core::planner`, and — in editor mode — the
//! full typed property set fetched from the engine's libobs property API
//! (`SourceSettingsData`). Applying dispatches `BackendCommand::UpdateSource`
//! and `BackendCommand::SetSourceTransform`; the previous values travel with
//! the event so the workspace can record undo history.

use gpui::{
    div, prelude::*, px, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    MouseButton, Render, SharedString, Window,
};

use revo_core::models::{SourcePropertySpec, SourceSettingsData};
use revo_core::planner;

use crate::{
    actions::{ColorField, ComponentEvent, DropdownField, FontField},
    components::editable_text::{
        caret_index_at_x, render_caret_value, CaretSession, EditModifiers,
    },
    components::modal_shell::{modal_shell_with_body, ModalConfig, ModalSize},
    theme::RevoTheme,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Value boxes measure and render their caret with these shared metrics
/// (plan §5.1 applied to the source editor).
const PROP_FONT_PX: f32 = 13.;
const PROP_PAD_PX: f32 = 8.;

/// Minimum height of one transform row (Svelte row spacing parity + 15%).
const TRANSFORM_ROW_MIN_HEIGHT: f32 = 50.0;

#[derive(Clone, Debug, PartialEq)]
pub enum SourcePropertiesEvent {
    /// Transform-only apply from the properties view.
    Applied {
        id: String,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        rotation: i32,
    },
    /// Full apply from the editor: name + typed params + transform, with the
    /// previous values for the workspace's undo history.
    AppliedFull {
        id: String,
        name: String,
        params: Vec<(String, String)>,
        previous_params: Vec<(String, String)>,
        transform: [i32; 5],
        previous_transform: Option<[i32; 5]>,
    },
    /// The edited filter list for `SetSourceFilters`, with the seeded list
    /// for undo.
    AppliedFilters {
        id: String,
        filters: Vec<revo_core::models::SourceFilterItem>,
        previous: Vec<revo_core::models::SourceFilterItem>,
    },
    /// Hand-off requested to the full source editor.
    OpenEditor {
        id: String,
    },
    /// Fetch one filter's typed settings for the per-filter accordion
    /// (editor mode); UI-only, never dispatches a backend command.
    FetchFilterSettings {
        filter_name: String,
        kind: String,
    },
    Cancelled,
}

/// One numeric transform field draft.
#[derive(Clone, Debug, Default)]
struct NumberDraft {
    value: String,
}

/// Rebuilds the General-tab spec list: the libobs "font" property is a
/// nested font object the editor cannot type into, so it becomes the flat
/// `font_face` (font-picker-backed) + `font_size` pair — the same keys
/// `apply_source_params` rebuilds the object from.
fn rebuild_specs(specs: Vec<SourcePropertySpec>) -> Vec<SourcePropertySpec> {
    if !specs.iter().any(|spec| spec.key == "font") {
        return specs;
    }
    let mut rebuilt = Vec::with_capacity(specs.len() + 1);
    for spec in specs {
        if spec.key == "font" {
            rebuilt.push(SourcePropertySpec {
                key: "font_face".into(),
                label: "Font".into(),
                kind: "font".into(),
                hint: String::new(),
                options: Vec::new(),
            });
            rebuilt.push(SourcePropertySpec {
                key: "font_size".into(),
                label: "Font size".into(),
                kind: "int".into(),
                hint: String::new(),
                options: Vec::new(),
            });
        } else {
            rebuilt.push(spec);
        }
    }
    rebuilt
}

/// Draft values for the per-filter settings accordion, validated per kind;
/// empty text values for the synthetic font pair are skipped so they never
/// overwrite engine defaults.
fn collected_filter_param_values(
    drafts: &[(SourcePropertySpec, ParamDraft)],
) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for (spec, draft) in drafts {
        if let ParamDraft::Text(value) = draft {
            if spec.kind == "font" && value.trim().is_empty() {
                continue;
            }
            if spec.key == "font_size" && value.trim().is_empty() {
                continue;
            }
            match spec.kind.as_str() {
                "int" => {
                    let _ = value
                        .trim()
                        .parse::<i64>()
                        .map_err(|_| format!("{} must be a whole number", spec.label))?;
                }
                "float" => {
                    let _ = value
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| format!("{} must be a number", spec.label))?;
                }
                _ => {}
            }
        }
        out.push((spec.key.clone(), draft.value()));
    }
    Ok(out)
}

impl NumberDraft {
    fn set(&mut self, value: i32) {
        self.value = value.to_string();
    }

    fn parsed(&self) -> Option<i32> {
        self.value.trim().parse::<i32>().ok()
    }
}

/// Draft editor for one typed source property.
#[derive(Clone, Debug, PartialEq)]
enum ParamDraft {
    Text(String),
    Bool(bool),
    /// Cycling list; `value` is the current option value.
    List {
        value: String,
        options: Vec<(String, String)>,
    },
}

/// Typed drafts for one filter's settings, seeded from the engine's
/// property API (or the kind's defaults for locally added filters).
/// Saving merges them back into the working filter list, which travels
/// through `BackendCommand::SetSourceFilters`.
#[derive(Clone, Debug, PartialEq)]
struct FilterSettingsDraft {
    filter_name: String,
    drafts: Vec<(SourcePropertySpec, ParamDraft)>,
    /// Whether the engine resolved the filter (false = settings
    /// unavailable, e.g. mock builds or an unknown filter kind).
    available: bool,
}

impl ParamDraft {
    fn value(&self) -> String {
        match self {
            Self::Text(value) => value.clone(),
            Self::Bool(enabled) => if *enabled { "true" } else { "false" }.to_string(),
            Self::List { value, .. } => value.clone(),
        }
    }
}

/// Editor-mode tabs (Svelte EditSourceModal): General carries the name and
/// the source's typed params, Filters the working filter list, Other the
/// transform editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTab {
    General,
    Filters,
    Other,
}

/// Filter kinds offered by the add-filter dropdown; video kinds for video
/// sources, audio kinds for audio sources.
pub const VIDEO_FILTER_KINDS: [&str; 5] = [
    "color_correction",
    "chroma_key",
    "crop_pad",
    "sharpness",
    "scroll",
];
pub const AUDIO_FILTER_KINDS: [&str; 6] = [
    "gain",
    "noise_suppression",
    "noise_gate",
    "compressor",
    "limiter",
    "expander",
];

pub struct SourcePropertiesModal {
    pub theme: RevoTheme,
    pub source_id: String,
    pub source_name: String,
    pub source_type: String,
    /// Active editor tab (editor mode).
    pub active_tab: EditorTab,
    /// The source's filter list (Filters tab); `None` until fetched.
    filters: Option<Vec<revo_core::models::SourceFilterItem>>,
    /// The pre-edit filter list for undo.
    seeded_filters: Vec<revo_core::models::SourceFilterItem>,
    /// Kind selected in the add-filter row.
    new_filter_kind: String,
    /// Name typed in the add-filter row.
    new_filter_name: String,
    /// Which filter's settings accordion is expanded (by name), and its
    /// typed drafts once the engine fetch returns.
    expanded_filter: Option<String>,
    filter_settings: Option<FilterSettingsDraft>,
    /// Editor mode (Svelte EditSourceModal): "Edit Source" title with the
    /// General tab and the full typed property set; filters stay a
    /// documented placeholder until the filter pipeline migrates.
    pub editor: bool,
    pos_x: NumberDraft,
    pos_y: NumberDraft,
    width: NumberDraft,
    height: NumberDraft,
    rotation: NumberDraft,
    /// Editable name draft (editor mode; Svelte editName).
    name_draft: String,
    /// Typed property drafts seeded from the engine settings, in spec order.
    params: Vec<(SourcePropertySpec, ParamDraft)>,
    /// Values as loaded from the engine — the undo baseline.
    seeded_params: Vec<(String, String)>,
    seeded_transform: Option<[i32; 5]>,
    /// Whether the engine settings payload arrived (mock builds keep the
    /// transform-only view).
    settings_loaded: bool,
    /// Scene canvas resolution (`WIDTHxHEIGHT`) used by Fit/Stretch; falls
    /// back to the planner default when settings carry none.
    pub canvas: String,
    pub error: Option<String>,
    pub notice: Option<String>,
    focused_field: Option<usize>,
    main_focus: FocusHandle,
    /// Caret/selection session per focused slot index (plan §14.0).
    sessions: HashMap<usize, CaretSession>,
    /// Per-slot bounds probes for click-to-caret mapping.
    field_bounds: HashMap<usize, Rc<RefCell<Option<gpui::Bounds<gpui::Pixels>>>>>,
}

impl EventEmitter<SourcePropertiesEvent> for SourcePropertiesModal {}

// Color/font picker requests flow through the shared ComponentEvent
// vocabulary; the Workspace opens the popup and routes the picks back to
// `set_param_value` / `set_filter_param`.
impl EventEmitter<ComponentEvent> for SourcePropertiesModal {}

impl Focusable for SourcePropertiesModal {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.main_focus.clone()
    }
}

/// Field slots: 0-4 are the transform numbers, 5+ the property drafts in
/// `params` order; `usize::MAX` is the editable name field.
const TRANSFORM_FIELD_COUNT: usize = 5;
const NAME_FIELD_SLOT: usize = usize::MAX;
/// The add-filter name field slot (must not collide with the name slot).
const FILTER_NAME_SLOT: usize = usize::MAX - 1;
/// Base slot for the per-filter settings accordion fields; far above any
/// realistic param count so the ranges never collide.
const FILTER_PARAM_SLOT_BASE: usize = 1_000_000;

impl SourcePropertiesModal {
    /// Seeds the modal from the snapshot's source item and placed transform.
    /// `canvas` is the configured scene resolution, when known.
    pub fn new(
        source_id: String,
        source_name: String,
        source_type: String,
        transform: Option<revo_core::service::PlacedSource>,
        canvas: Option<String>,
        theme: RevoTheme,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(
            source_id,
            source_name,
            source_type,
            transform,
            canvas,
            false,
            theme,
            cx,
        )
    }

    /// Editor-mode constructor (the Svelte EditSource window).
    pub fn new_editor(
        source_id: String,
        source_name: String,
        source_type: String,
        transform: Option<revo_core::service::PlacedSource>,
        canvas: Option<String>,
        theme: RevoTheme,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(
            source_id,
            source_name,
            source_type,
            transform,
            canvas,
            true,
            theme,
            cx,
        )
    }

    fn build(
        source_id: String,
        source_name: String,
        source_type: String,
        transform: Option<revo_core::service::PlacedSource>,
        canvas: Option<String>,
        editor: bool,
        theme: RevoTheme,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut modal = Self {
            theme,
            source_id,
            source_name: source_name.clone(),
            source_type,
            editor,
            active_tab: EditorTab::General,
            filters: None,
            seeded_filters: Vec::new(),
            new_filter_kind: "color_correction".into(),
            new_filter_name: String::new(),
            expanded_filter: None,
            filter_settings: None,
            // New sources without a placed transform would otherwise start
            // empty and block Apply with "must be a whole number" (plan
            // §14.0): seed the real 0s the boxes already displayed.
            pos_x: NumberDraft { value: "0".into() },
            pos_y: NumberDraft { value: "0".into() },
            width: NumberDraft { value: "0".into() },
            height: NumberDraft { value: "0".into() },
            rotation: NumberDraft { value: "0".into() },
            name_draft: source_name,
            params: Vec::new(),
            seeded_params: Vec::new(),
            seeded_transform: transform
                .as_ref()
                .map(|t| [t.x, t.y, t.width, t.height, t.rotation]),
            settings_loaded: false,
            canvas: canvas.unwrap_or_else(|| "1920x1080".into()),
            error: None,
            notice: None,
            focused_field: None,
            main_focus: cx.focus_handle(),
            sessions: HashMap::new(),
            field_bounds: HashMap::new(),
        };
        if let Some(transform) = transform {
            modal.pos_x.set(transform.x);
            modal.pos_y.set(transform.y);
            modal.width.set(transform.width);
            modal.height.set(transform.height);
            modal.rotation.set(transform.rotation);
        }
        modal
    }

    /// Seeds the typed property drafts from the engine settings payload
    /// (Svelte editParams parity: values normalized to strings).
    pub fn load_settings(&mut self, mut data: SourceSettingsData, cx: &mut Context<Self>) {
        self.seeded_params = data
            .source_properties
            .iter()
            .filter_map(|spec| {
                data.params
                    .get(&spec.key)
                    .map(|value| (spec.key.clone(), value.clone()))
            })
            .collect();
        self.seeded_filters = self.filters.clone().unwrap_or_default();
        // Text sources get a synthetic Text/Text-file mode spec: only one
        // of the "text"/"text_file" fields is relevant at a time (Svelte
        // editSourceMode parity).
        let has_text_fields = data.source_properties.iter().any(|spec| spec.key == "text")
            && data
                .source_properties
                .iter()
                .any(|spec| spec.key == "text_file");
        let mut specs = data.source_properties.clone();
        if has_text_fields {
            let mode_value = data
                .params
                .get("text_file")
                .map(|file| file.trim().is_empty())
                .unwrap_or(true);
            specs.insert(
                0,
                SourcePropertySpec {
                    key: "__mode".into(),
                    label: "Mode".into(),
                    kind: "list".into(),
                    hint: String::new(),
                    options: vec![
                        revo_core::models::SourcePropertyOption {
                            value: "Text".into(),
                            label: "Text".into(),
                        },
                        revo_core::models::SourcePropertyOption {
                            value: "Text file (UTF-8)".into(),
                            label: "Text file (UTF-8)".into(),
                        },
                    ],
                },
            );
            let mode_text = if mode_value {
                "Text"
            } else {
                "Text file (UTF-8)"
            };
            data.params.insert("__mode".into(), mode_text.into());
        }
        // Text sources: the libobs "font" property is a nested font object
        // the editor cannot type into; `rebuild_specs` swaps it for the
        // flat `font_face`/`font_size` pair.
        specs = rebuild_specs(specs);
        self.params = specs
            .iter()
            .map(|spec| {
                let raw = data.params.get(&spec.key).cloned().unwrap_or_default();
                let draft = match spec.kind.as_str() {
                    "bool" => ParamDraft::Bool(raw == "true" || raw == "1"),
                    "list" => ParamDraft::List {
                        value: raw,
                        options: spec
                            .options
                            .iter()
                            .map(|option| (option.value.clone(), option.label.clone()))
                            .collect(),
                    },
                    _ => ParamDraft::Text(raw),
                };
                (spec.clone(), draft)
            })
            .collect();
        if self.name_draft.trim().is_empty() {
            self.name_draft = data.name.clone();
        }
        self.settings_loaded = true;
        self.notice = None;
        cx.notify();
    }

    /// Applies a background settings-fetch result. Ignored once the editor
    /// has already been seeded (e.g. from the warm startup cache), so the
    /// revalidation fetch never overwrites an in-flight draft.
    pub fn refresh_settings(
        &mut self,
        data: Option<revo_core::models::SourceSettingsData>,
        cx: &mut Context<Self>,
    ) {
        if self.settings_loaded {
            return;
        }
        match data {
            Some(data) => self.load_settings(data, cx),
            None => {
                self.notice = Some("Live source settings unavailable (mock backend)".into());
                cx.notify();
            }
        }
    }

    /// The parsed transform, or an error message when a field is not a
    /// whole number.
    fn parsed_transform(&self) -> Result<[i32; 5], String> {
        let parse = |field: &NumberDraft, label: &str| {
            field
                .parsed()
                .ok_or_else(|| format!("{label} must be a whole number"))
        };
        Ok([
            parse(&self.pos_x, "Position X")?,
            parse(&self.pos_y, "Position Y")?,
            parse(&self.width, "Width")?,
            parse(&self.height, "Height")?,
            parse(&self.rotation, "Rotation")?,
        ])
    }

    /// The edited params keyed by spec, or an error for invalid numeric
    /// values.
    fn collected_params(&self) -> Result<Vec<(String, String)>, String> {
        let mut out = Vec::new();
        for (spec, draft) in &self.params {
            if spec.key == "__mode" {
                continue;
            }
            if let ParamDraft::Text(value) = draft {
                // The synthetic font pair skips empty drafts so they never
                // overwrite the source's current font object.
                if spec.kind == "font" && value.trim().is_empty() {
                    continue;
                }
                if spec.key == "font_size" && value.trim().is_empty() {
                    continue;
                }
                match spec.kind.as_str() {
                    "int" => {
                        let _ = value
                            .trim()
                            .parse::<i64>()
                            .map_err(|_| format!("{} must be a whole number", spec.label))?;
                    }
                    "float" => {
                        let _ = value
                            .trim()
                            .parse::<f64>()
                            .map_err(|_| format!("{} must be a number", spec.label))?;
                    }
                    _ => {}
                }
            }
            out.push((spec.key.clone(), draft.value()));
        }
        Ok(out)
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        match self.parsed_transform() {
            Ok(transform) => {
                self.error = None;
                self.notice = Some("Transform applied".into());
                cx.emit(SourcePropertiesEvent::Applied {
                    id: self.source_id.clone(),
                    x: transform[0],
                    y: transform[1],
                    width: transform[2],
                    height: transform[3],
                    rotation: transform[4],
                });
            }
            Err(error) => {
                self.error = Some(error);
                self.notice = None;
            }
        }
        cx.notify();
    }

    /// Full editor apply: name + typed params + transform, with the seeded
    /// values for undo.
    fn apply_full(&mut self, cx: &mut Context<Self>) {
        let transform = match self.parsed_transform() {
            Ok(transform) => transform,
            Err(error) => {
                self.error = Some(error);
                self.notice = None;
                cx.notify();
                return;
            }
        };
        let params = match self.collected_params() {
            Ok(params) => params,
            Err(error) => {
                self.error = Some(error);
                self.notice = None;
                cx.notify();
                return;
            }
        };
        let name = self.name_draft.trim().to_owned();
        if name.is_empty() {
            self.error = Some("Source name is required".into());
            self.notice = None;
            cx.notify();
            return;
        }
        // The open filter-settings accordion merges into the working list
        // before the save so its drafts travel through SetSourceFilters.
        if let Err(error) = self.merge_filter_settings() {
            self.error = Some(error);
            self.notice = None;
            cx.notify();
            return;
        }
        self.error = None;
        self.notice = Some("Source updated".into());
        // Filters apply with the same save when the list was fetched.
        if let Some(filters) = self.filters.clone() {
            let previous = self.seeded_filters.clone();
            if previous != filters {
                cx.emit(SourcePropertiesEvent::AppliedFilters {
                    id: self.source_id.clone(),
                    filters,
                    previous,
                });
            }
        }
        cx.emit(SourcePropertiesEvent::AppliedFull {
            id: self.source_id.clone(),
            name,
            params,
            previous_params: self.seeded_params.clone(),
            transform,
            previous_transform: self.seeded_transform,
        });
        // Plan §14.4: a successful editor save dismisses the modal (the
        // workspace treats `Cancelled` as "close"); every validation error
        // above returns earlier and keeps the draft open.
        cx.emit(SourcePropertiesEvent::Cancelled);
        cx.notify();
    }

    /// Fits the current placed bounds into the scene canvas (uniform scale,
    /// centered) and emits the resulting transform.
    fn fit_to_canvas(&mut self, cx: &mut Context<Self>) {
        self.autofit(false, cx);
    }

    /// Stretches the current placed bounds to fill the scene canvas
    /// (independent scales, centered) and emits the resulting transform.
    fn stretch_to_canvas(&mut self, cx: &mut Context<Self>) {
        self.autofit(true, cx);
    }

    fn autofit(&mut self, stretch: bool, cx: &mut Context<Self>) {
        let Some((canvas_width, canvas_height)) = planner::parse_resolution(&self.canvas) else {
            self.error = Some(format!("Invalid canvas resolution: {}", self.canvas));
            cx.notify();
            return;
        };
        let Ok(current) = self.parsed_transform() else {
            self.error = Some("Fix the transform fields first".into());
            self.notice = None;
            cx.notify();
            return;
        };
        let base_width = current[2].max(0) as u32;
        let base_height = current[3].max(0) as u32;
        let (new_width, new_height) = if stretch {
            match planner::stretch_scale(base_width, base_height, canvas_width, canvas_height) {
                Some((scale_x, scale_y)) => (
                    (base_width as f32 * scale_x).round() as i32,
                    (base_height as f32 * scale_y).round() as i32,
                ),
                None => (current[2], current[3]),
            }
        } else {
            match planner::fit_scale(base_width, base_height, canvas_width, canvas_height) {
                Some(scale) => (
                    (base_width as f32 * scale).round() as i32,
                    (base_height as f32 * scale).round() as i32,
                ),
                None => (current[2], current[3]),
            }
        };
        let (x, y) = planner::centered_position(
            canvas_width,
            canvas_height,
            new_width as f32,
            new_height as f32,
        );
        self.width.set(new_width);
        self.height.set(new_height);
        self.pos_x.set(x.round() as i32);
        self.pos_y.set(y.round() as i32);
        self.error = None;
        self.notice = Some(if stretch { "Stretched" } else { "Fitted" }.into());
        cx.notify();
    }

    fn open_editor(&mut self, cx: &mut Context<Self>) {
        cx.emit(SourcePropertiesEvent::OpenEditor {
            id: self.source_id.clone(),
        });
    }

    fn text_field(
        &mut self,
        id: &'static str,
        focused: bool,
        display: String,
        show_swatch: bool,
        on_click_field: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let text_field_box = div()
            .id(id)
            .cursor_text()
            .flex_1()
            .min_w(px(120.))
            .px_2()
            .py_1()
            .rounded_md()
            .relative()
            .border_1()
            .border_color(if focused {
                self.theme.accent
            } else {
                self.theme.border_strong
            })
            .bg(self.theme.surface_elevated)
            .text_color(if display.is_empty() {
                self.theme.text_muted
            } else {
                self.theme.text
            })
            .flex()
            .items_center()
            .gap_2()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    let was_focused = this.focused_field == Some(on_click_field);
                    this.focused_field = Some(on_click_field);
                    this.main_focus.focus(window);
                    if let Some(value) = this.slot_text(on_click_field) {
                        let mut session = this
                            .sessions
                            .remove(&on_click_field)
                            .unwrap_or_else(|| CaretSession::new(value.clone()));
                        session.sync_value(value);
                        let bounds = this.field_bounds.get(&on_click_field).and_then(|c| *c.borrow());
                        let origin_x = bounds.map(|b| f32::from(b.origin.x)).unwrap_or(f32::MIN / 4.);
                        let local_x = f32::from(event.position.x) - origin_x - PROP_PAD_PX;
                        let index = caret_index_at_x(window, &session.value, local_x.max(0.), PROP_FONT_PX);
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
                                    session.anchor = Some(s); session.cursor = e;
                                }
                                3.. => { session.anchor = Some(0); session.cursor = session.len(); }
                                _ => { if !was_focused && session.cursor > session.len() { session.cursor = session.len(); } session.anchor = None; session.cursor = index; }
                            }
                        }
                        this.sessions.insert(on_click_field, session);
                    }
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(move |this, event: &gpui::MouseMoveEvent, window, cx| {
                if !event.pressed_button.is_some_and(|b| b == MouseButton::Left) { return; }
                if this.focused_field != Some(on_click_field) { return; }
                let bounds = this.field_bounds.get(&on_click_field).and_then(|c| *c.borrow());
                let Some(bounds) = bounds else { return; };
                let local_x = f32::from(event.position.x) - f32::from(bounds.origin.x) - PROP_PAD_PX;
                let mut session = match this.sessions.remove(&on_click_field) { Some(s) => s, None => return };
                let index = caret_index_at_x(window, &session.value, local_x.max(0.), PROP_FONT_PX);
                if session.anchor.is_none() { session.anchor = Some(session.cursor); }
                session.cursor = index;
                this.sessions.insert(on_click_field, session);
                cx.notify();
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                if let Some(session) = this.sessions.get_mut(&on_click_field) {
                    if session.selection_chars().is_none() { session.anchor = None; }
                }
                cx.notify();
            }))
            .children(if show_swatch && !display.is_empty() {
                let hex = display.trim_start_matches('#').to_string();
                let swatch = u32::from_str_radix(&hex, 8)
                    .or_else(|_| u32::from_str_radix(&hex, 16))
                    .unwrap_or(0xFFFFFF);
                Some(
                    div()
                        .w(px(14.))
                        .h(px(14.))
                        .rounded_sm()
                        .border_1()
                        .border_color(self.theme.border_strong)
                        .bg(gpui::rgb(swatch))
                        .into_any_element(),
                )
            } else {
                None
            })
            .child(self.slot_display(
                on_click_field,
                window,
                display.clone(),
                Some("…").filter(|_| !focused),
            ));
        text_field_box.child(self.bounds_probe(on_click_field))
    }

    /// Current draft text of an editable slot (`None` for non-text slots).
    fn slot_text(&self, index: usize) -> Option<String> {
        if index == NAME_FIELD_SLOT {
            return Some(self.name_draft.clone());
        }
        if index == FILTER_NAME_SLOT {
            return Some(self.new_filter_name.clone());
        }
        if index >= FILTER_PARAM_SLOT_BASE {
            let fd = self
                .filter_settings
                .as_ref()?
                .drafts
                .get(index - FILTER_PARAM_SLOT_BASE)?
                .1
                .clone();
            return match fd {
                ParamDraft::Text(value) => Some(value),
                _ => None,
            };
        }
        if index < TRANSFORM_FIELD_COUNT {
            let draft = match index {
                0 => &self.pos_x,
                1 => &self.pos_y,
                2 => &self.width,
                3 => &self.height,
                _ => &self.rotation,
            };
            return Some(draft.value.clone());
        }
        let (_, draft) = self.params.get(index - TRANSFORM_FIELD_COUNT)?;
        match draft {
            ParamDraft::Text(value) => Some(value.clone()),
            _ => None,
        }
    }

    /// Writes a slot's text back into its store (mirrors the legacy
    /// per-slot key routing, now driven by `CaretSession`).
    fn set_slot_text(&mut self, index: usize, value: String) {
        if index == NAME_FIELD_SLOT {
            self.name_draft = value;
            return;
        }
        if index == FILTER_NAME_SLOT {
            self.new_filter_name = value;
            return;
        }
        if index >= FILTER_PARAM_SLOT_BASE {
            if let Some(draft) = self.filter_settings.as_mut() {
                if let Some((_, fd)) = draft.drafts.get_mut(index - FILTER_PARAM_SLOT_BASE) {
                    if let ParamDraft::Text(slot) = fd {
                        *slot = value;
                    }
                }
            }
            return;
        }
        if index < TRANSFORM_FIELD_COUNT {
            let draft = match index {
                0 => &mut self.pos_x,
                1 => &mut self.pos_y,
                2 => &mut self.width,
                3 => &mut self.height,
                _ => &mut self.rotation,
            };
            draft.value = value;
            return;
        }
        if let Some((_, ParamDraft::Text(slot))) =
            self.params.get_mut(index - TRANSFORM_FIELD_COUNT)
        {
            *slot = value;
        }
    }

    /// Focuses a slot from a pointer press and places the caret at the
    /// clicked character using the slot's last recorded bounds.
    fn focus_slot(&mut self, index: usize, event: &gpui::MouseDownEvent, window: &mut Window) {
        let was_focused = self.focused_field == Some(index);
        self.focused_field = Some(index);
        self.main_focus.focus(window);
        let Some(value) = self.slot_text(index) else {
            return;
        };
        let mut session = self
            .sessions
            .remove(&index)
            .unwrap_or_else(|| CaretSession::new(value.clone()));
        if !was_focused {
            session.sync_value(value);
        }
        let bounds = self.field_bounds.get(&index).and_then(|c| *c.borrow());
        let origin_x = bounds
            .map(|b| f32::from(b.origin.x))
            .unwrap_or(f32::MIN / 4.);
        let local_x = f32::from(event.position.x) - origin_x - PROP_PAD_PX;
        let caret = caret_index_at_x(window, &session.value, local_x.max(0.), PROP_FONT_PX);
        session.set_caret(caret, false);
        self.sessions.insert(index, session);
    }

    /// Paints one value box: caret + selection while focused, plain text
    /// otherwise (with the caller's empty placeholder).
    fn slot_display(
        &mut self,
        index: usize,
        window: &Window,
        fallback: String,
        empty_hint: Option<&str>,
    ) -> gpui::AnyElement {
        if self.focused_field == Some(index) {
            let mut session = self
                .sessions
                .remove(&index)
                .unwrap_or_else(|| CaretSession::new(fallback.clone()));
            session.sync_value(fallback.clone());
            let selection_bg = gpui::Rgba {
                a: 0.3,
                ..self.theme.accent
            };
            let element = render_caret_value(
                window,
                &session,
                PROP_FONT_PX,
                PROP_PAD_PX,
                self.theme.text,
                self.theme.accent,
                selection_bg,
            );
            self.sessions.insert(index, session);
            element
        } else {
            div()
                .child(match empty_hint {
                    Some(hint) if fallback.is_empty() => SharedString::from(hint.to_string()),
                    _ => SharedString::from(fallback),
                })
                .into_any_element()
        }
    }

    /// Zero-paint canvas recording where a slot's box actually rendered, so
    /// the next click can map pointer-x onto a caret index.
    fn bounds_probe(&mut self, index: usize) -> gpui::AnyElement {
        let cell = self
            .field_bounds
            .entry(index)
            .or_insert_with(|| Rc::new(RefCell::new(None)))
            .clone();
        gpui::canvas(
            move |bounds, _, _| *cell.borrow_mut() = Some(bounds),
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
        .into_any_element()
    }

    fn on_field_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(index) = self.focused_field else {
            return;
        };
        let Some(value) = self.slot_text(index) else {
            return;
        };
        let keystroke = &event.keystroke;
        let mods = EditModifiers {
            control: keystroke.modifiers.control,
            alt: keystroke.modifiers.alt,
            platform: keystroke.modifiers.platform,
            shift: keystroke.modifiers.shift,
        };
        let mut session = self
            .sessions
            .remove(&index)
            .unwrap_or_else(|| CaretSession::new(value.clone()));
        session.sync_value(value);
        if session.apply_keystroke(&keystroke.key, keystroke.key_char.as_deref(), mods) {
            self.set_slot_text(index, session.value.clone());
            self.notice = None;
        }
        self.sessions.insert(index, session);
        cx.notify();
    }

    fn transform_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Div {
        let pos_row = div()
            .flex()
            .flex_row()
            .gap_2()
            .min_h(px(TRANSFORM_ROW_MIN_HEIGHT))
            .child(self.number_field("Position X", 0, self.pos_x.value.clone(), window, cx))
            .child(self.number_field("Position Y", 1, self.pos_y.value.clone(), window, cx));
        let size_row = div()
            .flex()
            .flex_row()
            .gap_2()
            .min_h(px(TRANSFORM_ROW_MIN_HEIGHT))
            .child(self.number_field("Width", 2, self.width.value.clone(), window, cx))
            .child(self.number_field("Height", 3, self.height.value.clone(), window, cx))
            .child(self.number_field("Rotation", 4, self.rotation.value.clone(), window, cx));
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(pos_row)
            .child(size_row)
    }

    /// One typed property row: bool toggle, list cycler, or text field.
    fn param_row(
        &mut self,
        window: &mut Window,
        spec: &SourcePropertySpec,
        draft: &mut ParamDraft,
        param_index: usize,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let field_index = param_index + TRANSFORM_FIELD_COUNT;
        let label = div()
            .w(px(150.))
            .flex_none()
            .text_color(self.theme.text_secondary)
            .text_size(px(11.))
            .child(SharedString::from(spec.label.clone()));
        let kind = spec.kind.as_str();
        let mut row = div().flex().flex_row().items_center().gap_2().flex_1();
        match draft {
            ParamDraft::Bool(enabled) => {
                row = row.child(
                    div()
                        .id(SharedString::from(format!("source-param-{field_index}")))
                        .cursor_pointer()
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(if *enabled {
                            self.theme.accent
                        } else {
                            self.theme.surface_active
                        })
                        .text_color(if *enabled {
                            self.theme.text_strong
                        } else {
                            self.theme.text_muted
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some((_, ParamDraft::Bool(enabled))) =
                                this.params.get_mut(param_index)
                            {
                                *enabled = !*enabled;
                            }
                            cx.notify();
                        }))
                        .child(if *enabled { "On" } else { "Off" }),
                );
            }
            ParamDraft::List { value, options } => {
                let label_text = options
                    .iter()
                    .find(|(option_value, _)| option_value == value)
                    .map(|(_, label)| label.clone())
                    .unwrap_or_else(|| value.clone());
                let source_id = self.source_id.clone();
                let key = spec.key.clone();
                let current = value.clone();
                let list_options = options.clone();
                row = row.child(
                    div()
                        .id(SharedString::from(format!("source-param-{field_index}")))
                        .cursor_pointer()
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(self.theme.surface_active)
                        .text_color(self.theme.text)
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(SharedString::from(label_text))
                        .child(crate::components::icon::icon(
                            self.theme.text_muted,
                            "chevron-down",
                            12.,
                        ))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |_, event: &gpui::MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                cx.emit(ComponentEvent::OpenDropdown {
                                    field: DropdownField::SourceParam {
                                        source_id: source_id.clone(),
                                        key: key.clone(),
                                    },
                                    options: list_options.clone(),
                                    current: current.clone(),
                                    position: (
                                        f32::from(event.position.x),
                                        f32::from(event.position.y),
                                    ),
                                });
                            }),
                        ),
                );
            }
            ParamDraft::Text(value) => {
                let value = value.clone();
                let focused = self.focused_field == Some(field_index);
                let id: &'static str =
                    Box::leak(format!("source-param-text-{field_index}").into_boxed_str());
                row = row.child(self.text_field(
                    id,
                    focused,
                    value.clone(),
                    kind == "color",
                    field_index,
                    window,
                    cx,
                ));
                if kind == "color" || kind == "font" {
                    // The picker button anchors the shared popup; the hex
                    // / family field stays directly editable either way.
                    let trigger_source = self.source_id.clone();
                    let trigger_key = spec.key.clone();
                    let trigger_kind = spec.kind.clone();
                    let trigger_current = if value.is_empty() {
                        if kind == "font" {
                            "DejaVu Sans".to_string()
                        } else {
                            "#FFFFFF".to_string()
                        }
                    } else {
                        value
                    };
                    row = row.child(
                        div()
                            .id(SharedString::from(format!(
                                "source-param-picker-{field_index}"
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
                            .child(if kind == "font" {
                                "Fonts…"
                            } else {
                                "Pick…"
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |_, event: &gpui::MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    let position =
                                        (f32::from(event.position.x), f32::from(event.position.y));
                                    let event = if trigger_kind == "font" {
                                        ComponentEvent::OpenFontPicker {
                                            field: FontField::SourceFont {
                                                source_id: trigger_source.clone(),
                                            },
                                            current: trigger_current.clone(),
                                            position,
                                        }
                                    } else {
                                        ComponentEvent::OpenColorPicker {
                                            field: ColorField::SourceParam {
                                                source_id: trigger_source.clone(),
                                                key: trigger_key.clone(),
                                            },
                                            current: trigger_current.clone(),
                                            position,
                                        }
                                    };
                                    cx.emit(event);
                                }),
                            ),
                    );
                }
            }
        }
        let mut wrapper = div().flex().flex_col().gap_1();
        wrapper = wrapper.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(label)
                .child(row),
        );
        if !spec.hint.is_empty() {
            wrapper = wrapper.child(
                div()
                    .text_color(self.theme.text_muted)
                    .text_size(px(10.))
                    .child(SharedString::from(spec.hint.clone())),
            );
        }
        wrapper
    }

    fn number_field(
        &mut self,
        label: &str,
        field_index: usize,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let focused = self.focused_field == Some(field_index);
        let box_ = div()
            .id(SharedString::from(format!(
                "source-property-field-{field_index}"
            )))
            .cursor_pointer()
            .px_2()
            .py_1()
            .rounded_md()
            .relative()
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
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    this.focus_slot(field_index, event, window);
                    cx.notify();
                }),
            )
            .child(self.slot_display(field_index, window, value.clone(), Some("0")))
            .child(self.bounds_probe(field_index));
        div()
            .flex()
            .flex_col()
            .gap_1()
            .min_w(px(110.))
            .flex_1()
            .child(
                div()
                    .text_color(self.theme.text_secondary)
                    .text_size(px(11.))
                    .child(label.to_string()),
            )
            .child(box_)
    }

    fn property_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Div {
        let mut rows = div().flex().flex_col().gap_2().min_h(px(120.));
        if !self.settings_loaded {
            rows = rows.child(
                div()
                    .text_color(self.theme.text_muted)
                    .text_size(px(11.))
                    .child(if self.editor {
                        "Loading source settings… (mock builds keep the transform-only view)"
                    } else {
                        "Source properties load in the full editor."
                    }),
            );
            return rows;
        }
        for index in 0..self.params.len() {
            let (spec, mut draft) = self.params[index].clone();
            // Text sources: only the field matching the selected mode shows.
            if spec.key == "text" || spec.key == "text_file" {
                let mode = self
                    .params
                    .iter()
                    .find(|(mode_spec, _)| mode_spec.key == "__mode")
                    .map(|(_, draft)| draft.value())
                    .unwrap_or_else(|| "Text".into());
                let file_mode = mode == "Text file (UTF-8)";
                if (spec.key == "text") == file_mode {
                    continue;
                }
            }
            if spec.kind == "info" {
                rows = rows.child(
                    div()
                        .text_color(self.theme.text_muted)
                        .text_size(px(10.))
                        .child(SharedString::from(spec.label.clone())),
                );
                continue;
            }
            let row = self.param_row(window, &spec, &mut draft, index, cx);
            self.params[index].1 = draft;
            rows = rows.child(row);
        }
        rows
    }

    /// Seeds the filter list from the engine (Filters tab).
    /// Opens the editor on its Filters tab (plan §17.2 mixer entry point).
    pub fn focus_filters_tab(&mut self) {
        self.active_tab = EditorTab::Filters;
    }

    pub fn load_filters(
        &mut self,
        filters: Vec<revo_core::models::SourceFilterItem>,
        cx: &mut Context<Self>,
    ) {
        self.seeded_filters = self.filters.clone().unwrap_or_default();
        if self.seeded_filters.is_empty() {
            self.seeded_filters = filters.clone();
        }
        self.filters = Some(filters);
        cx.notify();
    }

    /// Expands (or collapses) one filter's settings accordion; expanding
    /// requests the typed settings through
    /// [`SourcePropertiesEvent::FetchFilterSettings`].
    fn expand_filter(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(filters) = &self.filters else {
            return;
        };
        let Some(item) = filters.get(index) else {
            return;
        };
        let name = item.name.clone();
        if self.expanded_filter.as_deref() == Some(name.as_str()) {
            self.expanded_filter = None;
            self.filter_settings = None;
            cx.notify();
            return;
        }
        self.expanded_filter = Some(name.clone());
        self.filter_settings = None;
        self.focused_field = None;
        cx.emit(SourcePropertiesEvent::FetchFilterSettings {
            filter_name: name,
            kind: item.kind.clone(),
        });
        cx.notify();
    }

    /// Stores a completed filter-settings fetch (typed property specs) for
    /// the expanded filter; `None` marks the settings unavailable.
    pub fn load_filter_settings(
        &mut self,
        filter_name: String,
        settings: Option<SourceSettingsData>,
        cx: &mut Context<Self>,
    ) {
        if self.expanded_filter.as_deref() != Some(filter_name.as_str()) {
            return;
        }
        let Some(settings) = settings else {
            self.filter_settings = Some(FilterSettingsDraft {
                filter_name,
                drafts: Vec::new(),
                available: false,
            });
            cx.notify();
            return;
        };
        let drafts = settings
            .source_properties
            .iter()
            .map(|spec| {
                let raw = settings.params.get(&spec.key).cloned().unwrap_or_default();
                let draft = match spec.kind.as_str() {
                    "bool" => ParamDraft::Bool(raw == "true" || raw == "1"),
                    "list" => ParamDraft::List {
                        value: raw,
                        options: spec
                            .options
                            .iter()
                            .map(|option| (option.value.clone(), option.label.clone()))
                            .collect(),
                    },
                    _ => ParamDraft::Text(raw),
                };
                (spec.clone(), draft)
            })
            .collect();
        self.filter_settings = Some(FilterSettingsDraft {
            filter_name,
            drafts,
            available: true,
        });
        cx.notify();
    }

    /// Merges the open accordion's drafts into the working filter list so
    /// the next Apply saves them through `SetSourceFilters`.
    fn merge_filter_settings(&mut self) -> Result<(), String> {
        let Some(draft) = &self.filter_settings else {
            return Ok(());
        };
        if !draft.available {
            return Ok(());
        }
        let values = collected_filter_param_values(&draft.drafts)?;
        let Some(filters) = self.filters.as_mut() else {
            return Ok(());
        };
        let Some(item) = filters
            .iter_mut()
            .find(|item| item.name == draft.filter_name)
        else {
            return Ok(());
        };
        for (key, value) in values {
            item.params.insert(key, value);
        }
        Ok(())
    }

    /// Color-picker target for one General-tab param row.
    pub fn set_param_value(&mut self, key: &str, value: &str, cx: &mut Context<Self>) {
        for (spec, draft) in self.params.iter_mut() {
            if spec.key == key {
                *draft = ParamDraft::Text(value.to_string());
                break;
            }
        }
        cx.notify();
    }

    /// Color-picker target for one filter-param row in the accordion.
    pub fn set_filter_param(
        &mut self,
        filter_name: &str,
        key: &str,
        value: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(draft) = &mut self.filter_settings else {
            return;
        };
        if draft.filter_name != filter_name {
            return;
        }
        for (spec, fd) in draft.drafts.iter_mut() {
            if spec.key == key {
                *fd = ParamDraft::Text(value.to_string());
                break;
            }
        }
        cx.notify();
    }

    /// Dropdown target: selects the kind offered by the add-filter row.
    pub fn set_new_filter_kind(&mut self, kind: &str, cx: &mut Context<Self>) {
        self.new_filter_kind = kind.to_string();
        cx.notify();
    }

    /// The audio/video filter kinds offered for this source type.
    fn offered_filter_kinds(&self) -> &'static [&'static str] {
        let audio = self.source_type.contains("pulse")
            || self.source_type.contains("wasapi")
            || self.source_type.contains("coreaudio")
            || self.source_type.contains("audio");
        if audio {
            &AUDIO_FILTER_KINDS
        } else {
            &VIDEO_FILTER_KINDS
        }
    }

    /// The working Filters tab: list with enabled toggles and remove, plus
    /// the add row (kind dropdown + name).
    fn filters_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Div {
        let mut body = div().flex().flex_col().gap_2();
        let Some(filters) = self.filters.clone() else {
            return body.child(
                div()
                    .text_color(self.theme.text_muted)
                    .text_size(px(11.))
                    .child("Loading filters… (mock builds have no filter list)"),
            );
        };
        for index in 0..filters.len() {
            let item = filters[index].clone();
            let enabled = item.enabled;
            let expanded = self.expanded_filter.as_deref() == Some(item.name.as_str());
            let row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(if expanded {
                    self.theme.accent
                } else {
                    self.theme.border
                })
                .bg(self.theme.surface_elevated)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(SharedString::from(item.name.clone())),
                )
                .child(
                    div()
                        .text_color(self.theme.text_muted)
                        .text_size(px(10.))
                        .child(SharedString::from(item.kind.clone())),
                )
                .child(
                    div()
                        .id(SharedString::from(format!(
                            "source-filter-settings-{index}"
                        )))
                        .cursor_pointer()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(if expanded {
                            self.theme.surface_active
                        } else {
                            self.theme.surface_elevated
                        })
                        .border_1()
                        .border_color(if expanded {
                            self.theme.accent
                        } else {
                            self.theme.border_strong
                        })
                        .text_color(if expanded {
                            self.theme.text_strong
                        } else {
                            self.theme.text_muted
                        })
                        .text_size(px(11.))
                        .child(if expanded {
                            "Settings ▴"
                        } else {
                            "Settings ▾"
                        })
                        .on_click(cx.listener(move |this, _, _, cx| this.expand_filter(index, cx))),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("source-filter-toggle-{index}")))
                        .cursor_pointer()
                        .px_2()
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
                        .text_size(px(11.))
                        .child(if enabled { "On" } else { "Off" })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(filters) = this.filters.as_mut() {
                                if let Some(item) = filters.get_mut(index) {
                                    item.enabled = !item.enabled;
                                }
                            }
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("source-filter-remove-{index}")))
                        .cursor_pointer()
                        .px_2()
                        .py_1()
                        .text_color(self.theme.text_muted)
                        .hover(|style| style.text_color(self.theme.danger))
                        .child("✕")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(filters) = this.filters.as_mut() {
                                if index < filters.len() {
                                    let removed = filters[index].name.clone();
                                    if this.expanded_filter.as_deref() == Some(removed.as_str()) {
                                        this.expanded_filter = None;
                                        this.filter_settings = None;
                                    }
                                    filters.remove(index);
                                }
                            }
                            cx.notify();
                        })),
                );
            body = body.child(row);
            if expanded {
                body = body.child(self.filter_settings_body(window, &item, cx));
            }
        }
        // Add-filter row: kind cycle + name + add button.
        let kind_display = if self.new_filter_kind.is_empty() {
            self.offered_filter_kinds()
                .first()
                .copied()
                .unwrap_or("")
                .to_string()
        } else {
            self.new_filter_kind.clone()
        };
        body = body
            .child(
                div()
                    .text_color(self.theme.text_secondary)
                    .text_size(px(11.))
                    .child("Add filter"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("source-filter-add-kind")
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
                            .child(SharedString::from(kind_display.clone()))
                            .child(crate::components::icon::icon(
                                self.theme.text_muted,
                                "chevron-down",
                                11.,
                            ))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    let options = this
                                        .offered_filter_kinds()
                                        .iter()
                                        .map(|kind| (kind.to_string(), kind.to_string()))
                                        .collect::<Vec<_>>();
                                    cx.emit(ComponentEvent::OpenDropdown {
                                        field: DropdownField::AddFilterKind {
                                            source_id: this.source_id.clone(),
                                        },
                                        options,
                                        current: this.new_filter_kind.clone(),
                                        position: (
                                            f32::from(event.position.x),
                                            f32::from(event.position.y),
                                        ),
                                    });
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("source-filter-add-name")
                            .cursor_pointer()
                            .flex_1()
                            .min_w(px(120.))
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .relative()
                            .border_1()
                            .border_color(if self.focused_field == Some(FILTER_NAME_SLOT) {
                                self.theme.accent
                            } else {
                                self.theme.border_strong
                            })
                            .bg(self.theme.surface_elevated)
                            .text_color(if self.new_filter_name.is_empty() {
                                self.theme.text_muted
                            } else {
                                self.theme.text
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                                    this.focus_slot(FILTER_NAME_SLOT, event, window);
                                    cx.notify();
                                }),
                            )
                            .child(self.slot_display(
                                FILTER_NAME_SLOT,
                                window,
                                self.new_filter_name.clone(),
                                Some("Filter name…"),
                            ))
                            .child(self.bounds_probe(FILTER_NAME_SLOT)),
                    )
                    .child(
                        div()
                            .id("source-filter-add")
                            .cursor_pointer()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(self.theme.accent)
                            .text_color(self.theme.text_strong)
                            .text_size(px(11.2))
                            .child("Add")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let name = if this.new_filter_name.trim().is_empty() {
                                    format!(
                                        "{} {}",
                                        this.new_filter_kind,
                                        this.filters
                                            .as_ref()
                                            .map_or(1, |filters| filters.len() + 1)
                                    )
                                } else {
                                    this.new_filter_name.trim().to_owned()
                                };
                                if let Some(filters) = this.filters.as_mut() {
                                    filters.push(revo_core::models::SourceFilterItem {
                                        id: this.new_filter_kind.clone(),
                                        name,
                                        kind: this.new_filter_kind.clone(),
                                        enabled: true,
                                        locked: false,
                                        params: Default::default(),
                                    });
                                }
                                this.new_filter_name.clear();
                                this.expanded_filter = None;
                                this.filter_settings = None;
                                cx.notify();
                            })),
                    ),
            );
        body
    }

    /// The expanded filter's settings accordion: typed param drafts seeded
    /// from the engine (or the kind's defaults for locally added filters).
    /// Edits stay in the working draft until Apply.
    fn filter_settings_body(
        &mut self,
        window: &mut Window,
        item: &revo_core::models::SourceFilterItem,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let mut body = div()
            .flex()
            .flex_col()
            .gap_2()
            .px_2()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(self.theme.border)
            .bg(self.theme.surface);
        let Some(draft) = self.filter_settings.clone() else {
            return body.child(
                div()
                    .text_color(self.theme.text_muted)
                    .text_size(px(11.))
                    .child(SharedString::from(format!(
                        "Loading filter settings for '{}'… (mock builds have no filter list)",
                        item.name
                    ))),
            );
        };
        if !draft.available {
            return body.child(
                div()
                    .text_color(self.theme.text_muted)
                    .text_size(px(11.))
                    .child("Filter settings are unavailable for this build."),
            );
        }
        for index in 0..draft.drafts.len() {
            let (spec, fd) = draft.drafts[index].clone();
            if spec.kind == "info" {
                body = body.child(
                    div()
                        .text_color(self.theme.text_muted)
                        .text_size(px(10.))
                        .child(SharedString::from(spec.label.clone())),
                );
                continue;
            }
            let field = self.filter_param_field(window, &spec, fd, index, cx);
            body = body.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(150.))
                            .flex_none()
                            .text_color(self.theme.text_secondary)
                            .text_size(px(11.))
                            .child(SharedString::from(spec.label.clone())),
                    )
                    .child(field),
            );
        }
        if draft.drafts.is_empty() {
            body = body.child(
                div()
                    .text_color(self.theme.text_muted)
                    .text_size(px(11.))
                    .child("This filter exposes no typed properties."),
            );
        }
        body
    }

    /// One filter-param field: bool toggle, list cycler, or text field with
    /// color/font picker triggers for the matching kinds.
    fn filter_param_field(
        &mut self,
        window: &mut Window,
        spec: &SourcePropertySpec,
        fd: ParamDraft,
        param_index: usize,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let field_index = FILTER_PARAM_SLOT_BASE + param_index;
        let trigger_source = self.source_id.clone();
        let trigger_filter = self
            .filter_settings
            .as_ref()
            .map(|draft| draft.filter_name.clone())
            .unwrap_or_default();
        let trigger_kind = spec.kind.clone();
        let trigger_key = spec.key.clone();
        match fd {
            ParamDraft::Bool(enabled) => div()
                .id(SharedString::from(format!("filter-param-{field_index}")))
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
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some((_, ParamDraft::Bool(enabled))) = this
                        .filter_settings
                        .as_mut()
                        .and_then(|draft| draft.drafts.get_mut(param_index))
                    {
                        *enabled = !*enabled;
                    }
                    cx.notify();
                }))
                .child(if enabled { "On" } else { "Off" })
                .into_any_element(),
            ParamDraft::List { value, options } => {
                let label_text = options
                    .iter()
                    .find(|(option_value, _)| option_value == &value)
                    .map(|(_, label)| label.clone())
                    .unwrap_or_else(|| value.clone());
                let list_options = options.clone();
                let current = value.clone();
                div()
                    .id(SharedString::from(format!("filter-param-{field_index}")))
                    .cursor_pointer()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(self.theme.surface_active)
                    .text_color(self.theme.text)
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(SharedString::from(label_text))
                    .child(crate::components::icon::icon(
                        self.theme.text_muted,
                        "chevron-down",
                        12.,
                    ))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |_, event: &gpui::MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            cx.emit(ComponentEvent::OpenDropdown {
                                field: DropdownField::FilterParam {
                                    source_id: trigger_source.clone(),
                                    filter_name: trigger_filter.clone(),
                                    key: trigger_key.clone(),
                                },
                                options: list_options.clone(),
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
            ParamDraft::Text(value) => {
                let focused = self.focused_field == Some(field_index);
                let id: &'static str =
                    Box::leak(format!("filter-param-text-{field_index}").into_boxed_str());
                let kind = trigger_kind.clone();
                // Filter params never carry font-object properties; only
                // color kinds anchor the shared picker here (hex stays
                // directly editable).
                if kind == "color" {
                    let trigger_current = if value.is_empty() {
                        "#FFFFFF".to_string()
                    } else {
                        value.clone()
                    };
                    let row = div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .flex_1()
                        .child(self.text_field(
                            id,
                            focused,
                            value.clone(),
                            true,
                            field_index,
                            window,
                            cx,
                        ))
                        .child(
                            div()
                                .id(SharedString::from(format!(
                                    "filter-param-picker-{field_index}"
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
                                .child("Pick…")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |_, event: &gpui::MouseDownEvent, _, cx| {
                                        cx.stop_propagation();
                                        cx.emit(ComponentEvent::OpenColorPicker {
                                            field: ColorField::FilterParam {
                                                source_id: trigger_source.clone(),
                                                filter_name: trigger_filter.clone(),
                                                key: trigger_key.clone(),
                                            },
                                            current: trigger_current.clone(),
                                            position: (
                                                f32::from(event.position.x),
                                                f32::from(event.position.y),
                                            ),
                                        });
                                    }),
                                ),
                        );
                    row.into_any_element()
                } else {
                    self.text_field(id, focused, value, false, field_index, window, cx)
                        .into_any_element()
                }
            }
        }
    }

    /// The editable name row (editor mode).
    fn name_row(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let focused = self.focused_field == Some(NAME_FIELD_SLOT);
        let box_ = div()
            .id("source-editor-name")
            .cursor_pointer()
            .w_full()
            .px_2()
            .py_1()
            .rounded_md()
            .relative()
            .border_1()
            .border_color(if focused {
                self.theme.accent
            } else {
                self.theme.border_strong
            })
            .bg(self.theme.surface_elevated)
            .text_color(if self.name_draft.is_empty() {
                self.theme.text_muted
            } else {
                self.theme.text
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    this.focus_slot(NAME_FIELD_SLOT, event, window);
                    cx.notify();
                }),
            )
            .child(self.slot_display(
                NAME_FIELD_SLOT,
                window,
                self.name_draft.clone(),
                Some("Source name…"),
            ))
            .child(self.bounds_probe(NAME_FIELD_SLOT));
        box_
    }
}

impl Render for SourcePropertiesModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let error_note = self.error.clone().unwrap_or_default();
        let notice = self.notice.clone().unwrap_or_default();

        let config = ModalConfig::standard(
            if self.editor {
                format!("Edit Source - {}", self.source_name)
            } else {
                format!("Properties - {}", self.source_name)
            },
            ModalSize::Medium,
        );
        let theme = self.theme;

        let fit_button = div()
            .id("source-properties-fit")
            .cursor_pointer()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(theme.surface_active)
            .text_color(theme.text)
            .on_click(cx.listener(|this, _, _, cx| this.fit_to_canvas(cx)))
            .child("Fit to canvas");
        let stretch_button = div()
            .id("source-properties-stretch")
            .cursor_pointer()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(theme.surface_active)
            .text_color(theme.text)
            .on_click(cx.listener(|this, _, _, cx| this.stretch_to_canvas(cx)))
            .child("Stretch to canvas");

        let footer = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .flex_wrap()
                    .child(fit_button)
                    .child(stretch_button)
                    .when(!self.editor, |row| {
                        // Working hand-off to the full editor (Svelte parity:
                        // "Open the full source editor").
                        row.child(
                            div()
                                .id("source-properties-editor")
                                .cursor_pointer()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .bg(theme.surface_active)
                                .text_color(theme.text)
                                .on_click(cx.listener(|this, _, _, cx| this.open_editor(cx)))
                                .child("Open the full source editor"),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_color(if error_note.is_empty() {
                                theme.success
                            } else {
                                theme.danger
                            })
                            .text_size(px(11.))
                            .child(SharedString::from(if error_note.is_empty() {
                                notice
                            } else {
                                error_note
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap_2()
                            .child(
                                div()
                                    .id("source-properties-cancel")
                                    .cursor_pointer()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .bg(theme.surface_active)
                                    .text_color(theme.text)
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(SourcePropertiesEvent::Cancelled);
                                    }))
                                    .child("Close"),
                            )
                            .child(if self.editor {
                                div()
                                    .id("source-properties-apply-full")
                                    .cursor_pointer()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .bg(theme.accent)
                                    .text_color(theme.text_strong)
                                    .on_click(cx.listener(|this, _, _, cx| this.apply_full(cx)))
                                    .child("Apply")
                            } else {
                                div()
                                    .id("source-properties-apply")
                                    .cursor_pointer()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .bg(theme.accent)
                                    .text_color(theme.text_strong)
                                    .on_click(cx.listener(|this, _, _, cx| this.apply(cx)))
                                    .child("Apply")
                            }),
                    ),
            )
            .into_any_element();

        let on_key = cx.listener(|this, event: &KeyDownEvent, _, cx| {
            this.on_field_key(event, cx);
        });

        let mut header = div()
            .flex()
            .flex_row()
            .justify_between()
            .child(
                div()
                    .text_color(theme.text_secondary)
                    .text_size(px(11.))
                    .child(SharedString::from(self.source_type.clone())),
            )
            .child(
                div()
                    .text_color(theme.text_muted)
                    .text_size(px(11.))
                    .child(SharedString::from(format!("Canvas {}", self.canvas))),
            );

        // Editor mode mirrors the Svelte tab strip: General carries the
        // transform editor and the full property list; Filters/Other stay
        // documented placeholders.
        if self.editor {
            for (tab, label) in [
                (EditorTab::General, "General"),
                (EditorTab::Filters, "Filters"),
                (EditorTab::Other, "Other"),
            ] {
                let active = self.active_tab == tab;
                header = header.child(
                    div()
                        .id(SharedString::from(format!("source-editor-tab-{label}")))
                        .cursor_pointer()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(if active {
                            theme.surface_active
                        } else {
                            theme.surface
                        })
                        .border_1()
                        .border_color(if active { theme.accent } else { theme.border })
                        .text_color(if active {
                            theme.text_strong
                        } else {
                            theme.text_muted
                        })
                        .text_size(px(11.))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.active_tab = tab;
                            cx.notify();
                        })),
                );
            }
        }

        header = div().flex().flex_col().gap_1().child(header);

        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .size_full()
            .track_focus(&self.main_focus)
            .child(header);
        if self.editor {
            body = body.child(self.name_row(window, cx));
            match self.active_tab {
                EditorTab::General => {
                    body = body
                        .child(
                            div()
                                .text_color(theme.text_secondary)
                                .text_size(px(11.))
                                .child("Source properties"),
                        )
                        .child(self.property_rows(window, cx));
                }
                EditorTab::Filters => body = body.child(self.filters_body(window, cx)),
                EditorTab::Other => {
                    body = body.child(self.transform_rows(window, cx));
                }
            }
        } else {
            body = body.child(self.transform_rows(window, cx));
        }
        body = body.on_key_down(on_key);

        let close =
            crate::components::modal_shell::close_x_button("source-properties-close", self.theme)
                .on_click(cx.listener(|_, _, _, cx| cx.emit(SourcePropertiesEvent::Cancelled)))
                .into_any_element();

        modal_shell_with_body(&config, self.theme, body, Some(footer), Some(close))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(key: &str, kind: &str) -> SourcePropertySpec {
        SourcePropertySpec {
            key: key.into(),
            label: key.into(),
            kind: kind.into(),
            hint: String::new(),
            options: Vec::new(),
        }
    }

    #[test]
    fn rebuild_specs_swaps_the_nested_font_object_for_the_flat_pair() {
        let specs = vec![
            spec("font", "text"),
            spec("text", "text"),
            spec("color1", "color"),
        ];
        let rebuilt = rebuild_specs(specs);
        let keys: Vec<&str> = rebuilt.iter().map(|spec| spec.key.as_str()).collect();
        assert_eq!(keys, vec!["font_face", "font_size", "text", "color1"]);
        assert_eq!(rebuilt[0].kind, "font");
        assert_eq!(rebuilt[1].kind, "int");
        // Sources without the font property stay untouched.
        let untouched = rebuild_specs(vec![spec("text", "text")]);
        assert_eq!(untouched.len(), 1);
        assert_eq!(untouched[0].key, "text");
    }

    #[test]
    fn filter_param_values_validate_and_skip_the_font_pair() {
        let drafts = vec![
            (spec("brightness", "float"), ParamDraft::Text("0.25".into())),
            (spec("opacity", "int"), ParamDraft::Text("88".into())),
            (spec("font_face", "font"), ParamDraft::Text(String::new())),
            (spec("font_size", "int"), ParamDraft::Text(String::new())),
            (
                spec("key_color", "color"),
                ParamDraft::Text("#00FF00".into()),
            ),
            (spec("relative", "bool"), ParamDraft::Bool(true)),
        ];
        let values = collected_filter_param_values(&drafts).expect("valid drafts");
        assert_eq!(
            values,
            vec![
                ("brightness".into(), "0.25".into()),
                ("opacity".into(), "88".into()),
                ("key_color".into(), "#00FF00".into()),
                ("relative".into(), "true".into()),
            ]
        );
        // Invalid numerics surface the label.
        let broken = vec![(
            spec("opacity", "int"),
            ParamDraft::Text("not-a-number".into()),
        )];
        assert_eq!(
            collected_filter_param_values(&broken),
            Err("opacity must be a whole number".into())
        );
    }

    #[test]
    fn param_draft_values_cover_all_kinds() {
        assert_eq!(ParamDraft::Bool(true).value(), "true");
        assert_eq!(ParamDraft::Bool(false).value(), "false");
        assert_eq!(
            ParamDraft::List {
                value: "left".into(),
                options: vec![("left".into(), "Left".into())],
            }
            .value(),
            "left"
        );
        assert_eq!(ParamDraft::Text("hex".into()).value(), "hex");
    }
}

#[cfg(test)]
mod source_caret_tests {
    use super::*;
    use gpui::TestAppContext;

    fn placed() -> revo_core::service::PlacedSource {
        revo_core::service::PlacedSource {
            id: "src-1".into(),
            x: 1920,
            y: 0,
            width: 1920,
            height: 1080,
            rotation: 0,
        }
    }

    #[gpui::test]
    fn transform_slot_edits_at_the_caret(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let _ = window;
            SourcePropertiesModal::new(
                "src-1".into(),
                "Camera".into(),
                "display_capture".into(),
                Some(placed()),
                Some("1920x1080".into()),
                RevoTheme::future(),
                cx,
            )
        });
        cx.update(|window, cx| {
            entity.update(cx, |modal, cx| {
                modal.focused_field = Some(2);
                modal.main_focus.focus(window);
                cx.notify();
            });
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("left left x");
        cx.run_until_parked();
        entity.update(cx, |modal, _| {
            assert_eq!(
                modal.width.value, "19x20",
                "navigation then insert must edit inside the numeric draft"
            );
        });
    }

    #[gpui::test]
    fn backspace_deletes_left_of_the_caret(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let _ = window;
            SourcePropertiesModal::new(
                "src-1".into(),
                "Camera".into(),
                "display_capture".into(),
                Some(placed()),
                Some("1920x1080".into()),
                RevoTheme::future(),
                cx,
            )
        });
        cx.update(|window, cx| {
            entity.update(cx, |modal, cx| {
                modal.focused_field = Some(3); // height "1080"
                modal.main_focus.focus(window);
                cx.notify();
            });
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("home delete delete");
        cx.run_until_parked();
        entity.update(cx, |modal, _| {
            assert_eq!(modal.height.value, "80");
        });
    }

    #[gpui::test]
    fn editor_apply_success_emits_save_then_close(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let _ = window;
            SourcePropertiesModal::new_editor(
                "src-1".into(),
                "Text".into(),
                "text_gdiplus".into(),
                None,
                None,
                RevoTheme::future(),
                cx,
            )
        });
        let mut events = cx.events(&entity);
        entity.update(cx, |modal, cx| modal.apply_full(cx));
        cx.run_until_parked();
        let first = events
            .try_next()
            .expect("first event")
            .expect("save event emitted");
        assert!(
            matches!(first, SourcePropertiesEvent::AppliedFull { .. }),
            "save event must come first: {first:?}"
        );
        let second = events
            .try_next()
            .expect("second event")
            .expect("close event emitted");
        assert!(
            matches!(second, SourcePropertiesEvent::Cancelled),
            "a successful save must dismiss the modal (plan §14.4): {second:?}"
        );
    }

    #[gpui::test]
    fn fresh_editor_applies_with_seeded_zero_transform(cx: &mut TestAppContext) {
        // Regression (plan §14.0): an unplaced new source used to fail Apply
        // with "Position X must be a whole number" on empty drafts.
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let _ = window;
            SourcePropertiesModal::new_editor(
                "src-1".into(),
                "Text".into(),
                "text_gdiplus".into(),
                None,
                None,
                RevoTheme::future(),
                cx,
            )
        });
        entity.update(cx, |modal, cx| {
            modal.apply_full(cx);
            assert!(modal.error.is_none(), "unexpected error {:?}", modal.error);
        });
    }

    #[gpui::test]
    fn name_slot_routes_through_the_caret_session(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let _ = window;
            SourcePropertiesModal::new_editor(
                "src-1".into(),
                "Text".into(),
                "text_gdiplus".into(),
                None,
                None,
                RevoTheme::future(),
                cx,
            )
        });
        cx.update(|window, cx| {
            entity.update(cx, |modal, cx| {
                modal.name_draft = "Title".into();
                modal.focused_field = Some(NAME_FIELD_SLOT);
                modal.main_focus.focus(window);
                cx.notify();
            });
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("home right X");
        cx.run_until_parked();
        entity.update(cx, |modal, _| {
            assert_eq!(modal.name_draft, "TXitle");
        });
    }
}
