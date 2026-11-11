use gpui::{
    div, prelude::*, px, ClickEvent, Context, IntoElement, MouseButton, MouseDownEvent, Render,
    SharedString, Window,
};

use crate::{
    actions::{ComponentEvent, ContextMenuTarget, MoveDirection, QuickSourceAction},
    theme::RevoTheme,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceItem {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub visible: bool,
    pub audio_capable: bool,
    pub locked: bool,
}

pub struct SourcesPanel {
    pub sources: Vec<SourceItem>,
    pub selected_id: Option<String>,
    pub theme: RevoTheme,
}

impl gpui::EventEmitter<ComponentEvent> for SourcesPanel {}

impl SourcesPanel {
    pub fn select(&mut self, id: &str) -> Option<ComponentEvent> {
        self.sources.iter().any(|source| source.id == id).then(|| {
            self.selected_id = Some(id.to_owned());
            ComponentEvent::SelectSource(id.to_owned())
        })
    }

    pub fn toggle_visibility(&mut self, id: &str) -> Option<ComponentEvent> {
        let source = self.sources.iter_mut().find(|source| source.id == id)?;
        source.visible = !source.visible;
        Some(ComponentEvent::ToggleSourceVisibility(id.to_owned()))
    }

    pub fn toggle_lock(&mut self, id: &str) -> Option<ComponentEvent> {
        let source = self.sources.iter_mut().find(|source| source.id == id)?;
        source.locked = !source.locked;
        Some(ComponentEvent::ToggleSourceLock(id.to_owned()))
    }

    pub fn move_source(&mut self, id: &str, direction: MoveDirection) -> Option<ComponentEvent> {
        let from = self.sources.iter().position(|source| source.id == id)?;
        if self.sources[from].locked {
            return None;
        }
        let last = self.sources.len().saturating_sub(1);
        let to = match direction {
            MoveDirection::Up => (from + 1).min(last),
            MoveDirection::Down => from.saturating_sub(1),
            MoveDirection::Top => last,
            MoveDirection::Bottom => 0,
        };
        if from != to {
            let source = self.sources.remove(from);
            self.sources.insert(to, source);
        }
        Some(ComponentEvent::MoveSource {
            id: id.to_owned(),
            direction,
        })
    }

    /// The contextual quick action plus its user-facing label, mirroring the
    /// Svelte header (`Interact` / `Text Edit` / `Change color` / `Select
    /// file` / device label).
    pub fn contextual_action(&self) -> Option<(ComponentEvent, &'static str)> {
        let id = self.selected_id.as_ref()?.clone();
        let action = self.quick_action_for(&id)?;
        let label = match action {
            QuickSourceAction::Interact => "Interact",
            QuickSourceAction::EditText => "Edit text",
            QuickSourceAction::ChangeColor => "Change color",
            QuickSourceAction::SelectFile => "Select image",
            QuickSourceAction::SelectDevice => "Select device",
        };
        Some((
            ComponentEvent::QuickSourceAction {
                id: id.clone(),
                action,
            },
            label,
        ))
    }

    /// Quick action implied by one source's type; shared by the header
    /// "Quick action" button and the right-click context menu.
    pub fn quick_action_for(&self, id: &str) -> Option<QuickSourceAction> {
        let source = self.sources.iter().find(|source| source.id == id)?;
        let kind = source.source_type.to_ascii_lowercase();
        Some(if kind == "browser_source" {
            QuickSourceAction::Interact
        } else if kind.contains("text") {
            QuickSourceAction::EditText
        } else if kind.contains("color_source") {
            QuickSourceAction::ChangeColor
        } else if kind.contains("image") || kind.contains("slideshow") {
            QuickSourceAction::SelectFile
        } else {
            QuickSourceAction::SelectDevice
        })
    }

    /// Right-click request for one source row; positions the shared
    /// workspace context menu at the cursor.
    pub fn open_menu(&self, id: &str, position: (f32, f32)) -> Option<ComponentEvent> {
        let source = self.sources.iter().find(|source| source.id == id)?;
        Some(ComponentEvent::OpenContextMenu {
            target: ContextMenuTarget::Source {
                id: source.id.clone(),
                name: source.name.clone(),
                visible: source.visible,
                locked: source.locked,
            },
            position,
        })
    }
}

/// Audio capture type prefixes, mirrored from the Svelte
/// `sourceClassification.ts` module.
const AUDIO_CAPTURE_TYPE_PREFIXES: [&str; 4] = ["wasapi_", "pipewire_", "pulse_", "alsa_"];

fn is_audio_capture_source_type(source_type: &str) -> bool {
    let value = source_type.to_ascii_lowercase();
    AUDIO_CAPTURE_TYPE_PREFIXES
        .iter()
        .any(|prefix| value.starts_with(prefix))
        || value.contains("audio_input")
        || value.contains("audio_output")
        || value.contains("audio_capture")
        || value.contains("audio_line")
}

fn is_video_like_source_type(source_type: &str) -> bool {
    let value = source_type.to_ascii_lowercase();
    value.contains("video")
        || value.contains("v4l2")
        || value.contains("camera")
        || value.contains("webcam")
        || value.contains("dshow")
        || value.contains("window")
        || value.contains("xcomposite")
        || value.contains("display")
        || value.contains("screen")
        || value.contains("game_capture")
}

/// Maps an OBS source type onto the shell's icon vocabulary, mirroring the
/// Svelte `sourceTypeIconPath` mapping (check order matters).
pub(crate) fn source_type_icon(source_type: &str, audio_capable: bool) -> &'static str {
    let kind = source_type.to_ascii_lowercase();
    if kind.contains("browser") {
        "browser"
    } else if kind.contains("image") || kind.contains("slideshow") {
        "image"
    } else if kind.contains("text") {
        "text-icon"
    } else if kind.contains("color") {
        "color"
    } else if kind.contains("game") {
        "game"
    } else if kind.contains("camera")
        || kind.contains("webcam")
        || kind.contains("video_capture")
        || kind.contains("v4l2")
        || kind == "video"
    {
        "camera"
    } else if kind.contains("pulse")
        || kind.contains("pipewire")
        || kind.contains("alsa")
        || kind.contains("audio")
    {
        "audio"
    } else if kind.contains("ffmpeg") || kind.contains("media") {
        "media"
    } else if !is_video_like_source_type(&kind)
        && (audio_capable || is_audio_capture_source_type(&kind))
    {
        "audio"
    } else if kind.contains("window")
        || kind.contains("xcomposite")
        || kind.contains("display")
        || kind.contains("screen")
        || kind.contains("capture")
        || kind == "capture"
    {
        "display"
    } else {
        "source"
    }
}

impl Render for SourcesPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let mut list = div().flex().flex_col().gap_1();
        for source in self.sources.clone().into_iter().rev() {
            let selected = self.selected_id.as_deref() == Some(&source.id);
            let id = source.id.clone();
            let visible_id = id.clone();
            let lock_id = id.clone();
            let edit_id = id.clone();
            let up_id = id.clone();
            let down_id = id.clone();
            let menu_id = id.clone();
            list = list.child(
                div()
                    .id(SharedString::from(format!("source-row-{id}")))
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1p5()
                    // Same row height as the Scenes panel rows (which carry
                    // the 72x40 thumbnails).
                    .min_h(px(48.))
                    .bg(if selected {
                        self.theme.surface_selected
                    } else {
                        self.theme.surface
                    })
                    .border_1()
                    .border_color(if selected {
                        self.theme.border_selected
                    } else {
                        self.theme.border
                    })
                    .rounded_lg()
                    // Plan §15.1: single click selects, double click opens
                    // the source editor (Svelte parity).
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                        if event.click_count() >= 2 {
                            cx.emit(ComponentEvent::EditSource(id.clone()));
                            return;
                        }
                        if let Some(event) = this.select(&id) {
                            cx.emit(event);
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            let position =
                                (f32::from(event.position.x), f32::from(event.position.y));
                            if let Some(event) = this.open_menu(&menu_id, position) {
                                cx.emit(event);
                            }
                            cx.notify();
                        }),
                    )
                    .child(crate::components::icon::icon(
                        if selected {
                            self.theme.text_strong
                        } else {
                            self.theme.text_muted
                        },
                        source_type_icon(&source.source_type, source.audio_capable),
                        14.,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(source.name.clone()),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("source-up-{up_id}")))
                            .when(!source.locked, |control| control.cursor_pointer())
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(self.theme.surface_active)
                            .text_color(if source.locked {
                                self.theme.text_disabled
                            } else {
                                self.theme.text
                            })
                            .child(crate::components::icon::icon(
                                if source.locked {
                                    self.theme.text_disabled
                                } else {
                                    self.theme.text
                                },
                                "chevron-up",
                                12.,
                            ))
                            .when(!source.locked, |control| {
                                control.on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(event) = this.move_source(&up_id, MoveDirection::Up)
                                    {
                                        cx.emit(event);
                                        cx.notify();
                                    }
                                }))
                            }),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("source-down-{down_id}")))
                            .when(!source.locked, |control| control.cursor_pointer())
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(self.theme.surface_active)
                            .text_color(if source.locked {
                                self.theme.text_disabled
                            } else {
                                self.theme.text
                            })
                            .child(crate::components::icon::icon(
                                if source.locked {
                                    self.theme.text_disabled
                                } else {
                                    self.theme.text
                                },
                                "chevron-down",
                                12.,
                            ))
                            .when(!source.locked, |control| {
                                control.on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(event) =
                                        this.move_source(&down_id, MoveDirection::Down)
                                    {
                                        cx.emit(event);
                                        cx.notify();
                                    }
                                }))
                            }),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("source-visible-{visible_id}")))
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(self.theme.surface_active)
                            .child(crate::components::icon::icon(
                                self.theme.text,
                                if source.visible { "eye" } else { "eye-off" },
                                13.,
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                if let Some(event) = this.toggle_visibility(&visible_id) {
                                    cx.emit(event);
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("source-lock-{lock_id}")))
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(self.theme.surface_active)
                            .child(crate::components::icon::icon(
                                self.theme.text,
                                if source.locked { "lock" } else { "unlock" },
                                13.,
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                if let Some(event) = this.toggle_lock(&lock_id) {
                                    cx.emit(event);
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("source-edit-{edit_id}")))
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(self.theme.accent)
                            .child(crate::components::icon::icon(
                                self.theme.text_strong,
                                "pencil",
                                13.,
                            ))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.stop_propagation();
                                cx.emit(ComponentEvent::EditSource(edit_id.clone()));
                            })),
                    ),
            );
        }
        // Svelte header parity: the contextual action (labeled with what it
        // does — Interact / Text Edit / Change color / Select file / Select
        // device) sits before the "+" add button.
        let action_button = self.contextual_action().map(|(event, label)| {
            div()
                .id("sources-contextual-action")
                .cursor_pointer()
                .px_2()
                .py_1()
                .flex()
                .items_center()
                .gap_1()
                .rounded_md()
                .bg(self.theme.surface_active)
                .child(label)
                .on_click(cx.listener(move |_, _, _, cx| cx.emit(event.clone())))
        });
        let header = div()
            .flex()
            .justify_between()
            .items_center()
            .child(div().text_color(self.theme.text_strong).child("Sources"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .children(action_button)
                    .child(
                        div()
                            .id("sources-add")
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .flex()
                            .items_center()
                            .gap_1()
                            .rounded_md()
                            .bg(self.theme.accent)
                            .child(crate::components::icon::icon(
                                self.theme.text_strong,
                                "plus",
                                12.,
                            ))
                            .child("Add source")
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(ComponentEvent::AddSource)),
                            ),
                    ),
            );
        div()
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .text_color(self.theme.text)
            .child(header)
            .child(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_state_and_context_action_are_typed() {
        let mut panel = SourcesPanel {
            sources: vec![SourceItem {
                id: "text-1".into(),
                name: "Lower third".into(),
                source_type: "text_ft2_source_v2".into(),
                visible: true,
                audio_capable: false,
                locked: false,
            }],
            selected_id: None,
            theme: RevoTheme::future(),
        };
        panel.select("text-1");
        assert_eq!(
            panel.toggle_visibility("text-1"),
            Some(ComponentEvent::ToggleSourceVisibility("text-1".into()))
        );
        assert_eq!(
            panel.contextual_action(),
            Some((
                ComponentEvent::QuickSourceAction {
                    id: "text-1".into(),
                    action: QuickSourceAction::EditText
                },
                "Edit text"
            ))
        );
    }

    #[test]
    fn right_click_requests_a_typed_source_context_menu() {
        let panel = SourcesPanel {
            sources: vec![SourceItem {
                id: "camera".into(),
                name: "Camera".into(),
                source_type: "v4l2_input".into(),
                visible: false,
                audio_capable: false,
                locked: true,
            }],
            selected_id: None,
            theme: RevoTheme::future(),
        };
        assert_eq!(
            panel.open_menu("camera", (12.0, 24.0)),
            Some(ComponentEvent::OpenContextMenu {
                target: ContextMenuTarget::Source {
                    id: "camera".into(),
                    name: "Camera".into(),
                    visible: false,
                    locked: true,
                },
                position: (12.0, 24.0),
            })
        );
        assert_eq!(
            panel.quick_action_for("camera"),
            Some(QuickSourceAction::SelectDevice)
        );
        assert_eq!(panel.open_menu("missing", (0.0, 0.0)), None);
        assert_eq!(panel.quick_action_for("missing"), None);
    }

    #[test]
    fn source_move_updates_backend_order_used_by_reversed_rendering() {
        let mut panel = SourcesPanel {
            sources: vec![
                SourceItem {
                    id: "bottom".into(),
                    name: "Bottom".into(),
                    source_type: "image_source".into(),
                    visible: true,
                    audio_capable: false,
                    locked: false,
                },
                SourceItem {
                    id: "top".into(),
                    name: "Top".into(),
                    source_type: "image_source".into(),
                    visible: true,
                    audio_capable: false,
                    locked: false,
                },
            ],
            selected_id: None,
            theme: RevoTheme::future(),
        };

        assert_eq!(
            panel.move_source("bottom", MoveDirection::Up),
            Some(ComponentEvent::MoveSource {
                id: "bottom".into(),
                direction: MoveDirection::Up,
            })
        );
        assert_eq!(panel.sources[1].id, "bottom");
    }
}
