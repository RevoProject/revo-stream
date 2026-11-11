use gpui::{div, prelude::*, px, svg, Context, Div, IntoElement, Render, SharedString, Window};

use crate::{
    actions::ComponentEvent,
    components::{icon::icon, panel},
    theme::RevoTheme,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeaderBarState {
    pub demo_mode: bool,
    pub recording: bool,
    pub streaming: bool,
    pub busy: bool,
    pub studio_mode: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub version: String,
}

pub struct HeaderBar {
    pub state: HeaderBarState,
    pub theme: RevoTheme,
}

impl gpui::EventEmitter<ComponentEvent> for HeaderBar {}

/// Icon-only utility button: no frame at rest, text hidden, subtle hover.
/// The caller attaches the click listener (entity-scoped `cx.listener`).
fn ghost_button(
    theme: RevoTheme,
    icon_name: &'static str,
    enabled: bool,
    id: &'static str,
) -> gpui::Stateful<gpui::Div> {
    let color = if enabled {
        theme.text_muted
    } else {
        theme.text_disabled
    };
    div()
        .id(SharedString::from(id))
        .when(enabled, |button| button.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .px_2()
        .py_2()
        .rounded_md()
        .text_color(color)
        .hover(|style| style.bg(theme.surface_active))
        .child(icon(color, icon_name, 15.))
}

fn button(
    theme: RevoTheme,
    label: &'static str,
    enabled: bool,
    icon_name: Option<&'static str>,
) -> Div {
    let color = if enabled {
        theme.text
    } else {
        theme.text_disabled
    };
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1p5()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(if enabled {
            theme.surface_active
        } else {
            theme.surface_elevated
        })
        .text_color(color)
        .when(enabled, |button| button.cursor_pointer());
    if let Some(icon_name) = icon_name {
        row = row.child(icon(color, icon_name, 13.));
    }
    row.child(label)
}

impl Render for HeaderBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let busy = self.state.busy;
        let can_undo = self.state.can_undo && !busy;
        let can_redo = self.state.can_redo && !busy;
        let recording = self.state.recording;
        let streaming = self.state.streaming;
        let undo = button(self.theme, "Undo", can_undo, Some("undo"))
            .id("header-undo")
            .when(can_undo, |button| {
                button.on_click(cx.listener(|_, _, _, cx| cx.emit(ComponentEvent::Undo)))
            });
        let redo = button(self.theme, "Redo", can_redo, Some("redo"))
            .id("header-redo")
            .when(can_redo, |button| {
                button.on_click(cx.listener(|_, _, _, cx| cx.emit(ComponentEvent::Redo)))
            });
        let history = ghost_button(self.theme, "history", true, "header-history")
            .on_click(cx.listener(|_, _, _, cx| cx.emit(ComponentEvent::OpenHistory)));
        let info = ghost_button(self.theme, "info", true, "header-info")
            .on_click(cx.listener(|_, _, _, cx| cx.emit(ComponentEvent::OpenDebugConsoleWindow)));
        let settings = ghost_button(self.theme, "settings", true, "header-settings")
            .on_click(cx.listener(|_, _, _, cx| cx.emit(ComponentEvent::OpenSettings)));
        // The resolution button opens the floating preview window (the
        // resolution label describes what the window shows).
        let preview = button(self.theme, "1920x1080 Preview", !busy, Some("monitor"))
            .id("header-open-preview")
            .when(!busy, |button| {
                button.on_click(cx.listener(|_, _, _, cx| {
                    cx.emit(ComponentEvent::OpenPreviewInWindow { target_id: None })
                }))
            });
        let studio = button(self.theme, "Studio", !busy, Some("layers"))
            .id("header-studio-toggle")
            // Studio mode active: darker blue so the toggled state reads
            // clearly instead of blending with the default button.
            .bg(if busy {
                self.theme.surface_elevated
            } else if self.state.studio_mode {
                self.theme.accent_pressed
            } else {
                self.theme.surface_active
            })
            .when(!busy, |button| {
                button.on_click(cx.listener(|this, _, _, cx| {
                    this.state.studio_mode = !this.state.studio_mode;
                    cx.emit(ComponentEvent::SetStudioMode(this.state.studio_mode));
                    cx.notify();
                }))
            });
        let record = button(
            self.theme,
            if recording {
                "Stop recording"
            } else {
                "Record"
            },
            !busy,
            Some("record"),
        )
        .id("header-record-toggle")
        .bg(if busy {
            self.theme.surface_elevated
        } else if recording {
            self.theme.danger
        } else {
            self.theme.surface_active
        })
        .when(!busy, |button| {
            button.on_click(cx.listener(move |_, _, _, cx| {
                cx.emit(if recording {
                    ComponentEvent::StopRecording
                } else {
                    ComponentEvent::StartRecording
                });
            }))
        });
        let stream = button(
            self.theme,
            if streaming {
                "Stop streaming"
            } else {
                "Go live"
            },
            !busy,
            Some("video"),
        )
        .id("header-stream-toggle")
        .bg(if busy {
            self.theme.surface_elevated
        } else if streaming {
            // While live the button drops to a calmer, darker green instead
            // of the bright success accent.
            self.theme.success_active
        } else {
            self.theme.accent
        })
        .when(!busy, |button| {
            button.on_click(cx.listener(move |_, _, _, cx| {
                cx.emit(if streaming {
                    ComponentEvent::StopStreaming
                } else {
                    ComponentEvent::StartStreaming
                });
            }))
        });

        let mut actions = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .child(preview)
            .child(studio)
            .child(settings);
        actions = actions.child(record).child(stream);

        panel(self.theme)
            .flex_row()
            .justify_between()
            .items_center()
            .bg(self.theme.header)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        // Plan §24: the brand (mark + wordmark) is clickable
                        // and asks before opening the official website.
                        div()
                            .id("header-brand")
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(ComponentEvent::OpenOfficialWebsite);
                            }))
                            .child(
                                // Brand mark (same asset as the boot splash),
                                // tinted like the Svelte header brand.
                                svg()
                                    .path("brand/mark.svg")
                                    .w(px(20.))
                                    .h(px(20.))
                                    .flex_none()
                                    .text_color(self.theme.accent),
                            )
                            .child(div().text_color(self.theme.text_strong).child("RevoStream")),
                    )
                    .child(self.state.version.clone())
                    .child(undo)
                    .child(redo)
                    .child(history)
                    .child(info),
            )
            .child(actions)
    }
}
