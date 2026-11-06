//! Native port of the Tauri browser-interaction auxiliary window
//! (`src/routes/interact/+page.svelte`): a CDP-driven headless browser
//! surface with frame polling and forwarded pointer/wheel/key input.
//!
//! Mock mode placeholder: no CDP session is opened and no frames arrive;
//! the surface stays structural (status text, pointer/key event
//! bookkeeping) and real sessions require the `obs-backend` feature.

use std::sync::{Arc, Mutex};
#[cfg(feature = "obs-backend")]
use std::time::Duration;
use std::time::Instant;

#[cfg(feature = "obs-backend")]
use gpui::ImageFormat;
use gpui::{
    canvas, div, prelude::*, px, Context, FocusHandle, Image as GpuiImage, IntoElement,
    KeyDownEvent, KeyUpEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render,
    ScrollDelta, ScrollWheelEvent, SharedString, Task, Window,
};

#[cfg(feature = "obs-backend")]
use crate::services::{
    interact_close_session, interact_dispatch_key, interact_dispatch_mouse, interact_frame_png,
    interact_reapply, open_interaction_session,
};
use crate::services::{AnyBackend, BackendError, InteractKeyPayload, InteractMousePayload};
use crate::theme::RevoTheme;

/// Viewport used for coordinate scaling (the Svelte window hardcodes the
/// same 1280x720 surface).
pub const INTERACT_VIEWPORT: (u32, u32) = (1280, 720);

/// Frame poll cadence matching the Svelte `setTimeout(..., 120)` loop.
pub const INTERACT_FRAME_POLL_MS: u64 = 120;

/// Pointer-move throttle window matching the Svelte `movePending` guard.
pub const MOVE_THROTTLE_MS: u128 = 30;

/// Documented mock-mode status: no real CDP sessions in mock builds.
pub const MOCK_PLACEHOLDER: &str =
    "Mock backend: interaction sessions unavailable (placeholder surface)";

/// DOM `WheelEvent` line-mode fallback: CDP wheel deltas are pixel-based.
const LINE_HEIGHT_PX: f64 = 40.0;

/// Typed session lifecycle (the Svelte `status`/`error` pair).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InteractSessionPhase {
    Starting,
    Ready,
    Failed(String),
    Closed,
    /// Documented placeholder state for mock builds: the structural surface
    /// renders, but no CDP session exists to drive.
    MockPlaceholder,
}

/// Pure session-phase transition for the session-open result.
pub fn advance_session_phase(result: Result<(), String>) -> InteractSessionPhase {
    match result {
        Ok(()) => InteractSessionPhase::Ready,
        Err(error) => InteractSessionPhase::Failed(error),
    }
}

/// The visible status line: errors win over the friendly status, mirroring
/// the Svelte `{error ?? status}` binding.
pub fn status_line(phase: &InteractSessionPhase, status: &str) -> String {
    match phase {
        InteractSessionPhase::Failed(error) => error.clone(),
        InteractSessionPhase::Closed => "closed".into(),
        InteractSessionPhase::MockPlaceholder => MOCK_PLACEHOLDER.into(),
        InteractSessionPhase::Starting | InteractSessionPhase::Ready => status.to_owned(),
    }
}

/// Pure port of the Svelte `pagePoint` coordinate scaling: window-relative
/// pointer position inside the frame element box maps onto the browser
/// viewport, rounded like `Math.round`.
pub fn page_point(
    client_x: f64,
    client_y: f64,
    bounds_origin_x: f64,
    bounds_origin_y: f64,
    bounds_width: f64,
    bounds_height: f64,
    viewport: (u32, u32),
) -> Option<(i64, i64)> {
    if bounds_width <= 0.0 || bounds_height <= 0.0 {
        return None;
    }
    let scale_x = f64::from(viewport.0) / bounds_width;
    let scale_y = f64::from(viewport.1) / bounds_height;
    Some((
        ((client_x - bounds_origin_x) * scale_x).round() as i64,
        ((client_y - bounds_origin_y) * scale_y).round() as i64,
    ))
}

/// CDP button label for a gpui mouse button (the Svelte mapping
/// `event.button === 1 ? "middle" : event.button === 2 ? "right" : "left"`;
/// wheel events send `none`).
pub fn map_button_label(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Middle => "middle",
        MouseButton::Right => "right",
        MouseButton::Left | MouseButton::Navigate(_) => "left",
    }
}

/// DOM `event.buttons` bitmask bit for one button
/// (left 1, right 2, middle 4, back 8, forward 16).
pub fn button_bit(button: MouseButton) -> i64 {
    match button {
        MouseButton::Left => 1,
        MouseButton::Right => 2,
        MouseButton::Middle => 4,
        MouseButton::Navigate(gpui::NavigationDirection::Back) => 8,
        MouseButton::Navigate(gpui::NavigationDirection::Forward) => 16,
    }
}

/// `buttons` payload for a mouse-move event: gpui reports only the single
/// pressed button (documented approximation of the DOM bitmask).
pub fn buttons_mask(pressed: Option<MouseButton>) -> i64 {
    pressed.map(button_bit).unwrap_or(0)
}

/// Wheel deltas for the CDP `mouseWheel` payload: pixel deltas pass
/// through; line deltas scale by a nominal browser line height.
pub fn wheel_delta_xy(delta: ScrollDelta) -> (f64, f64) {
    match delta {
        ScrollDelta::Pixels(delta) => (delta.x.to_f64(), delta.y.to_f64()),
        ScrollDelta::Lines(delta) => (
            f64::from(delta.x) * LINE_HEIGHT_PX,
            f64::from(delta.y) * LINE_HEIGHT_PX,
        ),
    }
}

/// 30 ms pointer-move throttle (the Svelte `movePending` flag), injectible
/// with the caller's clock so it stays testable.
#[derive(Debug, Default)]
pub struct MoveThrottle {
    last_sent: Option<Instant>,
}

impl MoveThrottle {
    /// Acquires the throttle slot when at least [`MOVE_THROTTLE_MS`] passed
    /// since the last acquire.
    pub fn try_acquire(&mut self, now: Instant) -> bool {
        let allowed = self.last_sent.map_or(true, |last| {
            now.duration_since(last).as_millis() >= MOVE_THROTTLE_MS
        });
        if allowed {
            self.last_sent = Some(now);
        }
        allowed
    }
}

/// One typed forwarded input event (pointer, wheel, or key).
#[derive(Clone, Debug, PartialEq)]
pub enum InteractEvent {
    PointerDown {
        x: i64,
        y: i64,
        button: &'static str,
        buttons: i64,
        click_count: i64,
    },
    PointerUp {
        x: i64,
        y: i64,
        button: &'static str,
        buttons: i64,
        click_count: i64,
    },
    PointerMove {
        x: i64,
        y: i64,
        buttons: i64,
    },
    Wheel {
        x: i64,
        y: i64,
        delta_x: f64,
        delta_y: f64,
    },
    KeyDown {
        key: String,
        code: String,
        key_code: i64,
        text: String,
    },
    KeyUp {
        key: String,
        code: String,
        key_code: i64,
    },
}

impl InteractEvent {
    /// The `obs_interact_mouse` invoke payload (field-for-field).
    pub fn mouse_payload(&self, source_id: String) -> Option<InteractMousePayload> {
        let (kind, x, y, button, buttons, click_count, delta_x, delta_y) = match self {
            Self::PointerDown {
                x,
                y,
                button,
                buttons,
                click_count,
            } => (
                "mousePressed",
                *x,
                *y,
                *button,
                *buttons,
                *click_count,
                0.0,
                0.0,
            ),
            Self::PointerUp {
                x,
                y,
                button,
                buttons,
                click_count,
            } => (
                "mouseReleased",
                *x,
                *y,
                *button,
                *buttons,
                *click_count,
                0.0,
                0.0,
            ),
            Self::PointerMove { x, y, buttons } => {
                ("mouseMoved", *x, *y, "left", *buttons, 0, 0.0, 0.0)
            }
            Self::Wheel {
                x,
                y,
                delta_x,
                delta_y,
            } => ("mouseWheel", *x, *y, "none", 0, 0, *delta_x, *delta_y),
            Self::KeyDown { .. } | Self::KeyUp { .. } => return None,
        };
        Some(InteractMousePayload {
            source_id,
            kind,
            x: x as f64,
            y: y as f64,
            button,
            buttons,
            click_count,
            delta_x,
            delta_y,
        })
    }

    /// The `obs_interact_key` invoke payload (field-for-field).
    pub fn key_payload(&self, source_id: String) -> Option<InteractKeyPayload> {
        match self {
            Self::KeyDown {
                key,
                code,
                key_code,
                text,
            } => Some(InteractKeyPayload {
                source_id,
                kind: if text.is_empty() { "keyDown" } else { "char" },
                key: key.clone(),
                code: code.clone(),
                key_code: *key_code,
                text: text.clone(),
            }),
            Self::KeyUp {
                key,
                code,
                key_code,
            } => Some(InteractKeyPayload {
                source_id,
                kind: "keyUp",
                key: key.clone(),
                code: code.clone(),
                key_code: *key_code,
                text: String::new(),
            }),
            _ => None,
        }
    }
}

/// Pure gpui-keystroke → DOM key payload mapping (the Svelte `mapKey`).
/// gpui keystrokes carry no physical `code` or DOM `keyCode`, so both are
/// approximated from the key name (documented). Returns
/// `(key, code, key_code, text)`.
pub fn map_keystroke(
    key: &str,
    key_char: Option<&str>,
    control: bool,
    alt: bool,
    platform: bool,
) -> (String, String, i64, String) {
    let (key, text) = match key {
        "space" => (" ".to_owned(), key_char.unwrap_or(" ").to_owned()),
        other => (other.to_owned(), key_char.unwrap_or(other).to_owned()),
    };
    let text = if key.chars().count() == 1 && !control && !alt && !platform {
        text
    } else {
        String::new()
    };
    let code = physical_key_code(&key);
    let key_code = approximate_key_code(&key, &text);
    (key, code, key_code, text)
}

/// DOM `event.code` approximation from the logical key name.
pub fn physical_key_code(key: &str) -> String {
    match key {
        " " => return "Space".into(),
        "escape" => return "Escape".into(),
        "enter" => return "Enter".into(),
        "tab" => return "Tab".into(),
        "backspace" => return "Backspace".into(),
        "delete" => return "Delete".into(),
        "left" => return "ArrowLeft".into(),
        "right" => return "ArrowRight".into(),
        "up" => return "ArrowUp".into(),
        "down" => return "ArrowDown".into(),
        "home" => return "Home".into(),
        "end" => return "End".into(),
        "pageup" => return "PageUp".into(),
        "pagedown" => return "PageDown".into(),
        _ => {}
    }
    let chars: Vec<char> = key.chars().collect();
    if chars.len() == 1 {
        if chars[0].is_ascii_alphabetic() {
            return format!("Key{}", chars[0].to_ascii_uppercase());
        }
        if chars[0].is_ascii_digit() {
            return format!("Digit{}", chars[0]);
        }
    }
    String::new()
}

/// DOM `event.keyCode` approximation from the key name / typed text
/// (printable ASCII uses its character code; named keys use their DOM
/// keyCode table; unknown keys report 0).
pub fn approximate_key_code(key: &str, text: &str) -> i64 {
    match key {
        " " | "space" => return 32,
        "enter" => return 13,
        "tab" => return 9,
        "escape" => return 27,
        "backspace" => return 8,
        "delete" => return 46,
        "left" => return 37,
        "up" => return 38,
        "right" => return 39,
        "down" => return 40,
        _ => {}
    }
    let printable = [text, key].iter().find_map(|candidate| {
        let mut chars = candidate.chars();
        match (chars.next(), chars.next()) {
            (Some(ch), None) if !ch.is_control() => Some(ch),
            _ => None,
        }
    });
    printable
        .map(|ch| i64::from(ch.to_ascii_uppercase() as u8))
        .unwrap_or(0)
}

/// Stable held-key identity for the `keysHeld` dedupe set (DOM uses
/// `code:location`; gpui keystrokes are identified by modifiers + key).
pub fn keystroke_identity(control: bool, alt: bool, platform: bool, key: &str) -> String {
    format!("{control}-{alt}-{platform}-{key}")
}

/// The status/last-pointer summary shown in the header.
pub fn header_summary(
    status: &str,
    last_pointer: Option<(i64, i64, i64)>,
    keys_held: usize,
) -> String {
    match last_pointer {
        Some((x, y, buttons)) => {
            format!("{status} — pointer ({x}, {y}) buttons {buttons}, keys {keys_held}")
        }
        None => format!("{status} — pointer (-, -), keys {keys_held}"),
    }
}

pub struct InteractWindow {
    pub source_id: Option<String>,
    pub viewport: (u32, u32),
    pub frame_uri: Option<String>,
    pub frame_image: Option<Arc<GpuiImage>>,
    pub phase: InteractSessionPhase,
    pub status: String,
    pub keys_held: Vec<String>,
    pub last_pointer: Option<(i64, i64, i64)>,
    theme: RevoTheme,
    #[cfg_attr(not(feature = "obs-backend"), allow(dead_code))]
    backend: AnyBackend,
    /// Bounds of the frame surface in window coordinates, refreshed on
    /// every paint by the overlay canvas (the Svelte `img` bounding rect).
    frame_bounds: Arc<Mutex<Option<(f64, f64, f64, f64)>>>,
    move_throttle: MoveThrottle,
    surface_focus: FocusHandle,
    /// Owned session/frame-poll task; dropped (cancelling it) with the view.
    #[cfg_attr(not(feature = "obs-backend"), allow(dead_code))]
    poll_task: Option<Task<()>>,
}

impl InteractWindow {
    pub fn new(backend: AnyBackend, source_id: Option<String>, cx: &mut Context<Self>) -> Self {
        let theme = RevoTheme::future();
        let mut window = Self {
            source_id: source_id.clone(),
            viewport: INTERACT_VIEWPORT,
            frame_uri: None,
            frame_image: None,
            phase: InteractSessionPhase::Starting,
            status: "Starting interaction session...".into(),
            keys_held: Vec::new(),
            last_pointer: None,
            theme,
            frame_bounds: Arc::new(Mutex::new(None)),
            move_throttle: MoveThrottle::default(),
            surface_focus: cx.focus_handle(),
            poll_task: None,
            backend,
        };
        window.start_session(cx);
        window
    }

    /// Opens the CDP session (obs backend, browser source required) or falls
    /// back to the documented mock placeholder, then starts the 120 ms
    /// frame-poll loop on success.
    fn start_session(&mut self, cx: &mut Context<'_, Self>) {
        if self.source_id.is_none() {
            self.phase = InteractSessionPhase::Failed("No source selected".into());
            cx.notify();
            return;
        }

        #[cfg(feature = "obs-backend")]
        if matches!(self.backend, AnyBackend::Obs(_)) {
            let backend = self.backend.clone();
            let source_id = self.source_id.clone().unwrap_or_default();
            let start = cx.spawn(async move |window, cx| {
                let opened = cx
                    .background_executor()
                    .spawn(async move { open_interaction_session(&backend, &source_id) })
                    .await;
                let outcome = opened.map_err(|error| error.to_string());
                let alive = window
                    .update(cx, |window, cx| {
                        window.phase = advance_session_phase(outcome.clone());
                        if window.phase == InteractSessionPhase::Ready {
                            window.status = "Interaction session ready".into();
                        }
                        cx.notify();
                    })
                    .is_ok();
                if !alive || outcome.is_err() {
                    return;
                }
                window
                    .update(cx, |window, cx| {
                        window.poll_task = Some(window.spawn_frame_poll(cx));
                    })
                    .ok();
            });
            self.poll_task = Some(start);
            return;
        }

        self.phase = InteractSessionPhase::MockPlaceholder;
        self.status = MOCK_PLACEHOLDER.into();
        let _ = cx;
    }

    /// Bounded frame loop: newest CDP frame lands on the foreground context
    /// every 120 ms; the owned task cancels on view drop.
    #[cfg(feature = "obs-backend")]
    fn spawn_frame_poll(&self, cx: &mut Context<'_, Self>) -> Task<()> {
        let backend = self.backend.clone();
        let source_id = self.source_id.clone().unwrap_or_default();
        cx.spawn(async move |window, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(INTERACT_FRAME_POLL_MS))
                .await;
            let frame = cx
                .background_executor()
                .spawn(async move { interact_frame_png(&backend, &source_id) })
                .await;
            let alive = window
                .update(cx, |window, cx| window.apply_frame(frame, cx))
                .is_ok();
            if !alive {
                return;
            }
        })
    }

    /// Applies one frame-poll round (failure swaps the phase, like the
    /// Svelte loop which stops on thrown errors).
    pub fn apply_frame(&mut self, frame: Result<String, BackendError>, cx: &mut Context<'_, Self>) {
        match frame {
            Ok(data_uri) => {
                self.frame_uri = Some(data_uri.clone());
                #[cfg(feature = "obs-backend")]
                {
                    self.frame_image = crate::services::decode_png_data_uri(&data_uri)
                        .map(|bytes| Arc::new(GpuiImage::from_bytes(ImageFormat::Png, bytes)));
                }
                self.status = "Receiving browser frames (120ms loop)".into();
            }
            Err(error) => {
                self.phase = InteractSessionPhase::Failed(format!("frame poll failed: {error}"));
            }
        }
        cx.notify();
    }

    /// Re-runs custom CSS/JS injection (the Svelte "Reapply code" button).
    pub fn reapply(&mut self, cx: &mut Context<'_, Self>) {
        #[cfg(feature = "obs-backend")]
        if matches!(self.backend, AnyBackend::Obs(_)) {
            let backend = self.backend.clone();
            let source_id = self.source_id.clone().unwrap_or_default();
            cx.background_executor()
                .spawn(async move {
                    let _ = interact_reapply(&backend, &source_id);
                })
                .detach();
            self.status = "Reapplied custom code".into();
            cx.notify();
            return;
        }
        let _ = cx;
    }

    /// Central typed-event handler: pointer/key bookkeeping stays structural
    /// in every mode; remote dispatch only happens for the obs backend.
    pub fn handle_interact_event(&mut self, event: InteractEvent, cx: &mut Context<'_, Self>) {
        match &event {
            InteractEvent::PointerDown { x, y, buttons, .. }
            | InteractEvent::PointerUp { x, y, buttons, .. }
            | InteractEvent::PointerMove { x, y, buttons } => {
                self.last_pointer = Some((*x, *y, *buttons));
            }
            InteractEvent::Wheel { x, y, .. } => {
                self.last_pointer = Some((*x, *y, 0));
            }
            InteractEvent::KeyDown { key, code, .. } => {
                self.status = format!("Key down: {key} ({code})");
            }
            InteractEvent::KeyUp { key, .. } => {
                self.status = format!("Key up: {key}");
            }
        }
        self.dispatch_remote(&event, cx);
        cx.notify();
    }

    /// Fire-and-forget remote dispatch (the Svelte `.catch(() => undefined)`).
    fn dispatch_remote(&self, event: &InteractEvent, cx: &mut Context<'_, Self>) {
        #[cfg(feature = "obs-backend")]
        if matches!(self.backend, AnyBackend::Obs(_)) {
            let backend = self.backend.clone();
            let source_id = self.source_id.clone().unwrap_or_default();
            match event {
                InteractEvent::KeyDown { .. } | InteractEvent::KeyUp { .. } => {
                    let payload = event.key_payload(source_id);
                    cx.background_executor()
                        .spawn(async move {
                            if let Some(payload) = payload {
                                let _ = interact_dispatch_key(&backend, payload);
                            }
                        })
                        .detach();
                }
                _ => {
                    let payload = event.mouse_payload(source_id);
                    cx.background_executor()
                        .spawn(async move {
                            if let Some(payload) = payload {
                                let _ = interact_dispatch_mouse(&backend, payload);
                            }
                        })
                        .detach();
                }
            }
        }
        let _ = (event, cx);
    }

    /// Window-relative pointer position scaled onto the browser viewport
    /// using the last-painted frame bounds (the Svelte `pagePoint`).
    fn page_point_from(&self, position: gpui::Point<gpui::Pixels>) -> Option<(i64, i64)> {
        let (origin_x, origin_y, width, height) = (*self.frame_bounds.lock().ok()?)?;
        page_point(
            position.x.to_f64(),
            position.y.to_f64(),
            origin_x,
            origin_y,
            width,
            height,
            self.viewport,
        )
    }
}

impl Drop for InteractWindow {
    fn drop(&mut self) {
        #[cfg(feature = "obs-backend")]
        if matches!(self.backend, AnyBackend::Obs(_)) {
            // Teardown cannot await: the CDP close runs on a detached thread
            // so window close never blocks the foreground on a browser kill.
            let backend = self.backend.clone();
            let source_id = self.source_id.clone().unwrap_or_default();
            std::thread::spawn(move || {
                interact_close_session(&backend, &source_id);
            });
        }
    }
}

impl Render for InteractWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let theme = self.theme;
        let summary = header_summary(
            &status_line(&self.phase, &self.status),
            self.last_pointer,
            self.keys_held.len(),
        );

        let frame_bounds_slot = Arc::clone(&self.frame_bounds);
        let frame_overlay = canvas(
            move |bounds, _, _| {
                if let Ok(mut slot) = frame_bounds_slot.lock() {
                    *slot = Some((
                        bounds.origin.x.to_f64(),
                        bounds.origin.y.to_f64(),
                        bounds.size.width.to_f64(),
                        bounds.size.height.to_f64(),
                    ));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();

        let mut surface = div()
            .id("interact-surface")
            .relative()
            .flex_1()
            .min_h(px(0.))
            .items_center()
            .justify_center()
            .overflow_hidden()
            .bg(theme.background)
            .track_focus(&self.surface_focus)
            .key_context("Interact")
            .on_click(cx.listener(|this, _, window, cx| {
                window.focus(&this.surface_focus.clone());
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.is_held {
                    return;
                }
                let keystroke = &event.keystroke;
                let (key, code, key_code, text) = map_keystroke(
                    &keystroke.key,
                    keystroke.key_char.as_deref(),
                    keystroke.modifiers.control,
                    keystroke.modifiers.alt,
                    keystroke.modifiers.platform,
                );
                let identity = keystroke_identity(
                    keystroke.modifiers.control,
                    keystroke.modifiers.alt,
                    keystroke.modifiers.platform,
                    &keystroke.key,
                );
                if this.keys_held.contains(&identity) {
                    return;
                }
                this.keys_held.push(identity);
                this.handle_interact_event(
                    InteractEvent::KeyDown {
                        key,
                        code,
                        key_code,
                        text,
                    },
                    cx,
                );
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, cx| {
                let keystroke = &event.keystroke;
                let (key, code, key_code, _text) = map_keystroke(
                    &keystroke.key,
                    keystroke.key_char.as_deref(),
                    keystroke.modifiers.control,
                    keystroke.modifiers.alt,
                    keystroke.modifiers.platform,
                );
                let identity = keystroke_identity(
                    keystroke.modifiers.control,
                    keystroke.modifiers.alt,
                    keystroke.modifiers.platform,
                    &keystroke.key,
                );
                this.keys_held.retain(|held| held != &identity);
                this.handle_interact_event(
                    InteractEvent::KeyUp {
                        key,
                        code,
                        key_code,
                    },
                    cx,
                );
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                let Some((x, y)) = this.page_point_from(event.position) else {
                    return;
                };
                if !this.move_throttle.try_acquire(Instant::now()) {
                    return;
                }
                this.handle_interact_event(
                    InteractEvent::PointerMove {
                        x,
                        y,
                        buttons: buttons_mask(event.pressed_button),
                    },
                    cx,
                );
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                let Some((x, y)) = this.page_point_from(event.position) else {
                    return;
                };
                let (delta_x, delta_y) = wheel_delta_xy(event.delta);
                this.handle_interact_event(
                    InteractEvent::Wheel {
                        x,
                        y,
                        delta_x,
                        delta_y,
                    },
                    cx,
                );
            }));
        for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            surface = surface
                .on_mouse_down(
                    button,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        let Some((x, y)) = this.page_point_from(event.position) else {
                            return;
                        };
                        this.handle_interact_event(
                            InteractEvent::PointerDown {
                                x,
                                y,
                                button: map_button_label(button),
                                buttons: button_bit(button),
                                click_count: event.click_count.max(1) as i64,
                            },
                            cx,
                        );
                    }),
                )
                .on_mouse_up(
                    button,
                    cx.listener(move |this, event: &MouseUpEvent, _, cx| {
                        let Some((x, y)) = this.page_point_from(event.position) else {
                            return;
                        };
                        this.handle_interact_event(
                            InteractEvent::PointerUp {
                                x,
                                y,
                                button: map_button_label(button),
                                buttons: 0,
                                click_count: event.click_count.max(1) as i64,
                            },
                            cx,
                        );
                    }),
                );
        }

        surface = match &self.frame_image {
            Some(frame) => surface.child(
                gpui::img(Arc::clone(frame))
                    .size_full()
                    .object_fit(gpui::ObjectFit::Contain),
            ),
            None => surface.child(div().text_color(theme.text_muted).child(match &self.phase {
                InteractSessionPhase::Failed(error) => SharedString::from(error.clone()),
                _ => SharedString::from("Waiting for first frame..."),
            })),
        };
        surface = surface.child(frame_overlay);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.text)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_color(theme.text_muted)
                            .text_size(px(12.))
                            .child(SharedString::from(summary)),
                    )
                    .child(
                        div()
                            .id("interact-reapply")
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .text_color(theme.text)
                            .text_size(px(12.))
                            .child("Reapply code")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.reapply(cx);
                            })),
                    ),
            )
            .child(surface)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn page_point_scales_window_coordinates_onto_the_viewport() {
        // Exact Svelte port: round((clientX - left) * (viewport / bounds)).
        let point = page_point(640.0, 360.0, 0.0, 0.0, 640.0, 360.0, (1280, 720));
        assert_eq!(point, Some((1280, 720)));

        let point = page_point(100.0, 50.0, 20.0, 10.0, 320.0, 180.0, (1280, 720));
        assert_eq!(point, Some((320, 160)));

        assert_eq!(page_point(1.0, 1.0, 0.0, 0.0, 0.0, 0.0, (1280, 720)), None);
    }

    #[test]
    fn move_throttle_suppresses_events_inside_the_thirty_ms_window() {
        let mut throttle = MoveThrottle::default();
        let start = Instant::now();
        assert!(throttle.try_acquire(start));
        assert!(!throttle.try_acquire(start + Duration::from_millis(29)));
        assert!(throttle.try_acquire(start + Duration::from_millis(30)));
        assert!(!throttle.try_acquire(start + Duration::from_millis(59)));
    }

    #[test]
    fn session_phase_transitions_track_start_failure_and_recovery() {
        let phase = advance_session_phase(Ok(()));
        assert_eq!(phase, InteractSessionPhase::Ready);

        let phase = advance_session_phase(Err("cdp down".into()));
        assert_eq!(phase, InteractSessionPhase::Failed("cdp down".into()));

        let recovered = advance_session_phase(Ok(()));
        assert_eq!(recovered, InteractSessionPhase::Ready);
    }

    #[test]
    fn status_line_prefers_errors_and_closed_over_status() {
        assert_eq!(
            status_line(&InteractSessionPhase::Ready, "Interaction session ready"),
            "Interaction session ready"
        );
        assert_eq!(
            status_line(&InteractSessionPhase::Failed("boom".into()), "ready"),
            "boom"
        );
        assert_eq!(
            status_line(&InteractSessionPhase::Closed, "ready"),
            "closed"
        );
        assert_eq!(
            status_line(&InteractSessionPhase::MockPlaceholder, "ready"),
            MOCK_PLACEHOLDER
        );
    }

    #[test]
    fn typed_events_map_to_svelte_payloads_field_for_field() {
        let press = InteractEvent::PointerDown {
            x: 90,
            y: 65,
            button: "left",
            buttons: 1,
            click_count: 1,
        };
        assert_eq!(
            press.mouse_payload("src".into()),
            Some(InteractMousePayload {
                source_id: "src".into(),
                kind: "mousePressed",
                x: 90.0,
                y: 65.0,
                button: "left",
                buttons: 1,
                click_count: 1,
                delta_x: 0.0,
                delta_y: 0.0,
            })
        );
        assert!(press.key_payload("src".into()).is_none());

        let wheel = InteractEvent::Wheel {
            x: 5,
            y: 6,
            delta_x: 1.5,
            delta_y: -2.0,
        };
        assert_eq!(
            wheel.mouse_payload("src".into()),
            Some(InteractMousePayload {
                source_id: "src".into(),
                kind: "mouseWheel",
                x: 5.0,
                y: 6.0,
                button: "none",
                buttons: 0,
                click_count: 0,
                delta_x: 1.5,
                delta_y: -2.0,
            })
        );

        let key = InteractEvent::KeyDown {
            key: "a".into(),
            code: "KeyA".into(),
            key_code: 65,
            text: "a".into(),
        };
        assert_eq!(
            key.key_payload("src".into()),
            Some(InteractKeyPayload {
                source_id: "src".into(),
                kind: "char",
                key: "a".into(),
                code: "KeyA".into(),
                key_code: 65,
                text: "a".into(),
            })
        );

        let up = InteractEvent::KeyUp {
            key: "a".into(),
            code: "KeyA".into(),
            key_code: 65,
        };
        assert_eq!(up.key_payload("src".into()).unwrap().kind, "keyUp");
    }

    #[test]
    fn button_labels_and_masks_match_dom_semantics() {
        assert_eq!(map_button_label(MouseButton::Left), "left");
        assert_eq!(map_button_label(MouseButton::Middle), "middle");
        assert_eq!(map_button_label(MouseButton::Right), "right");
        assert_eq!(button_bit(MouseButton::Left), 1);
        assert_eq!(button_bit(MouseButton::Right), 2);
        assert_eq!(button_bit(MouseButton::Middle), 4);
        assert_eq!(buttons_mask(None), 0);
        assert_eq!(buttons_mask(Some(MouseButton::Right)), 2);
    }

    #[test]
    fn wheel_deltas_convert_lines_to_pixels() {
        assert_eq!(
            wheel_delta_xy(ScrollDelta::Pixels(gpui::point(px(3.0), px(-4.0)))),
            (3.0, -4.0)
        );
        assert_eq!(
            wheel_delta_xy(ScrollDelta::Lines(gpui::point(1.0, -2.0))),
            (LINE_HEIGHT_PX, -2.0 * LINE_HEIGHT_PX)
        );
    }

    #[test]
    fn keystroke_mapping_covers_text_keys_named_keys_and_modifiers() {
        let (key, code, key_code, text) = map_keystroke("a", Some("a"), false, false, false);
        assert_eq!(
            (key.as_str(), code.as_str(), key_code, text.as_str()),
            ("a", "KeyA", 65, "a")
        );

        let (key, code, key_code, text) = map_keystroke("space", Some(" "), false, false, false);
        assert_eq!(
            (key.as_str(), code.as_str(), key_code, text.as_str()),
            (" ", "Space", 32, " ")
        );

        let (key, _code, key_code, text) = map_keystroke("escape", None, false, false, false);
        assert_eq!((key.as_str(), key_code, text.as_str()), ("escape", 27, ""));

        let (_key, code, _key_code, text) = map_keystroke("a", Some("a"), true, false, false);
        assert_eq!((code.as_str(), text.as_str()), ("KeyA", ""));
    }

    #[test]
    fn keystroke_identity_distinguishes_modifier_combinations() {
        assert_eq!(
            keystroke_identity(false, false, false, "a"),
            keystroke_identity(false, false, false, "a")
        );
        assert_ne!(
            keystroke_identity(false, false, false, "a"),
            keystroke_identity(true, false, false, "a")
        );
    }

    #[test]
    fn header_summary_formats_pointer_and_keys() {
        assert_eq!(
            header_summary("ready", Some((10, 20, 1)), 2),
            "ready — pointer (10, 20) buttons 1, keys 2"
        );
        assert_eq!(
            header_summary("ready", None, 0),
            "ready — pointer (-, -), keys 0"
        );
    }
}
