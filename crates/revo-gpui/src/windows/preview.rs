//! Native port of the Tauri preview auxiliary window
//! (`src/routes/preview/+page.svelte`): a self-contained window polling the
//! libobs preview (or one source's output) as raw RGBA frames on the
//! background executor.
//!
//! Mock mode placeholder: the mock backend has no libobs surface to capture,
//! so the poll loop is not started and the surface shows the documented
//! placeholder text instead of frames.

use std::sync::Arc;
#[cfg(feature = "obs-backend")]
use std::time::Duration;

use gpui::{
    div, prelude::*, Context, IntoElement, Render, RenderImage, SharedString, Task, Window,
};

#[cfg(feature = "obs-backend")]
use crate::services::capture_preview_frame;
use crate::services::preview_frame_image;
use crate::services::{AnyBackend, BackendError};
use crate::theme::RevoTheme;

/// Frame poll cadence matching the Svelte loop's `scheduleFrame(66)`.
pub const PREVIEW_POLL_INTERVAL_MS: u64 = 66;

/// Documented mock-mode surface: no libobs means no frames to poll.
pub const MOCK_PLACEHOLDER: &str =
    "Mock backend: live preview frames unavailable (placeholder surface)";

/// Typed frame-pipeline state: `Waiting` until the first capture lands,
/// `Ready` once the newest frame is on screen, `Failed` while the capture
/// path errors (mirrors the Svelte error line under the preview image).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreviewFrameState {
    Waiting,
    Ready,
    Failed(String),
}

/// Pure capture-result transition for [`PreviewFrameState`].
pub fn advance_frame_state(captured: Result<(), String>) -> PreviewFrameState {
    match captured {
        Ok(()) => PreviewFrameState::Ready,
        Err(error) => PreviewFrameState::Failed(error),
    }
}

/// Status line for the current backend: mock builds surface a documented
/// placeholder instead of a live capture status.
pub fn initial_status(backend: &AnyBackend) -> String {
    match backend {
        #[cfg(feature = "obs-backend")]
        AnyBackend::Obs(_) => "Preview: polling libobs frames (66ms loop)".into(),
        #[cfg(feature = "obs-backend")]
        AnyBackend::Mock(_) => MOCK_PLACEHOLDER.into(),
        #[cfg(not(feature = "obs-backend"))]
        AnyBackend::Mock(_) => MOCK_PLACEHOLDER.into(),
    }
}

pub struct PreviewWindow {
    pub source_id: Option<String>,
    pub frame_image: Option<Arc<RenderImage>>,
    pub frame_state: PreviewFrameState,
    pub status: String,
    pub theme: RevoTheme,
    /// Owned poll-loop task. Dropping the view drops the handle, which
    /// cancels the loop immediately.
    poll_task: Option<Task<()>>,
}

impl PreviewWindow {
    pub fn new(backend: AnyBackend, source_id: Option<String>, cx: &mut Context<Self>) -> Self {
        let theme = RevoTheme::future();
        let mut window = Self {
            source_id,
            frame_image: None,
            frame_state: PreviewFrameState::Waiting,
            status: initial_status(&backend),
            theme,
            poll_task: None,
        };
        window.poll_task = Some(window.spawn_poll(backend, cx));
        window
    }

    /// Starts the bounded frame-poll loop only when a libobs backend is
    /// linked; the loop captures on the background executor and lands the
    /// newest frame on the foreground context. Cancels on view drop.
    fn spawn_poll(&self, backend: AnyBackend, cx: &mut Context<Self>) -> Task<()> {
        #[cfg(feature = "obs-backend")]
        if matches!(backend, AnyBackend::Obs(_)) {
            let source_id = self.source_id.clone();
            return cx.spawn(async move |window, cx| loop {
                cx.background_executor()
                    .timer(Duration::from_millis(PREVIEW_POLL_INTERVAL_MS))
                    .await;
                let backend = backend.clone();
                let source_id = source_id.clone();
                let captured = cx
                    .background_executor()
                    .spawn(async move { capture_preview_frame(&backend, source_id.as_deref()) })
                    .await;
                let alive = window
                    .update(cx, |window, cx| window.apply_capture(captured, cx))
                    .is_ok();
                if !alive {
                    return;
                }
            });
        }
        #[cfg(feature = "obs-backend")]
        {
            let _ = cx;
        }
        #[cfg(not(feature = "obs-backend"))]
        {
            let _ = (backend, cx);
        }
        Task::ready(())
    }

    /// Applies one capture round: refreshes the newest frame on success or
    /// records the failure text (the Svelte window keeps the last frame and
    /// shows the error underneath).
    pub fn apply_capture(
        &mut self,
        captured: Result<revo_core::ipc::PreviewFrame, BackendError>,
        cx: &mut Context<'_, Self>,
    ) {
        match captured {
            Ok(frame) => {
                self.frame_image = preview_frame_image(frame);
                self.frame_state = advance_frame_state(Ok(()));
                self.status = "Live preview".into();
            }
            Err(error) => {
                self.frame_state = advance_frame_state(Err(error.to_string()));
            }
        }
        cx.notify();
    }
}

/// The status/error line shown under the frame surface.
pub fn status_line(status: &str, frame_state: &PreviewFrameState) -> String {
    match frame_state {
        PreviewFrameState::Failed(error) => error.clone(),
        _ => status.to_owned(),
    }
}

impl Render for PreviewWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let theme = self.theme;
        let status_text = status_line(&self.status, &self.frame_state);

        let mut root = div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .size_full()
            .bg(theme.background)
            .text_color(theme.text);

        root = match &self.frame_image {
            Some(frame) => root.child(
                div().id("preview-window-frame").size_full().child(
                    gpui::img(Arc::clone(frame))
                        .size_full()
                        .object_fit(gpui::ObjectFit::Contain),
                ),
            ),
            None => root.child(
                div()
                    .text_color(theme.text_muted)
                    .child(status_placeholder_text(&self.frame_state)),
            ),
        };

        root.child(
            div()
                .text_color(match &self.frame_state {
                    PreviewFrameState::Failed(_) => theme.danger,
                    _ => theme.text_muted,
                })
                .child(SharedString::from(status_text)),
        )
    }
}

/// Surface text for the frame area while no decoded frame is available.
pub fn status_placeholder_text(frame_state: &PreviewFrameState) -> &'static str {
    match frame_state {
        PreviewFrameState::Waiting => "Waiting for first frame...",
        PreviewFrameState::Ready | PreviewFrameState::Failed(_) => MOCK_PLACEHOLDER,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_state_transitions_cover_waiting_ready_and_failed() {
        let state = advance_frame_state(Ok(()));
        assert_eq!(state, PreviewFrameState::Ready);

        let state = advance_frame_state(Err("capture failed".into()));
        assert_eq!(state, PreviewFrameState::Failed("capture failed".into()));

        let recovered = advance_frame_state(Ok(()));
        assert_eq!(recovered, PreviewFrameState::Ready);
    }

    #[test]
    fn mock_backend_starts_in_placeholder_state_without_loop() {
        assert_eq!(initial_status(&AnyBackend::mock()), MOCK_PLACEHOLDER);
    }

    #[test]
    fn status_line_surfaces_capture_errors_over_status_text() {
        assert_eq!(
            status_line("Live preview", &PreviewFrameState::Ready),
            "Live preview"
        );
        assert_eq!(
            status_line("Live preview", &PreviewFrameState::Failed("boom".into())),
            "boom"
        );
        assert_eq!(
            status_line("Starting", &PreviewFrameState::Waiting),
            "Starting"
        );
    }

    #[test]
    fn placeholder_text_tracks_waiting_state() {
        assert_eq!(
            status_placeholder_text(&PreviewFrameState::Waiting),
            "Waiting for first frame..."
        );
        assert_eq!(
            status_placeholder_text(&PreviewFrameState::Ready),
            MOCK_PLACEHOLDER
        );
    }
}
