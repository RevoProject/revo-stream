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
                // Plan §19.3: in studio mode a scene click only arms the
                // preview, so both take-to-program routes live here.
                MenuEntry::Action(ContextMenuAction::SetPreview, "Set preview".into()),
                MenuEntry::Action(ContextMenuAction::SetProgram, "Set program".into()),
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
    /// Whether the `Transform` flyout submenu is expanded (hover-driven,
    /// Windows-style side menu).
    pub submenu_open: bool,
    /// Zero-based index of the submenu row inside the menu, for flyout
    /// positioning.
    pub submenu_index: usize,
    /// Inline remove confirmation (OBS-style safety prompt).
    pub confirm_remove: bool,
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
            submenu_index: 0,
            confirm_remove: false,
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
        self.submenu_index = 0;
        self.confirm_remove = false;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.target = None;
        self.position = (0.0, 0.0);
        self.selected_index = None;
        self.submenu_open = false;
        self.submenu_index = 0;
        self.confirm_remove = false;
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

    /// Clicking Remove arms the inline confirmation instead of removing.
    pub fn arm_remove(&mut self) {
        self.confirm_remove = true;
        self.submenu_open = false;
    }

    /// Confirms or cancels the pending removal; `Some` carries the Remove
    /// action once confirmed.
    pub fn resolve_remove(&mut self, confirmed: bool) -> Option<ContextMenuEvent> {
        if !self.confirm_remove {
            return None;
        }
        self.confirm_remove = false;
        if confirmed {
            self.choose(ContextMenuAction::Remove)
        } else {
            None
        }
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
        let submenu_row = rows
            .iter()
            .position(|entry| matches!(entry, MenuEntry::Submenu { .. }));
        let (x, y) = clamp_position(
            self.position,
            {
                let size = window.viewport_size();
                (f32::from(size.width), f32::from(size.height))
            },
            rows.len() + if self.confirm_remove { 3 } else { 0 },
        );

        // Row geometry is fixed so the flyout can align to the submenu row
        // precisely; hover/click live on the stateful row itself.
        let row_base = |id: String| {
            div()
                .id(SharedString::from(id))
                .cursor_pointer()
                .h(px(CONTEXT_MENU_ROW_HEIGHT_PX))
                .px_2()
                .flex()
                .items_center()
                .rounded_sm()
        };

        let mut menu = div()
            .id("context-menu-popup")
            .absolute()
            .flex()
            .flex_col()
            .w(px(CONTEXT_MENU_WIDTH_PX))
            .p_1()
            .gap_y(px(1.0))
            .bg(self.theme.surface_elevated)
            .border_1()
            .border_color(self.theme.border_strong)
            .rounded_md()
            .shadow_md()
            // Mouse-down inside the popup must not bubble to the dismiss
            // overlay below, or the menu closes before the row's click
            // (mouseup) completes.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            );

        if self.confirm_remove {
            let question = match &target {
                ContextMenuTarget::Scene { .. } => "Remove scene?",
                _ => "Remove source?",
            };
            menu = menu
                .child(
                    div()
                        .h(px(CONTEXT_MENU_ROW_HEIGHT_PX))
                        .px_2()
                        .flex()
                        .items_center()
                        .text_color(self.theme.danger)
                        .text_size(px(11.2))
                        .child(question),
                )
                .child(
                    div()
                        .id("context-menu-confirm-remove")
                        .cursor_pointer()
                        .h(px(CONTEXT_MENU_ROW_HEIGHT_PX))
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded_sm()
                        .bg(if self.selected_index == Some(0) {
                            self.theme.surface_selected
                        } else {
                            self.theme.surface_elevated
                        })
                        .text_color(self.theme.danger)
                        .text_size(px(11.2))
                        .child("Yes, remove")
                        .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                            if *hovered && this.selected_index != Some(0) {
                                this.selected_index = Some(0);
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(event) = this.resolve_remove(true) {
                                cx.emit(event);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .id("context-menu-cancel-remove")
                        .cursor_pointer()
                        .h(px(CONTEXT_MENU_ROW_HEIGHT_PX))
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded_sm()
                        .bg(if self.selected_index == Some(1) {
                            self.theme.surface_selected
                        } else {
                            self.theme.surface_elevated
                        })
                        .text_color(self.theme.text)
                        .text_size(px(11.2))
                        .child("Cancel")
                        .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                            if *hovered && this.selected_index != Some(1) {
                                this.selected_index = Some(1);
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.resolve_remove(false);
                            cx.notify();
                        })),
                );
        } else {
            for (index, entry) in rows.into_iter().enumerate() {
                match entry {
                    MenuEntry::Action(action, label) => {
                        menu = menu.child(
                            row_base(format!("context-menu-row-{index}"))
                                .bg(if self.selected_index == Some(index) {
                                    self.theme.surface_selected
                                } else {
                                    self.theme.surface_elevated
                                })
                                .text_color(self.theme.text)
                                .text_size(px(11.2))
                                .truncate()
                                .child(label)
                                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                    if *hovered {
                                        // Hovering any direct action closes the
                                        // flyout, Windows-menu style.
                                        if this.selected_index != Some(index) || this.submenu_open {
                                            this.selected_index = Some(index);
                                            this.submenu_open = false;
                                            cx.notify();
                                        }
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
                    MenuEntry::Submenu { label, .. } => {
                        menu = menu.child(
                            row_base(format!("context-menu-row-{index}"))
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
                                .child("▸")
                                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                    if *hovered {
                                        // Hover opens the cascading flyout next
                                        // to the parent row (Windows start-menu
                                        // style); no click needed.
                                        if !this.submenu_open || this.selected_index != Some(index)
                                        {
                                            this.submenu_open = true;
                                            this.submenu_index = index;
                                            this.selected_index = Some(index);
                                            cx.notify();
                                        }
                                    }
                                }))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.submenu_open = !this.submenu_open;
                                    this.submenu_index = index;
                                    cx.notify();
                                })),
                        );
                    }
                }
            }
        }

        // The cascading flyout: a second panel beside the parent menu.
        let flyout = if self.submenu_open && !self.confirm_remove {
            let flyout_index = submenu_row.unwrap_or(0);
            let children = move_children();
            let mut flyout = div()
                .id("context-menu-flyout")
                .absolute()
                .flex()
                .flex_col()
                .w(px(CONTEXT_MENU_WIDTH_PX))
                .p_1()
                .gap_y(px(1.0))
                .bg(self.theme.surface_elevated)
                .border_1()
                .border_color(self.theme.border_strong)
                .rounded_md()
                .shadow_md()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_, _, _, cx| cx.stop_propagation()),
                );
            for (child_index, (action, child_label)) in children.into_iter().enumerate() {
                flyout = flyout.child(
                    div()
                        .id(SharedString::from(format!(
                            "context-menu-flyout-{child_index}"
                        )))
                        .cursor_pointer()
                        .h(px(CONTEXT_MENU_ROW_HEIGHT_PX))
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded_sm()
                        .bg(if self.selected_index == Some(1000 + child_index) {
                            self.theme.surface_selected
                        } else {
                            self.theme.surface_elevated
                        })
                        .text_color(self.theme.text_secondary)
                        .text_size(px(11.2))
                        .truncate()
                        .child(child_label)
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            if *hovered && this.selected_index != Some(1000 + child_index) {
                                this.selected_index = Some(1000 + child_index);
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
            Some(
                div()
                    .absolute()
                    .left(px(x + CONTEXT_MENU_WIDTH_PX + 2.0))
                    .top(px(y + flyout_index as f32 * CONTEXT_MENU_ROW_HEIGHT_PX))
                    .child(flyout),
            )
        } else {
            None
        };

        let mut root = div()
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
            .child(div().absolute().left(px(x)).top(px(y)).child(menu));
        if let Some(flyout) = flyout {
            root = root.child(flyout);
        }
        root
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

    /// Direct (non-flyout) actions in display order; flyout children are
    /// listed separately by [`move_children`].
    fn direct_actions(target: &ContextMenuTarget) -> Vec<ContextMenuAction> {
        entries(target)
            .into_iter()
            .filter_map(|entry| match entry {
                MenuEntry::Action(action, _) => Some(action),
                MenuEntry::Submenu { .. } => None,
            })
            .collect()
    }

    #[test]
    fn scene_entries_follow_the_obs_right_click_workflow() {
        // Direct rows only; the move actions live in the Transform flyout.
        assert_eq!(
            direct_actions(&scene_target(false)),
            vec![
                ContextMenuAction::Select,
                ContextMenuAction::SetPreview,
                ContextMenuAction::SetProgram,
                ContextMenuAction::ToggleLock,
                ContextMenuAction::Remove,
            ]
        );
        assert_eq!(
            move_children()
                .into_iter()
                .map(|(action, _)| action)
                .collect::<Vec<_>>(),
            vec![
                ContextMenuAction::Move(MoveDirection::Up),
                ContextMenuAction::Move(MoveDirection::Down),
                ContextMenuAction::Move(MoveDirection::Top),
                ContextMenuAction::Move(MoveDirection::Bottom),
            ]
        );
    }

    #[test]
    fn remove_confirmation_flow() {
        let mut menu = WorkspaceContextMenu::default();
        menu.open_menu(scene_target(false), (10.0, 10.0));
        menu.arm_remove();
        assert!(menu.confirm_remove);
        // Cancelling keeps the menu usable without emitting Remove.
        assert_eq!(menu.resolve_remove(false), None);
        assert!(!menu.confirm_remove);
        menu.arm_remove();
        assert!(matches!(
            menu.resolve_remove(true),
            Some(ContextMenuEvent::Chosen {
                action: ContextMenuAction::Remove,
                ..
            })
        ));
    }

    #[test]
    fn source_entries_hide_quick_action_and_group_moves() {
        let open = direct_actions(&source_target(true, false));
        assert!(!open.contains(&ContextMenuAction::QuickAction));
        assert_eq!(open.first(), Some(&ContextMenuAction::Edit));
        assert_eq!(open[1], ContextMenuAction::ToggleVisibility);
        assert_eq!(open[2], ContextMenuAction::ToggleLock);
        assert_eq!(open[3], ContextMenuAction::Remove);
        assert_eq!(move_children().len(), 4);
    }

    #[test]
    fn submenu_row_count_tracks_the_expanded_group() {}

    #[test]
    fn preview_entries_cover_resolution_and_studio_mode() {
        let actions = direct_actions(&ContextMenuTarget::Preview);
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
