//! Functional native Source Properties modal, matching the Svelte
//! `SourcePropertiesModal`: transform fields (size, position) seeded from the
//! backend snapshot, Fit/Stretch-to-canvas helpers computed with
//! `revo-core::planner`, and a hand-off to the full source editor. Applying
//! dispatches `BackendCommand::SetSourceTransform`.

use gpui::{
    div, prelude::*, px, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    MouseButton, Render, SharedString, Window,
};

use revo_core::planner;

use crate::{
    components::modal_shell::{modal_shell_with_body, ModalConfig, ModalSize},
    components::text_field::apply_line_keystroke,
    theme::RevoTheme,
};

#[derive(Clone, Debug, PartialEq)]
pub enum SourcePropertiesEvent {
    /// The parsed transform values ready for `SetSourceTransform`.
    Applied {
        id: String,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        rotation: i32,
    },
    /// Hand-off requested to the full source editor.
    OpenEditor {
        id: String,
    },
    Cancelled,
}

/// One numeric transform field draft.
#[derive(Clone, Debug, Default)]
struct NumberDraft {
    value: String,
}

impl NumberDraft {
    fn set(&mut self, value: i32) {
        self.value = value.to_string();
    }

    fn parsed(&self) -> Option<i32> {
        self.value.trim().parse::<i32>().ok()
    }
}

pub struct SourcePropertiesModal {
    pub theme: RevoTheme,
    pub source_id: String,
    pub source_name: String,
    pub source_type: String,
    /// Editor mode (Svelte EditSourceModal): "Edit Source" title with the
    /// General/Filters/Other tab strip; filters/other stay documented
    /// placeholders until the filter pipeline migrates.
    pub editor: bool,
    /// Typed transform drafts; values are private so the modal owns
    /// validation.
    pos_x: NumberDraft,
    pos_y: NumberDraft,
    width: NumberDraft,
    height: NumberDraft,
    rotation: NumberDraft,
    /// Scene canvas resolution (`WIDTHxHEIGHT`) used by Fit/Stretch; falls
    /// back to the planner default when settings carry none.
    pub canvas: String,
    pub error: Option<String>,
    pub notice: Option<String>,
    focused_field: Option<usize>,
    /// One focus handle per numeric field, created up-front.
    field_focus: Vec<FocusHandle>,
    main_focus: FocusHandle,
}

impl EventEmitter<SourcePropertiesEvent> for SourcePropertiesModal {}

impl Focusable for SourcePropertiesModal {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.main_focus.clone()
    }
}

impl SourcePropertiesModal {
    /// Seeds the modal from the snapshot's source item and placed transform.
    /// `canvas` is the configured scene resolution, when known.
    pub fn new(
        source_id: String,
        source_name: String,
        source_type: String,
        transform: Option<revo_core::service::PlacedSource>,
        canvas: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(
            source_id,
            source_name,
            source_type,
            transform,
            canvas,
            false,
            cx,
        )
    }

    /// Editor-mode constructor (the Svelte EditSource window).
    #[allow(clippy::too_many_arguments)]
    pub fn new_editor(
        source_id: String,
        source_name: String,
        source_type: String,
        transform: Option<revo_core::service::PlacedSource>,
        canvas: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(
            source_id,
            source_name,
            source_type,
            transform,
            canvas,
            true,
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
        cx: &mut Context<Self>,
    ) -> Self {
        let mut modal = Self {
            theme: RevoTheme::future(),
            source_id,
            source_name,
            source_type,
            editor,
            pos_x: NumberDraft::default(),
            pos_y: NumberDraft::default(),
            width: NumberDraft::default(),
            height: NumberDraft::default(),
            rotation: NumberDraft::default(),
            canvas: canvas.unwrap_or_else(|| "1920x1080".into()),
            error: None,
            notice: None,
            focused_field: None,
            field_focus: (0..5).map(|_| cx.focus_handle()).collect(),
            main_focus: cx.focus_handle(),
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

    /// The parsed transform, or an error message when a field is not a
    /// non-negative number.
    fn parsed_transform(&self) -> Result<revo_core::service::PlacedSource, String> {
        let parse = |field: &NumberDraft, label: &str| {
            field
                .parsed()
                .ok_or_else(|| format!("{label} must be a whole number"))
        };
        Ok(revo_core::service::PlacedSource {
            id: self.source_id.clone(),
            x: parse(&self.pos_x, "Position X")?,
            y: parse(&self.pos_y, "Position Y")?,
            width: parse(&self.width, "Width")?,
            height: parse(&self.height, "Height")?,
            rotation: parse(&self.rotation, "Rotation")?,
        })
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        match self.parsed_transform() {
            Ok(transform) => {
                self.error = None;
                self.notice = Some("Transform applied".into());
                cx.emit(SourcePropertiesEvent::Applied {
                    id: self.source_id.clone(),
                    x: transform.x,
                    y: transform.y,
                    width: transform.width,
                    height: transform.height,
                    rotation: transform.rotation,
                });
            }
            Err(error) => {
                self.error = Some(error);
                self.notice = None;
            }
        }
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
        let base_width = current.width.max(0) as u32;
        let base_height = current.height.max(0) as u32;
        let (new_width, new_height) = if stretch {
            match planner::stretch_scale(base_width, base_height, canvas_width, canvas_height) {
                Some((scale_x, scale_y)) => (
                    (base_width as f32 * scale_x).round() as i32,
                    (base_height as f32 * scale_y).round() as i32,
                ),
                None => (current.width, current.height),
            }
        } else {
            match planner::fit_scale(base_width, base_height, canvas_width, canvas_height) {
                Some(scale) => (
                    (base_width as f32 * scale).round() as i32,
                    (base_height as f32 * scale).round() as i32,
                ),
                None => (current.width, current.height),
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

    fn number_field(
        &self,
        label: &str,
        field_index: usize,
        value: String,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let focus = self
            .field_focus
            .get(field_index)
            .cloned()
            .unwrap_or_else(|| cx.focus_handle());
        let focused = self.focused_field == Some(field_index);
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
            .child(
                div()
                    .id(SharedString::from(format!(
                        "source-property-field-{field_index}"
                    )))
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
                            this.focused_field = Some(field_index);
                            focus.focus(window);
                            cx.notify();
                        }),
                    )
                    .child(if value.is_empty() {
                        SharedString::from("0")
                    } else {
                        SharedString::from(value)
                    }),
            )
    }

    fn on_field_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(index) = self.focused_field else {
            return;
        };
        let value = match index {
            0 => &mut self.pos_x.value,
            1 => &mut self.pos_y.value,
            2 => &mut self.width.value,
            3 => &mut self.height.value,
            _ => &mut self.rotation.value,
        };
        let keystroke = &event.keystroke;
        if let Some(next) = apply_line_keystroke(
            value,
            &keystroke.key,
            keystroke.key_char.as_deref(),
            keystroke.modifiers.control,
            keystroke.modifiers.alt,
            keystroke.modifiers.platform,
        ) {
            *value = next;
            self.notice = None;
        }
        cx.notify();
    }

    fn field_rows(&self, cx: &mut Context<Self>) -> gpui::Div {
        let pos_row = div()
            .flex()
            .flex_row()
            .gap_2()
            .child(self.number_field("Position X", 0, self.pos_x.value.clone(), cx))
            .child(self.number_field("Position Y", 1, self.pos_y.value.clone(), cx));
        let size_row = div()
            .flex()
            .flex_row()
            .gap_2()
            .child(self.number_field("Width", 2, self.width.value.clone(), cx))
            .child(self.number_field("Height", 3, self.height.value.clone(), cx))
            .child(self.number_field("Rotation", 4, self.rotation.value.clone(), cx));
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(pos_row)
            .child(size_row)
    }
}

impl Render for SourcePropertiesModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
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
        let editor_button = div()
            .id("source-properties-editor")
            .cursor_pointer()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(theme.surface_active)
            .text_color(theme.text)
            .on_click(cx.listener(|this, _, _, cx| this.open_editor(cx)))
            .child("Open the full source editor");

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
                    .child(editor_button),
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
                            .child(
                                div()
                                    .id("source-properties-apply")
                                    .cursor_pointer()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .bg(theme.accent)
                                    .text_color(theme.text_strong)
                                    .on_click(cx.listener(|this, _, _, cx| this.apply(cx)))
                                    .child("Apply"),
                            ),
                    ),
            )
            .into_any_element();

        let on_key = cx.listener(|this, event: &KeyDownEvent, _, cx| {
            this.on_field_key(event, cx);
        });

        let close = div()
            .id("source-properties-close")
            .cursor_pointer()
            .text_color(self.theme.text_muted)
            .child("Close")
            .on_click(cx.listener(|_, _, _, cx| cx.emit(SourcePropertiesEvent::Cancelled)))
            .into_any_element();

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
        // transform editor; Filters/Other stay documented placeholders.
        if self.editor {
            header = header.child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(theme.surface_active)
                            .text_color(theme.text_strong)
                            .text_size(px(11.))
                            .child("General"),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .text_color(theme.text_muted)
                            .text_size(px(11.))
                            .child("Filters"),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .text_color(theme.text_muted)
                            .text_size(px(11.))
                            .child("Other"),
                    ),
            );
        }

        header = div().flex().flex_col().gap_1().child(header);

        modal_shell_with_body(
            &config,
            self.theme,
            div()
                .flex()
                .flex_col()
                .gap_3()
                .size_full()
                .track_focus(&self.main_focus)
                .child(header)
                .child(self.field_rows(cx))
                .on_key_down(on_key),
            Some(footer),
            Some(close),
        )
    }
}
