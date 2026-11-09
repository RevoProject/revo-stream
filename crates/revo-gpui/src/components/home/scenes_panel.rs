use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    div, prelude::*, px, rgb, Context, Image as GpuiImage, IntoElement, MouseButton,
    MouseDownEvent, Render, SharedString, Window,
};

use crate::{
    actions::{ComponentEvent, ContextMenuTarget},
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
    /// Tiny per-scene preview frames (Svelte `.scene-thumb` parity); filled
    /// by the workspace's slow round-robin capture loop.
    pub thumbnails: HashMap<String, Arc<GpuiImage>>,
    /// Round-robin cursor for the thumbnail refresh cycle.
    pub thumbnail_cursor: usize,
    pub theme: RevoTheme,
}

impl gpui::EventEmitter<ComponentEvent> for ScenesPanel {}

impl ScenesPanel {
    pub fn select(&mut self, name: &str) -> Option<ComponentEvent> {
        if !self.scenes.iter().any(|scene| scene.name == name) {
            return None;
        }
        // OBS studio behavior: in studio mode a click arms the PREVIEW
        // scene; the program keeps playing until a transition is taken.
        if self.studio_mode {
            self.preview_scene = Some(name.to_owned());
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

    /// Stores one decoded scene thumbnail.
    pub fn set_thumbnail(&mut self, scene: &str, image: Arc<GpuiImage>) {
        self.thumbnails.insert(scene.to_owned(), image);
    }

    /// The next scene whose thumbnail should refresh (round-robin over the
    /// current list); `None` when there are no scenes.
    pub fn next_thumbnail_target(&mut self) -> Option<String> {
        if self.scenes.is_empty() {
            return None;
        }
        let name = self.scenes[self.thumbnail_cursor % self.scenes.len()]
            .name
            .clone();
        self.thumbnail_cursor = (self.thumbnail_cursor + 1) % self.scenes.len();
        Some(name)
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
                self.theme.surface
            };
            list = list.child(
                div()
                    .id(SharedString::from(format!("scene-row-{}", scene.name)))
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1p5()
                    .bg(background)
                    .border_1()
                    .border_color(if preview {
                        self.theme.accent
                    } else {
                        self.theme.border
                    })
                    .rounded_lg()
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
                    .children(match self.thumbnails.get(&scene.name) {
                        Some(image) => Some(
                            // Svelte `.scene-thumb`: 72x40 16:9 box, cover fit.
                            div()
                                .w(px(72.))
                                .h(px(40.))
                                .flex_none()
                                .overflow_hidden()
                                .rounded_sm()
                                .border_1()
                                .border_color(self.theme.border)
                                .bg(rgb(0x000000))
                                .child(
                                    gpui::img(image.clone())
                                        .size_full()
                                        .object_fit(gpui::ObjectFit::Cover),
                                )
                                .into_any_element(),
                        ),
                        None => Some(
                            // Placeholder keeps the row height stable.
                            div()
                                .w(px(72.))
                                .h(px(40.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .border_1()
                                .border_color(self.theme.border)
                                .bg(self.theme.surface_elevated)
                                .child(crate::components::icon::icon(
                                    if scene.active {
                                        self.theme.text_strong
                                    } else {
                                        self.theme.text_muted
                                    },
                                    "layers",
                                    14.,
                                ))
                                .into_any_element(),
                        ),
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(SharedString::from(format!(
                                "{}{}{}",
                                scene.name,
                                if scene.active { " [active]" } else { "" },
                                if scene.locked { " [locked]" } else { "" }
                            ))),
                    ),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .text_color(self.theme.text)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_color(self.theme.text_strong).child("Scenes"))
                    .child(
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
    /// A valid 1x1 transparent PNG used as a thumbnail fixture.
    const TEST_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    use super::*;

    #[test]
    fn studio_mode_select_arms_the_preview_scene() {
        let mut panel = ScenesPanel {
            scenes: vec![SceneItem {
                name: "A".into(),
                active: true,
                locked: false,
            }],
            preview_scene: None,
            studio_mode: true,
            rename_draft: None,
            thumbnails: Default::default(),
            thumbnail_cursor: 0,
            theme: RevoTheme::classic(),
        };
        assert_eq!(panel.select("A"), None);
        assert_eq!(panel.preview_scene.as_deref(), Some("A"));
        // The program stays on the previously active scene.
        assert!(panel.scenes.iter().all(|scene| scene.active));
    }

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
            thumbnails: Default::default(),
            thumbnail_cursor: 0,
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
            thumbnails: Default::default(),
            thumbnail_cursor: 0,
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
            thumbnails: Default::default(),
            thumbnail_cursor: 0,
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

    #[test]
    fn thumbnails_round_robin_over_scenes() {
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
            thumbnails: Default::default(),
            thumbnail_cursor: 0,
            theme: RevoTheme::classic(),
        };
        assert_eq!(panel.next_thumbnail_target().as_deref(), Some("A"));
        assert_eq!(panel.next_thumbnail_target().as_deref(), Some("B"));
        // The cycle wraps.
        assert_eq!(panel.next_thumbnail_target().as_deref(), Some("A"));
        panel.set_thumbnail(
            "A",
            std::sync::Arc::new(gpui::Image::from_bytes(
                gpui::ImageFormat::Png,
                TEST_PNG.to_vec(),
            )),
        );
        assert!(panel.thumbnails.contains_key("A"));
    }
}
