use gpui::{
    div, prelude::*, Context, IntoElement, MouseButton, MouseDownEvent, Render, SharedString,
    Window,
};

use crate::{
    actions::{ComponentEvent, ContextMenuTarget},
    components::panel,
    theme::RevoTheme,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneItem {
    pub name: String,
    pub active: bool,
    pub locked: bool,
}

pub struct ScenesPanel {
    pub scenes: Vec<SceneItem>,
    pub preview_scene: Option<String>,
    pub studio_mode: bool,
    pub rename_draft: Option<(String, String)>,
    pub theme: RevoTheme,
}

impl gpui::EventEmitter<ComponentEvent> for ScenesPanel {}

impl ScenesPanel {
    pub fn select(&mut self, name: &str) -> Option<ComponentEvent> {
        if !self.scenes.iter().any(|scene| scene.name == name) {
            return None;
        }
        for scene in &mut self.scenes {
            scene.active = scene.name == name;
        }
        Some(ComponentEvent::SelectScene(name.to_owned()))
    }

    pub fn begin_rename(&mut self, name: &str) -> bool {
        let Some(scene) = self
            .scenes
            .iter()
            .find(|scene| scene.name == name && !scene.locked)
        else {
            return false;
        };
        self.rename_draft = Some((scene.name.clone(), scene.name.clone()));
        true
    }

    pub fn commit_rename(&mut self) -> Option<ComponentEvent> {
        let (from, to) = self.rename_draft.take()?;
        let to = to.trim().to_owned();
        if to.is_empty()
            || self
                .scenes
                .iter()
                .any(|scene| scene.name == to && scene.name != from)
        {
            return None;
        }
        self.scenes
            .iter_mut()
            .find(|scene| scene.name == from)?
            .name = to.clone();
        Some(ComponentEvent::RenameScene { from, to })
    }

    /// Right-click request for one scene row; positions the shared workspace
    /// context menu at the cursor.
    pub fn open_menu(&self, name: &str, position: (f32, f32)) -> Option<ComponentEvent> {
        let scene = self.scenes.iter().find(|scene| scene.name == name)?;
        Some(ComponentEvent::OpenContextMenu {
            target: ContextMenuTarget::Scene {
                name: scene.name.clone(),
                locked: scene.locked,
            },
            position,
        })
    }
}

impl Render for ScenesPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let mut list = div().flex().flex_col().gap_1();
        for scene in self.scenes.clone() {
            let preview = self.studio_mode && self.preview_scene.as_deref() == Some(&scene.name);
            let background = if scene.active {
                self.theme.surface_active
            } else {
                self.theme.surface_elevated
            };
            list = list.child(
                div()
                    .id(SharedString::from(format!("scene-row-{}", scene.name)))
                    .cursor_pointer()
                    .p_2()
                    .bg(background)
                    .border_1()
                    .border_color(if preview {
                        self.theme.accent
                    } else {
                        self.theme.border
                    })
                    .rounded_md()
                    .on_click(cx.listener({
                        let name = scene.name.clone();
                        move |this, _, _, cx| {
                            if let Some(event) = this.select(&name) {
                                cx.emit(event);
                                cx.notify();
                            }
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener({
                            let name = scene.name.clone();
                            move |this, event: &MouseDownEvent, _, cx| {
                                let position =
                                    (f32::from(event.position.x), f32::from(event.position.y));
                                if let Some(event) = this.open_menu(&name, position) {
                                    cx.emit(event);
                                }
                                cx.notify();
                            }
                        }),
                    )
                    .child(SharedString::from(format!(
                        "{}{}{}",
                        scene.name,
                        if scene.active { " [active]" } else { "" },
                        if scene.locked { " [locked]" } else { "" }
                    ))),
            );
        }
        panel(self.theme)
            .child(
                div().flex().justify_between().child("Scenes").child(
                    div()
                        .id("scenes-add")
                        .flex()
                        .items_center()
                        .gap_1()
                        .cursor_pointer()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(self.theme.accent)
                        .child(crate::components::icon::icon(
                            self.theme.text_strong,
                            "plus",
                            12.,
                        ))
                        .child("Add scene")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(ComponentEvent::AddScene))),
                ),
            )
            .child(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_selection_updates_active_scene() {
        let mut panel = ScenesPanel {
            scenes: vec![
                SceneItem {
                    name: "A".into(),
                    active: true,
                    locked: false,
                },
                SceneItem {
                    name: "B".into(),
                    active: false,
                    locked: false,
                },
            ],
            preview_scene: None,
            studio_mode: false,
            rename_draft: None,
            theme: RevoTheme::classic(),
        };
        assert_eq!(
            panel.select("B"),
            Some(ComponentEvent::SelectScene("B".into()))
        );
        assert_eq!(
            panel.scenes.iter().find(|scene| scene.active).unwrap().name,
            "B"
        );
    }

    #[test]
    fn rename_normalizes_whitespace_and_rejects_duplicates() {
        let mut panel = ScenesPanel {
            scenes: vec![
                SceneItem {
                    name: "A".into(),
                    active: true,
                    locked: false,
                },
                SceneItem {
                    name: "B".into(),
                    active: false,
                    locked: false,
                },
            ],
            preview_scene: None,
            studio_mode: false,
            rename_draft: Some(("A".into(), "  B  ".into())),
            theme: RevoTheme::classic(),
        };
        assert_eq!(panel.commit_rename(), None);
    }

    #[test]
    fn right_click_requests_a_typed_scene_context_menu() {
        let panel = ScenesPanel {
            scenes: vec![SceneItem {
                name: "A".into(),
                active: true,
                locked: true,
            }],
            preview_scene: None,
            studio_mode: false,
            rename_draft: None,
            theme: RevoTheme::classic(),
        };
        assert_eq!(
            panel.open_menu("A", (40.0, 60.0)),
            Some(ComponentEvent::OpenContextMenu {
                target: ContextMenuTarget::Scene {
                    name: "A".into(),
                    locked: true,
                },
                position: (40.0, 60.0),
            })
        );
        assert_eq!(panel.open_menu("missing", (0.0, 0.0)), None);
    }
}
