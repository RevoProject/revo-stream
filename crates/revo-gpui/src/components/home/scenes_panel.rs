use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    div, prelude::*, px, rgb, Context, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    Render, RenderImage, SharedString, Window,
};

use crate::{
    actions::{ComponentEvent, ContextMenuTarget},
    components::editable_text::CaretField,
    theme::RevoTheme,
};

/// Studio-mode transition duration used when the field is empty or invalid
/// (plan §19.2: default 300 ms).
const STUDIO_DURATION_DEFAULT_MS: u32 = 300;
const STUDIO_DURATION_MIN_MS: u32 = 50;
const STUDIO_DURATION_MAX_MS: u32 = 20_000;
const STUDIO_DURATION_FONT_PX: f32 = 11.5;
const STUDIO_DURATION_PAD_PX: f32 = 6.;

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
    pub thumbnails: HashMap<String, Arc<RenderImage>>,
    /// Round-robin cursor for the thumbnail refresh cycle.
    pub thumbnail_cursor: usize,
    /// Studio-mode transition duration draft, in ms (§19.2).
    pub studio_duration_ms: String,
    /// Caret state for the studio duration field.
    pub studio_duration_field: CaretField,
    /// Focus handle for the studio duration field (created on first focus).
    pub studio_duration_focus: Option<gpui::FocusHandle>,
    /// Whether the studio duration field currently owns keystrokes.
    pub studio_duration_focused: bool,
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

    /// Arms one scene as the studio preview (plan §19.3 "Set preview"); the
    /// program is untouched.
    pub fn set_preview(&mut self, name: &str) -> bool {
        if !self.scenes.iter().any(|scene| scene.name == name) {
            return false;
        }
        self.preview_scene = Some(name.to_owned());
        true
    }

    /// The studio transition duration in ms, clamped to the apply range; the
    /// field default is 300 (plan §19.2).
    pub fn studio_duration_ms(&self) -> u32 {
        self.studio_duration_ms
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .map(|value| value.round().clamp(0.0, i64::MAX as f64) as u32)
            .unwrap_or(STUDIO_DURATION_DEFAULT_MS)
            .clamp(STUDIO_DURATION_MIN_MS, STUDIO_DURATION_MAX_MS)
    }

    /// Applies one keystroke to the studio duration field; returns `true` when
    /// the text changed. The host owns the keystroke-to-field plumbing (focus
    /// and caret display), so this only mirrors the value.
    pub fn apply_studio_duration_keystroke(&mut self, event: &gpui::KeyDownEvent) -> bool {
        let current = self.studio_duration_ms.clone();
        let Some(next) = self.studio_duration_field.keystroke(event, &current) else {
            return false;
        };
        self.studio_duration_ms = next;
        true
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
    pub fn set_thumbnail(&mut self, scene: &str, image: Arc<RenderImage>) {
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

impl ScenesPanel {
    /// Studio-mode transition controls under the scene list (plan §19.2):
    /// Cut, Fade, Transition and an editable duration in ms, centred. Only
    /// shown while a preview scene is armed, because every control takes that
    /// scene to program.
    fn studio_controls(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        focus: &gpui::FocusHandle,
    ) -> Option<gpui::Div> {
        if !self.studio_mode {
            return None;
        }
        let scene = self.preview_scene.clone()?;
        let duration = self.studio_duration_ms();
        let focus = focus.clone();
        let focused = self.studio_duration_focused;
        self.studio_duration_field
            .set_value(self.studio_duration_ms.clone());

        let field = if focused {
            let caret = self
                .studio_duration_field
                .display(
                    window,
                    STUDIO_DURATION_FONT_PX,
                    STUDIO_DURATION_PAD_PX,
                    self.theme.text,
                    self.theme.accent,
                )
                .into_any_element();
            div()
                .id("scenes-studio-duration")
                .relative()
                .w(px(64.))
                .px_2()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(self.theme.accent)
                .bg(self.theme.surface_elevated)
                .text_size(px(STUDIO_DURATION_FONT_PX))
                .child(caret)
                .child(self.studio_duration_field.bounds_probe())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener({
                        let focus = focus.clone();
                        move |this, event: &MouseDownEvent, window, cx| {
                            this.studio_duration_focused = true;
                            let current = this.studio_duration_ms.clone();
                            this.studio_duration_field.place_caret(
                                window,
                                event.position.x,
                                &current,
                                STUDIO_DURATION_FONT_PX,
                                STUDIO_DURATION_PAD_PX,
                            );
                            focus.focus(window);
                            cx.notify();
                        }
                    }),
                )
                .into_any_element()
        } else {
            div()
                .id("scenes-studio-duration")
                .w(px(64.))
                .px_2()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(self.theme.border_strong)
                .bg(self.theme.surface_elevated)
                .text_size(px(STUDIO_DURATION_FONT_PX))
                .text_color(if self.studio_duration_ms.trim().is_empty() {
                    self.theme.text_muted
                } else {
                    self.theme.text
                })
                .child(if self.studio_duration_ms.trim().is_empty() {
                    "300".to_string()
                } else {
                    self.studio_duration_ms.clone()
                })
                .child(self.studio_duration_field.bounds_probe())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener({
                        let focus = focus.clone();
                        move |this, event: &MouseDownEvent, window, cx| {
                            this.studio_duration_focused = true;
                            let current = this.studio_duration_ms.clone();
                            this.studio_duration_field.place_caret(
                                window,
                                event.position.x,
                                &current,
                                STUDIO_DURATION_FONT_PX,
                                STUDIO_DURATION_PAD_PX,
                            );
                            focus.focus(window);
                            cx.notify();
                        }
                    }),
                )
                .into_any_element()
        };

        let button = |this: &mut Self,
                      id: &'static str,
                      label: &'static str,
                      primary: bool,
                      emit: fn(&str, u32) -> ComponentEvent|
         -> gpui::Stateful<gpui::Div> {
            let scene = scene.clone();
            let duration = duration;
            div()
                .id(id)
                .cursor_pointer()
                .px_3()
                .py_1()
                .rounded_md()
                .text_size(px(11.5))
                .bg(if primary {
                    this.theme.accent
                } else {
                    this.theme.surface_active
                })
                .text_color(if primary {
                    this.theme.text_strong
                } else {
                    this.theme.text
                })
                .child(label)
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.emit(emit(&scene, duration));
                }))
        };

        Some(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .items_center()
                .child(
                    div()
                        .text_color(self.theme.text_muted)
                        .text_size(px(10.))
                        .child(SharedString::from(format!("Studio transition → {scene}"))),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .child(button(
                            self,
                            "scenes-studio-cut",
                            "Cut",
                            true,
                            |scene, _| ComponentEvent::StudioCut {
                                scene: scene.to_string(),
                            },
                        ))
                        .child(button(
                            self,
                            "scenes-studio-fade",
                            "Fade",
                            false,
                            |scene, duration| ComponentEvent::StudioFade {
                                scene: scene.to_string(),
                                duration_ms: duration,
                            },
                        ))
                        .child(button(
                            self,
                            "scenes-studio-transition",
                            "Transition",
                            false,
                            |scene, duration| ComponentEvent::StudioTransition {
                                scene: scene.to_string(),
                                duration_ms: duration,
                            },
                        ))
                        .child(field)
                        .child(
                            div()
                                .text_color(self.theme.text_muted)
                                .text_size(px(10.))
                                .child("ms"),
                        ),
                ),
        )
    }
}

impl Render for ScenesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
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
                        // Plan §15.2: state shown as icons — an accent live
                        // dot for the active scene and a closed padlock for
                        // locked ones — instead of the "[active]"/"[locked]"
                        // text suffixes.
                        div().flex_1().min_w_0().child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .child(SharedString::from(scene.name.clone())),
                                )
                                .when(scene.active, |row| {
                                    row.child(crate::components::icon::icon(
                                        self.theme.accent,
                                        "record",
                                        10.,
                                    ))
                                })
                                .when(scene.locked, |row| {
                                    row.child(crate::components::icon::icon(
                                        self.theme.text_muted,
                                        "lock",
                                        11.,
                                    ))
                                }),
                        ),
                    ),
            );
        }
        let duration_focus = self
            .studio_duration_focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .text_color(self.theme.text)
            .track_focus(&duration_focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if !this.studio_duration_focused {
                    return;
                }
                if this.apply_studio_duration_keystroke(event) {
                    cx.notify();
                }
            }))
            .children(self.studio_controls(window, cx, &duration_focus))
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
    /// A 1x1 transparent BGRA thumbnail fixture (the layout gpui's
    /// [`RenderImage`] frames use).
    fn test_thumbnail() -> Arc<RenderImage> {
        Arc::new(RenderImage::new(vec![image::Frame::new(
            image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 0])),
        )]))
    }

    use super::*;

    #[test]
    fn set_preview_arms_known_scenes_only() {
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
            studio_mode: true,
            rename_draft: None,
            thumbnails: Default::default(),
            thumbnail_cursor: 0,
            studio_duration_ms: "300".into(),
            studio_duration_field: CaretField::default(),
            studio_duration_focus: None,
            studio_duration_focused: false,
            theme: RevoTheme::classic(),
        };
        // §19.3 "Set preview" arms without touching the program.
        assert!(panel.set_preview("B"));
        assert_eq!(panel.preview_scene.as_deref(), Some("B"));
        assert!(panel
            .scenes
            .iter()
            .any(|scene| scene.active && scene.name == "A"));
        // Unknown scenes are rejected.
        assert!(!panel.set_preview("missing"));
        assert_eq!(panel.preview_scene.as_deref(), Some("B"));
    }

    #[test]
    fn studio_duration_is_clamped_to_the_apply_range() {
        let mut panel = ScenesPanel {
            scenes: Vec::new(),
            preview_scene: None,
            studio_mode: true,
            rename_draft: None,
            thumbnails: Default::default(),
            thumbnail_cursor: 0,
            studio_duration_ms: String::new(),
            studio_duration_field: CaretField::default(),
            studio_duration_focus: None,
            studio_duration_focused: false,
            theme: RevoTheme::classic(),
        };
        // Empty and unparseable drafts fall back to the 300 ms default.
        assert_eq!(panel.studio_duration_ms(), 300);
        panel.studio_duration_ms = "450".into();
        assert_eq!(panel.studio_duration_ms(), 450);
        // Out-of-range values clamp instead of reaching libobs.
        panel.studio_duration_ms = "10".into();
        assert_eq!(panel.studio_duration_ms(), 50);
        panel.studio_duration_ms = "999999".into();
        assert_eq!(panel.studio_duration_ms(), 20_000);
        panel.studio_duration_ms = "not a number".into();
        assert_eq!(panel.studio_duration_ms(), 300);
    }

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
            studio_duration_ms: "300".into(),
            studio_duration_field: CaretField::default(),
            studio_duration_focus: None,
            studio_duration_focused: false,
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
            studio_duration_ms: "300".into(),
            studio_duration_field: CaretField::default(),
            studio_duration_focus: None,
            studio_duration_focused: false,
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
            studio_duration_ms: "300".into(),
            studio_duration_field: CaretField::default(),
            studio_duration_focus: None,
            studio_duration_focused: false,
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
            studio_duration_ms: "300".into(),
            studio_duration_field: CaretField::default(),
            studio_duration_focus: None,
            studio_duration_focused: false,
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
            studio_duration_ms: "300".into(),
            studio_duration_field: CaretField::default(),
            studio_duration_focus: None,
            studio_duration_focused: false,
            theme: RevoTheme::classic(),
        };
        assert_eq!(panel.next_thumbnail_target().as_deref(), Some("A"));
        assert_eq!(panel.next_thumbnail_target().as_deref(), Some("B"));
        // The cycle wraps.
        assert_eq!(panel.next_thumbnail_target().as_deref(), Some("A"));
        panel.set_thumbnail("A", test_thumbnail());
        assert!(panel.thumbnails.contains_key("A"));
    }
}
