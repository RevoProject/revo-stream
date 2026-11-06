use gpui::{div, prelude::*, Context, IntoElement, Render, SharedString, Window};

use crate::{
    actions::{ComponentEvent, MoveDirection, QuickSourceAction},
    components::panel,
    theme::RevoTheme,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceItem {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub visible: bool,
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

    pub fn contextual_action(&self) -> Option<ComponentEvent> {
        let source = self
            .sources
            .iter()
            .find(|source| Some(&source.id) == self.selected_id.as_ref())?;
        let kind = source.source_type.to_ascii_lowercase();
        let action = if kind == "browser_source" {
            QuickSourceAction::Interact
        } else if kind.contains("text") {
            QuickSourceAction::EditText
        } else if kind.contains("color_source") {
            QuickSourceAction::ChangeColor
        } else if kind.contains("image") || kind.contains("slideshow") {
            QuickSourceAction::SelectFile
        } else {
            QuickSourceAction::SelectDevice
        };
        Some(ComponentEvent::QuickSourceAction {
            id: source.id.clone(),
            action,
        })
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
            list = list.child(
                div()
                    .id(SharedString::from(format!("source-row-{id}")))
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .p_2()
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
                    .rounded_md()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(event) = this.select(&id) {
                            cx.emit(event);
                            cx.notify();
                        }
                    }))
                    .child(div().flex_1().child(source.name.clone()))
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
                            .child("Up")
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
                            .child("Down")
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
                            .bg(self.theme.surface_active)
                            .child(if source.visible { "Visible" } else { "Hidden" })
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
                            .bg(self.theme.surface_active)
                            .child(if source.locked { "Locked" } else { "Unlocked" })
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
                            .bg(self.theme.accent)
                            .child("Edit")
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.stop_propagation();
                                cx.emit(ComponentEvent::EditSource(edit_id.clone()));
                            })),
                    ),
            );
        }
        let mut header = div().flex().justify_between().child("Sources").child(
            div()
                .id("sources-add")
                .cursor_pointer()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(self.theme.accent)
                .child("Add source")
                .on_click(cx.listener(|_, _, _, cx| cx.emit(ComponentEvent::AddSource))),
        );
        if let Some(event) = self.contextual_action() {
            header = header.child(
                div()
                    .id("sources-contextual-action")
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(self.theme.surface_active)
                    .child("Quick action")
                    .on_click(cx.listener(move |_, _, _, cx| cx.emit(event.clone()))),
            );
        }
        panel(self.theme).child(header).child(list)
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
            Some(ComponentEvent::QuickSourceAction {
                id: "text-1".into(),
                action: QuickSourceAction::EditText
            })
        );
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
                    locked: false,
                },
                SourceItem {
                    id: "top".into(),
                    name: "Top".into(),
                    source_type: "image_source".into(),
                    visible: true,
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
