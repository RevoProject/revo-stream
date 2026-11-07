//! Shared right-click context menu for the native workspace.
//!
//! One overlay entity hosted by the Workspace: panel rows and the preview
//! surface request it through `ComponentEvent::OpenContextMenu`, and choices
//! come back as typed [`ContextMenuEvent`]s that route into the existing
//! component-event workflows (and, through them, into libobs-backed backend
//! commands).

use gpui::{div, prelude::*, px, Context, IntoElement, MouseButton, Render, SharedString, Window};

use revo_core::service::MoveDirection;

use crate::{
    actions::{ContextMenuAction, ContextMenuTarget},
    theme::RevoTheme,
};

pub const CONTEXT_MENU_WIDTH_PX: f32 = 180.0;
pub const CONTEXT_MENU_ROW_HEIGHT_PX: f32 = 26.0;
pub const CONTEXT_MENU_PADDING_PX: f32 = 8.0;
pub const CONTEXT_MENU_MARGIN_PX: f32 = 8.0;

#[derive(Clone, Debug, PartialEq)]
pub enum ContextMenuEvent {
    Chosen {
        target: ContextMenuTarget,
        action: ContextMenuAction,
    },
    Dismissed,
}

/// Menu entries for one target, in display order. Labels reflect the
/// target's current state (lock/visibility), matching the OBS right-click
/// workflow the panels mirror.
pub fn entries(target: &ContextMenuTarget) -> Vec<(ContextMenuAction, String)> {
    match target {
        ContextMenuTarget::Scene { locked, .. } => {
            vec![
                (ContextMenuAction::Select, "Select scene".into()),
                (
                    ContextMenuAction::ToggleLock,
                    if *locked {
                        "Unlock scene"
                    } else {
                        "Lock scene"
                    }
                    .into(),
                ),
                (ContextMenuAction::Move(MoveDirection::Up), "Move up".into()),
                (
                    ContextMenuAction::Move(MoveDirection::Down),
                    "Move down".into(),
                ),
                (ContextMenuAction::Remove, "Remove scene".into()),
            ]
        }
        ContextMenuTarget::Source {
            visible, locked, ..
        } => {
            vec![
                (ContextMenuAction::Edit, "Properties".into()),
                (ContextMenuAction::QuickAction, "Quick action".into()),
                (
                    ContextMenuAction::ToggleVisibility,
                    if *visible { "Hide" } else { "Show" }.into(),
                ),
                (
                    ContextMenuAction::ToggleLock,
                    if *locked { "Unlock" } else { "Lock" }.into(),
                ),
                (ContextMenuAction::Move(MoveDirection::Up), "Move up".into()),
                (
                    ContextMenuAction::Move(MoveDirection::Down),
                    "Move down".into(),
                ),
                (
                    ContextMenuAction::Move(MoveDirection::Top),
                    "Move to top".into(),
                ),
                (
                    ContextMenuAction::Move(MoveDirection::Bottom),
                    "Move to bottom".into(),
                ),
                (ContextMenuAction::Remove, "Remove source".into()),
            ]
        }
        ContextMenuTarget::Preview => vec![
            (
                ContextMenuAction::ForcePreviewResolution,
                "Force preview resolution".into(),
            ),
            (
                ContextMenuAction::ToggleStudioMode,
                "Toggle studio mode".into(),
            ),
        ],
    }
}

/// Keeps the menu inside the window: shifts left/up when the cursor sits too
/// close to the right/bottom edge and never enters the margin band.
pub fn clamp_position(
    position: (f32, f32),
    viewport: (f32, f32),
    entry_count: usize,
) -> (f32, f32) {
    let height = entry_count as f32 * CONTEXT_MENU_ROW_HEIGHT_PX + CONTEXT_MENU_PADDING_PX * 2.0;
    let max_x = (viewport.0 - CONTEXT_MENU_WIDTH_PX - CONTEXT_MENU_MARGIN_PX).max(0.0);
    let max_y = (viewport.1 - height - CONTEXT_MENU_MARGIN_PX).max(0.0);
    (
        position.0.clamp(CONTEXT_MENU_MARGIN_PX, max_x),
        position.1.clamp(CONTEXT_MENU_MARGIN_PX, max_y),
    )
}

pub struct WorkspaceContextMenu {
    pub open: bool,
    pub target: Option<ContextMenuTarget>,
    pub position: (f32, f32),
    pub selected_index: Option<usize>,
    pub theme: RevoTheme,
}

impl Default for WorkspaceContextMenu {
    fn default() -> Self {
        Self {
            open: false,
            target: None,
            position: (0.0, 0.0),
            selected_index: None,
            theme: RevoTheme::future(),
        }
    }
}

impl gpui::EventEmitter<ContextMenuEvent> for WorkspaceContextMenu {}

impl WorkspaceContextMenu {
    pub fn open_menu(&mut self, target: ContextMenuTarget, position: (f32, f32)) {
        self.open = true;
        self.target = Some(target);
        self.position = position;
        self.selected_index = Some(0);
    }

    pub fn close(&mut self) {
        self.open = false;
        self.target = None;
        self.position = (0.0, 0.0);
        self.selected_index = None;
    }

    /// Applies one choice, closing the menu; `None` while closed or when the
    /// target was lost.
    pub fn choose(&mut self, action: ContextMenuAction) -> Option<ContextMenuEvent> {
        if !self.open {
            return None;
        }
        let target = self.target.clone()?;
        self.close();
        Some(ContextMenuEvent::Chosen { target, action })
    }

    /// Click-outside/Escape dismissal; `None` while already closed so
    /// repeated clicks do not spam dismissals.
    pub fn dismiss(&mut self) -> Option<ContextMenuEvent> {
        if !self.open {
            return None;
        }
        self.close();
        Some(ContextMenuEvent::Dismissed)
    }
}

impl Render for WorkspaceContextMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let Some(target) = self.target.clone() else {
            return div();
        };
        if !self.open {
            return div();
        }
        let rows = entries(&target);
        let (x, y) = clamp_position(
            self.position,
            {
                let size = window.viewport_size();
                (f32::from(size.width), f32::from(size.height))
            },
            rows.len(),
        );
        let mut menu = div()
            .absolute()
            .flex()
            .flex_col()
            .w(px(CONTEXT_MENU_WIDTH_PX))
            .p_1()
            .gap_px()
            .bg(self.theme.surface_elevated)
            .border_1()
            .border_color(self.theme.border_strong)
            .rounded_md()
            .shadow_md();
        for (index, (action, label)) in rows.into_iter().enumerate() {
            menu = menu.child(
                div()
                    .id(SharedString::from(format!("context-menu-{index}")))
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(if self.selected_index == Some(index) {
                        self.theme.surface_selected
                    } else {
                        self.theme.surface_elevated
                    })
                    .text_color(self.theme.text)
                    .text_size(px(11.2))
                    .truncate()
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(event) = this.choose(action) {
                            cx.emit(event);
                        }
                        cx.notify();
                    })),
            );
        }
        div()
            .absolute()
            .size_full()
            .child(
                div()
                    .id("workspace-context-overlay")
                    .absolute()
                    .size_full()
                    .top_0()
                    .left_0()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if let Some(event) = this.dismiss() {
                                cx.emit(event);
                            }
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, _, cx| {
                            if let Some(event) = this.dismiss() {
                                cx.emit(event);
                            }
                            cx.notify();
                        }),
                    ),
            )
            .child(div().absolute().left(px(x)).top(px(y)).child(menu))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene_target(locked: bool) -> ContextMenuTarget {
        ContextMenuTarget::Scene {
            name: "Main Camera".into(),
            locked,
        }
    }

    fn source_target(visible: bool, locked: bool) -> ContextMenuTarget {
        ContextMenuTarget::Source {
            id: "camera".into(),
            name: "Camera".into(),
            visible,
            locked,
        }
    }

    #[test]
    fn scene_entries_follow_the_obs_right_click_workflow() {
        let open = entries(&scene_target(false));
        assert_eq!(
            open.iter().map(|(action, _)| *action).collect::<Vec<_>>(),
            vec![
                ContextMenuAction::Select,
                ContextMenuAction::ToggleLock,
                ContextMenuAction::Move(MoveDirection::Up),
                ContextMenuAction::Move(MoveDirection::Down),
                ContextMenuAction::Remove,
            ]
        );
        assert_eq!(open[1].1, "Lock scene");
        assert_eq!(entries(&scene_target(true))[1].1, "Unlock scene");
    }

    #[test]
    fn source_entries_reflect_visibility_and_lock_state() {
        let open = entries(&source_target(true, false));
        assert_eq!(
            open.iter()
                .position(|(action, _)| *action == ContextMenuAction::Remove),
            Some(8)
        );
        assert_eq!(open[2].1, "Hide");
        assert_eq!(open[3].1, "Lock");
        assert_eq!(entries(&source_target(false, true))[2].1, "Show");
        assert_eq!(entries(&source_target(false, true))[3].1, "Unlock");
    }

    #[test]
    fn preview_entries_cover_resolution_and_studio_mode() {
        let labels: Vec<String> = entries(&ContextMenuTarget::Preview)
            .into_iter()
            .map(|(_, label)| label)
            .collect();
        assert_eq!(
            labels,
            vec![
                "Force preview resolution".to_string(),
                "Toggle studio mode".to_string(),
            ]
        );
    }

    #[test]
    fn clamp_position_keeps_the_menu_inside_the_window() {
        assert_eq!(clamp_position((2.0, 2.0), (1280.0, 720.0), 5), (8.0, 8.0));
        assert_eq!(
            clamp_position((10.0, 10.0), (1280.0, 720.0), 5),
            (10.0, 10.0)
        );
        assert_eq!(
            clamp_position((1200.0, 700.0), (1280.0, 720.0), 5),
            (
                1280.0 - CONTEXT_MENU_WIDTH_PX - CONTEXT_MENU_MARGIN_PX,
                720.0
                    - (5.0 * CONTEXT_MENU_ROW_HEIGHT_PX + CONTEXT_MENU_PADDING_PX * 2.0)
                    - CONTEXT_MENU_MARGIN_PX
            )
        );
    }

    #[test]
    fn choose_and_dismiss_close_and_emit_once() {
        let mut menu = WorkspaceContextMenu::default();
        assert_eq!(menu.choose(ContextMenuAction::Select), None);
        assert_eq!(menu.dismiss(), None);

        menu.open_menu(scene_target(false), (40.0, 60.0));
        assert!(menu.open);
        assert_eq!(menu.selected_index, Some(0));

        let chosen = menu.choose(ContextMenuAction::Remove).unwrap();
        match chosen {
            ContextMenuEvent::Chosen { target, action } => {
                assert_eq!(target, scene_target(false));
                assert_eq!(action, ContextMenuAction::Remove);
            }
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(!menu.open);
        assert_eq!(menu.target, None);

        menu.open_menu(ContextMenuTarget::Preview, (10.0, 10.0));
        assert_eq!(menu.dismiss(), Some(ContextMenuEvent::Dismissed));
        assert_eq!(menu.dismiss(), None);
    }
}
