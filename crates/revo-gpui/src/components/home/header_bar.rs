use gpui::{div, img, prelude::*, px, Context, Div, IntoElement, Render, Window};

use crate::{
    actions::ComponentEvent,
    components::{icon::icon, panel},
    theme::RevoTheme,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeaderBarState {
    pub demo_mode: bool,
    pub obs_running: bool,
    pub recording: bool,
    pub streaming: bool,
    pub busy: bool,
    pub studio_mode: bool,
    pub realtime_refresh: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub version: String,
}

pub struct HeaderBar {
    pub state: HeaderBarState,
    pub theme: RevoTheme,
}

impl gpui::EventEmitter<ComponentEvent> for HeaderBar {}

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
        let obs_running = self.state.obs_running;
        let recording = self.state.recording;
        let streaming = self.state.streaming;
        let show_obs = !self.state.demo_mode;

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
        let history = button(self.theme, "History", true, Some("history"))
            .id("header-history")
            .on_click(cx.listener(|_, _, _, cx| cx.emit(ComponentEvent::OpenHistory)));
        let settings = button(self.theme, "Settings", true, Some("settings"))
            .id("header-settings")
            .on_click(cx.listener(|_, _, _, cx| cx.emit(ComponentEvent::OpenSettings)));
        let preview = button(self.theme, "1920x1080 Preview", !busy, Some("monitor"))
            .id("header-force-preview")
            .when(!busy, |button| {
                button.on_click(
                    cx.listener(|_, _, _, cx| cx.emit(ComponentEvent::ForcePreviewResolution)),
                )
            });
        let studio = button(self.theme, "Studio", !busy, Some("layers"))
            .id("header-studio-toggle")
            .when(!busy, |button| {
                button.on_click(cx.listener(|this, _, _, cx| {
                    this.state.studio_mode = !this.state.studio_mode;
                    cx.emit(ComponentEvent::SetStudioMode(this.state.studio_mode));
                    cx.notify();
                }))
            });
        let realtime = button(self.theme, "Realtime", !busy, Some("refresh"))
            .id("header-realtime-toggle")
            .when(!busy, |button| {
                button.on_click(cx.listener(|this, _, _, cx| {
                    this.state.realtime_refresh = !this.state.realtime_refresh;
                    cx.emit(ComponentEvent::SetRealtimeRefresh(
                        this.state.realtime_refresh,
                    ));
                    cx.notify();
                }))
            });
        let obs = button(
            self.theme,
            if obs_running { "Stop OBS" } else { "Start OBS" },
            !busy,
            Some("power"),
        )
        .id("header-obs-toggle")
        .when(!busy, |button| {
            button.on_click(cx.listener(move |_, _, _, cx| {
                cx.emit(if obs_running {
                    ComponentEvent::StopObs
                } else {
                    ComponentEvent::StartObs
                });
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
            self.theme.success
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
            .child(realtime)
            .child(studio)
            .child(settings);
        if show_obs {
            actions = actions.child(obs);
        }
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
                    .child(img("brand/favicon.png").w(px(20.)).h(px(20.)).flex_none())
                    .child(div().text_color(self.theme.text_strong).child("RevoStream"))
                    .child(self.state.version.clone())
                    .child(undo)
                    .child(redo)
                    .child(history),
            )
            .child(actions)
    }
}
