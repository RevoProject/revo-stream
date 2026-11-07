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

/// One context-menu row: a direct action or a submenu group whose children
/// render indented below the group header (Svelte-style `Transform`
/// grouping for the move actions).
#[derive(Clone, Debug, PartialEq)]
pub enum MenuEntry {
    Action(ContextMenuAction, String),
    Submenu {
        label: String,
        children: Vec<(ContextMenuAction, String)>,
    },
}

impl MenuEntry {
    /// Expanded row count: a closed submenu is one row; an open one adds
    /// its children.
    fn rows(&self, submenu_open: bool) -> usize {
        match self {
            Self::Action(..) => 1,
            Self::Submenu { children, .. } => {
                if submenu_open {
                    1 + children.len()
                } else {
                    1
                }
            }
        }
    }
}

/// Menu entries for one target, in display order. Labels reflect the
/// target's current state (lock/visibility), matching the OBS right-click
/// workflow the panels mirror. The move directions live in a `Transform`
/// submenu, and the quick action stays on the panel header (not duplicated
/// here).
pub fn entries(target: &ContextMenuTarget) -> Vec<MenuEntry> {
    match target {
        ContextMenuTarget::Scene { locked, .. } => {
            vec![
                MenuEntry::Action(ContextMenuAction::Select, "Select scene".into()),
                MenuEntry::Action(
                    ContextMenuAction::ToggleLock,
                    if *locked {
                        "Unlock scene"
                    } else {
                        "Lock scene"
                    }
                    .into(),
                ),
                MenuEntry::Submenu {
                    label: "Transform".into(),
                    children: move_children(),
                },
                MenuEntry::Action(ContextMenuAction::Remove, "Remove scene".into()),
            ]
        }
        ContextMenuTarget::Source {
            visible, locked, ..
        } => {
            vec![
                MenuEntry::Action(ContextMenuAction::Edit, "Properties".into()),
                MenuEntry::Action(
                    ContextMenuAction::ToggleVisibility,
                    if *visible { "Hide" } else { "Show" }.into(),
                ),
                MenuEntry::Action(
                    ContextMenuAction::ToggleLock,
                    if *locked { "Unlock" } else { "Lock" }.into(),
                ),
                MenuEntry::Submenu {
                    label: "Transform".into(),
                    children: move_children(),
                },
                MenuEntry::Action(ContextMenuAction::Remove, "Remove source".into()),
            ]
        }
        ContextMenuTarget::Preview => vec![
            MenuEntry::Action(
                ContextMenuAction::ForcePreviewResolution,
                "Force preview resolution".into(),
            ),
            MenuEntry::Action(
                ContextMenuAction::ToggleStudioMode,
                "Toggle studio mode".into(),
            ),
        ],
    }
}

/// The four ordering actions shared by the scene and source menus.
fn move_children() -> Vec<(ContextMenuAction, String)> {
    vec![
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
    ]
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
    /// Whether the `Transform` submenu group is expanded.
    pub submenu_open: bool,
    pub theme: RevoTheme,
}

impl Default for WorkspaceContextMenu {
    fn default() -> Self {
        Self {
            open: false,
            target: None,
            position: (0.0, 0.0),
            selected_index: None,
            submenu_open: false,
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
        self.submenu_open = false;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.target = None;
        self.position = (0.0, 0.0);
        self.selected_index = None;
        self.submenu_open = false;
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
        let effective_rows: usize = rows.iter().map(|entry| entry.rows(self.submenu_open)).sum();
        let (x, y) = clamp_position(
            self.position,
            {
                let size = window.viewport_size();
                (f32::from(size.width), f32::from(size.height))
            },
            effective_rows,
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
        let mut flat_index = 0usize;
        for entry in rows.into_iter() {
            match entry {
                MenuEntry::Action(action, label) => {
                    let index = flat_index;
                    flat_index += 1;
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
                            // Pointer hover moves the highlight, so the
                            // selected row always follows the cursor.
                            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                if *hovered && this.selected_index != Some(index) {
                                    this.selected_index = Some(index);
                                    cx.notify();
                                }
                            }))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(event) = this.choose(action) {
                                    cx.emit(event);
                                }
                                cx.notify();
                            })),
                    );
                }
                MenuEntry::Submenu { label, children } => {
                    let index = flat_index;
                    flat_index += 1;
                    let open = self.submenu_open;
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
                            .flex()
                            .justify_between()
                            .child(label)
                            .child(if open { "▾" } else { "▸" })
                            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                if *hovered && this.selected_index != Some(index) {
                                    this.selected_index = Some(index);
                                    cx.notify();
                                }
                            }))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.submenu_open = !this.submenu_open;
                                cx.notify();
                            })),
                    );
                    if open {
                        for (action, child_label) in children.into_iter() {
                            let child_index = flat_index;
                            flat_index += 1;
                            menu = menu.child(
                                div()
                                    .id(SharedString::from(format!("context-menu-{child_index}")))
                                    .cursor_pointer()
                                    .pl_5()
                                    .pr_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(if self.selected_index == Some(child_index) {
                                        self.theme.surface_selected
                                    } else {
                                        self.theme.surface_elevated
                                    })
                                    .text_color(self.theme.text_secondary)
                                    .text_size(px(11.2))
                                    .truncate()
                                    .child(child_label)
                                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                        if *hovered && this.selected_index != Some(child_index) {
                                            this.selected_index = Some(child_index);
                                            cx.notify();
                                        }
                                    }))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(event) = this.choose(action) {
                                            cx.emit(event);
                                        }
                                        cx.notify();
                                    })),
                            );
                        }
                    }
                }
            }
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
                    .occlude()
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

    /// Flattens entries the way the renderer does for an open submenu.
    fn actions_with_submenu(
        target: &ContextMenuTarget,
        submenu_open: bool,
    ) -> Vec<ContextMenuAction> {
        let mut actions = Vec::new();
        for entry in entries(target) {
            match entry {
                MenuEntry::Action(action, _) => actions.push(action),
                MenuEntry::Submenu { children, .. } => {
                    if submenu_open {
                        actions.extend(children.into_iter().map(|(action, _)| action));
                    }
                }
            }
        }
        actions
    }

    #[test]
    fn scene_entries_follow_the_obs_right_click_workflow() {
        let open = actions_with_submenu(&scene_target(false), true);
        assert_eq!(
            open,
            vec![
                ContextMenuAction::Select,
                ContextMenuAction::ToggleLock,
                ContextMenuAction::Move(MoveDirection::Up),
                ContextMenuAction::Move(MoveDirection::Down),
                ContextMenuAction::Move(MoveDirection::Top),
                ContextMenuAction::Move(MoveDirection::Bottom),
                ContextMenuAction::Remove,
            ]
        );
    }

    #[test]
    fn source_entries_hide_quick_action_and_group_moves() {
        let open = actions_with_submenu(&source_target(true, false), true);
        assert!(!open.contains(&ContextMenuAction::QuickAction));
        assert_eq!(open.first(), Some(&ContextMenuAction::Edit));
        assert_eq!(open[1], ContextMenuAction::ToggleVisibility);
        assert_eq!(open[2], ContextMenuAction::ToggleLock);
        assert_eq!(open[3], ContextMenuAction::Move(MoveDirection::Up));
        assert_eq!(open.last(), Some(&ContextMenuAction::Remove));
        // Closed submenu keeps only the direct actions.
        assert_eq!(
            actions_with_submenu(&source_target(true, false), false).len(),
            4
        );
    }

    #[test]
    fn submenu_row_count_tracks_the_expanded_group() {
        let rows: Vec<usize> = entries(&source_target(true, false))
            .iter()
            .map(|entry| entry.rows(true))
            .collect();
        assert_eq!(rows, vec![1, 1, 1, 5, 1]);
    }

    #[test]
    fn preview_entries_cover_resolution_and_studio_mode() {
        let actions = actions_with_submenu(&ContextMenuTarget::Preview, false);
        assert_eq!(
            actions,
            vec![
                ContextMenuAction::ForcePreviewResolution,
                ContextMenuAction::ToggleStudioMode,
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
