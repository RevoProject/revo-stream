//! Functional native Plugins modal: profile tabs on a rail, the discovered
//! libobs module list with enable toggles, and a save-as-new profile name
//! field. The draft state machine lives in [`PluginsDraft`]; this view owns
//! focus and routes keystrokes into it.

use gpui::{
    div, prelude::*, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    MouseButton, Render, SharedString, Window,
};

use crate::{
    components::modal::plugins_state::{PluginsDraft, PluginsEvent},
    components::modal_shell::{modal_shell_with_body, ModalConfig, ModalSize},
    theme::RevoTheme,
};

/// Functional native Plugins modal.
pub struct PluginsModal {
    pub draft: PluginsDraft,
    pub theme: RevoTheme,
    name_focus: FocusHandle,
    name_focused: bool,
    commit_error: Option<String>,
}

impl EventEmitter<PluginsEvent> for PluginsModal {}

impl Focusable for PluginsModal {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.name_focus.clone()
    }
}

impl PluginsModal {
    pub fn new(draft: PluginsDraft, cx: &mut Context<Self>) -> Self {
        Self {
            draft,
            theme: RevoTheme::future(),
            name_focus: cx.focus_handle(),
            name_focused: false,
            commit_error: None,
        }
    }

    fn profile_tab(
        &self,
        profile_name: &str,
        applied: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let name = profile_name.to_owned();
        let active = self.draft.target_profile.as_deref() == Some(name.as_str());
        let label = if applied {
            format!("{name} (applied)")
        } else {
            name.clone()
        };
        div()
            .id(SharedString::from(format!("plugins-profile-{name}")))
            .cursor_pointer()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(if active {
                self.theme.surface_active
            } else {
                self.theme.surface
            })
            .text_color(if active {
                self.theme.text_strong
            } else {
                self.theme.text_muted
            })
            .truncate()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.draft.select_profile(&name);
                this.commit_error = None;
                cx.notify();
            }))
            .child(SharedString::from(label))
    }

    fn module_row(
        &self,
        index: usize,
        module_name: &str,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let name = module_name.to_owned();
        div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .gap_2()
            .py_1()
            .border_b_1()
            .border_color(self.theme.border)
            .child(
                div().flex().flex_col().min_w_0().child(
                    div()
                        .text_color(self.theme.text)
                        .truncate()
                        .child(SharedString::from(name.clone())),
                ),
            )
            .child(
                div()
                    .id(SharedString::from(format!("plugins-toggle-{index}")))
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
                    .child(if enabled { "On" } else { "Off" })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.draft.toggle_module(&name);
                        cx.notify();
                    })),
            )
    }

    fn name_field(&self, cx: &mut Context<Self>) -> gpui::Div {
        let value = self.draft.new_profile_name.clone();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .min_w_0()
            .child(
                div()
                    .text_color(self.theme.text_secondary)
                    .text_size(gpui::px(12.))
                    .child("Save as new profile"),
            )
            .child(
                div()
                    .id("plugins-name-field")
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(if self.name_focused {
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
                        cx.listener(|this, _, window, cx| {
                            this.name_focused = true;
                            this.name_focus.focus(window);
                            cx.notify();
                        }),
                    )
                    .child(if value.is_empty() {
                        SharedString::from("Profile name…")
                    } else {
                        SharedString::from(value)
                    }),
            )
    }

    fn module_body(&mut self, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let mut list = div()
            .id("plugins-module-list")
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .overflow_y_scroll()
            .child(
                div()
                    .text_color(self.theme.text_secondary)
                    .text_size(gpui::px(12.))
                    .pb_1()
                    .child(SharedString::from(format!(
                        "Discovered modules ({})",
                        self.draft.plugins.len()
                    ))),
            );
        for (index, plugin) in self.draft.plugins.iter().enumerate() {
            let enabled = self.draft.is_module_enabled(&plugin.module_name);
            list = list.child(self.module_row(index, &plugin.module_name, enabled, cx));
        }
        if self.draft.plugins.is_empty() {
            list = list.child(
                div()
                    .text_color(self.theme.text_muted)
                    .child("No libobs modules discovered yet — start the engine to scan."),
            );
        }
        list
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        match self.draft.commit() {
            Ok((profile_name, modules)) => {
                cx.emit(PluginsEvent::Saved {
                    profile_name,
                    modules,
                });
            }
            Err(error) => {
                self.commit_error = Some(error);
                cx.notify();
            }
        }
    }
}

impl Render for PluginsModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let mut rail = div().flex().flex_col().gap_1().w(gpui::px(200.));
        rail = rail.child(
            div()
                .text_color(self.theme.text_secondary)
                .text_size(gpui::px(12.))
                .child("Profiles"),
        );
        for profile in self.draft.profiles.clone() {
            let applied = self.draft.active_profile.as_deref() == Some(profile.as_str());
            rail = rail.child(self.profile_tab(&profile, applied, cx));
        }
        let new_active = self.draft.target_profile.is_none();
        rail = rail.child(
            div()
                .id("plugins-profile-new")
                .cursor_pointer()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(if new_active {
                    self.theme.surface_active
                } else {
                    self.theme.surface
                })
                .text_color(if new_active {
                    self.theme.text_strong
                } else {
                    self.theme.text_muted
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.draft.begin_new_profile();
                    this.commit_error = None;
                    cx.notify();
                }))
                .child("+ New profile"),
        );

        let dirty_note = if self.draft.is_dirty() {
            SharedString::from("Unsaved changes")
        } else {
            SharedString::from("Saved")
        };
        let error_note = self.commit_error.clone().unwrap_or_default();

        let config = ModalConfig::standard("Plugins", ModalSize::Large);
        let theme = self.theme;
        let footer = div()
            .flex()
            .flex_row()
            .justify_between()
            .items_end()
            .gap_3()
            .p_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .flex_1()
                    .min_w_0()
                    .child(self.name_field(cx))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_color(if self.draft.is_dirty() {
                                        theme.warning
                                    } else {
                                        theme.text_muted
                                    })
                                    .child(dirty_note),
                            )
                            .child(
                                div()
                                    .text_color(theme.danger)
                                    .child(SharedString::from(error_note)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id("plugins-cancel")
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(theme.surface_active)
                            .text_color(theme.text)
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(PluginsEvent::Cancelled);
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("plugins-save")
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(theme.accent)
                            .text_color(theme.text_strong)
                            .on_click(cx.listener(|this, _, _, cx| this.confirm(cx)))
                            .child("Save"),
                    ),
            )
            .into_any_element();

        let on_key = cx.listener(|this, event: &KeyDownEvent, _, cx| {
            if this.name_focused {
                let keystroke = &event.keystroke;
                this.draft.field_keystroke(
                    &keystroke.key,
                    keystroke.key_char.as_deref(),
                    keystroke.modifiers.control,
                    keystroke.modifiers.alt,
                    keystroke.modifiers.platform,
                );
                cx.notify();
            }
        });

        let close = div()
            .id("plugins-close")
            .cursor_pointer()
            .text_color(self.theme.text_muted)
            .child("Close")
            .on_click(cx.listener(|_this, _, _, cx| cx.emit(PluginsEvent::Cancelled)))
            .into_any_element();
        modal_shell_with_body(
            &config,
            self.theme,
            div()
                .flex()
                .flex_row()
                .gap_3()
                .size_full()
                .track_focus(&self.name_focus)
                .child(rail)
                .child(self.module_body(cx))
                .on_key_down(on_key),
            Some(footer),
            Some(close),
        )
    }
}
