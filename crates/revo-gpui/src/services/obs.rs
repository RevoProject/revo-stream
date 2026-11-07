//! Real libobs backend wiring for the native GPUI shell, gated behind the
//! `obs-backend` feature so default builds (and `scripts/core/verify.sh`)
//! stay green without libobs/openssl host packages.
//!
//! The command backend itself lives in `revo_backend::obs_service` so the
//! native engine process can host the same runtime; this module keeps only
//! the GPUI-specific pieces: the main-thread platform trampoline and the
//! typed service-call helpers used by the preview and interaction windows.

use std::sync::mpsc;

use revo_backend::runtime::lifecycle::ObsPlatform;
use revo_core::service::BackendError;

pub use revo_backend::obs_service::{ObsBackendRuntime, PREVIEW_CAPTURE_INTERVAL_MS};

/// libobs platform hooks for the native GPUI application.
///
/// GPUI owns the process main thread. libobs asserts macOS main-dispatch for
/// hotkey platform init, so on macOS the hooks are marshalled onto the GPUI
/// foreground executor (which dispatches on the main thread) through a
/// channel trampoline: the worker blocks on a reply channel exactly like the
/// Tauri adapter blocks on `run_on_main_thread`. On other platforms libobs
/// startup/shutdown run directly on the service worker thread.
pub struct GpuiObsPlatform {
    /// Send-side of the main-thread job queue drained by a foreground-executor
    /// task spawned in [`GpuiObsPlatform::new`]. The `ForegroundExecutor` is
    /// `!Send`, which is why only this sender is retained.
    main_jobs: mpsc::Sender<Box<dyn FnOnce() + Send>>,
}

impl GpuiObsPlatform {
    pub fn new(executor: gpui::ForegroundExecutor) -> Self {
        let (main_jobs, receiver) = mpsc::channel::<Box<dyn FnOnce() + Send>>();
        executor
            .spawn(async move {
                while let Ok(job) = receiver.recv() {
                    job();
                }
            })
            .detach();
        Self { main_jobs }
    }

    /// Runs `job` on the GPUI main thread and blocks the worker until it
    /// completes. Mirrors the Tauri adapter's `run_on_main_thread` + reply
    /// channel for the macOS libobs lifecycle hooks.
    #[cfg(target_os = "macos")]
    fn dispatch_on_main<R: Send + 'static>(
        &self,
        job: impl FnOnce() -> R + Send + 'static,
    ) -> Result<R, String> {
        let (reply, receiver) = mpsc::channel::<R>();
        self.main_jobs
            .send(Box::new(move || {
                let _ = reply.send(job());
            }))
            .map_err(|_| "main-thread dispatcher dropped: GPUI event loop closed".to_string())?;
        receiver
            .recv()
            .map_err(|e| format!("main-thread job dropped: {e}"))
    }
}

impl ObsPlatform for GpuiObsPlatform {
    fn startup(&self, locale: &std::ffi::CStr, config: &std::ffi::CStr) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            // libobs hotkey platform init (HIToolbox/TSM) asserts the main
            // dispatch queue, so block this worker until main-thread startup
            // completes — same contract as the Tauri adapter.
            let locale = locale.to_owned();
            let config = config.to_owned();
            self.dispatch_on_main(move || {
                // SAFETY: The owned C strings outlive this call, and libobs
                // startup runs on the required macOS main thread.
                unsafe {
                    revo_lib::obs::obs_startup(
                        locale.as_ptr(),
                        config.as_ptr(),
                        std::ptr::null_mut(),
                    )
                }
            })?
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = &self.main_jobs;
            // SAFETY: The caller serializes the libobs lifecycle, and both C
            // strings remain valid for the duration of obs_startup.
            if unsafe {
                revo_lib::obs::obs_startup(locale.as_ptr(), config.as_ptr(), std::ptr::null_mut())
            } {
                Ok(())
            } else {
                Err("obs_startup failed".to_string())
            }
        }
    }

    fn shutdown(&self) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            // Match obs_startup: teardown touches HIToolbox on macOS.
            self.dispatch_on_main(move || {
                // SAFETY: The caller established that the libobs core is
                // active, and teardown runs on the required macOS main thread.
                unsafe { revo_lib::obs::obs_shutdown() };
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = &self.main_jobs;
            // SAFETY: The caller established that the libobs core is
            // active, and teardown runs on the same serialized lifecycle.
            unsafe { revo_lib::obs::obs_shutdown() };
            Ok(())
        }
    }
}

/// Captures a preview frame as a PNG data-URI string.
///
/// Without a `source_id` the current scene composite is captured (the
/// workspace preview loop); with one, the matching source is captured
/// individually (the preview auxiliary window). Runs through
/// [`revo_core::service::BackendService::call`] on the backend worker
/// thread: the libobs capture runs with the runtime lock held on a
/// background thread and the PNG encoding completes without ever blocking
/// the GPUI foreground executor.
///
/// Returns the `data:image/png;base64,...` frame, or an error for the mock
/// backend which has no libobs surface to capture.
pub fn capture_preview_png(
    backend: &super::AnyBackend,
    source_id: Option<&str>,
) -> Result<String, BackendError> {
    let source_id = source_id.map(str::to_owned);
    match backend {
        super::AnyBackend::Obs(service) => service.call(move |backend: &mut ObsBackendRuntime| {
            backend.capture_preview_png(source_id.as_deref())
        }),
        super::AnyBackend::Mock(_) => Err(BackendError::Runtime(
            "preview capture requires the obs backend".into(),
        )),
    }
}

/// Decodes a `data:image/png;base64,...` frame into raw PNG bytes so it can
/// be handed to GPUI as an in-memory image without touching the filesystem.
#[cfg(feature = "obs-backend")]
pub fn decode_png_data_uri(data_uri: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    const PREFIX: &str = "data:image/png;base64,";
    let payload = data_uri.strip_prefix(PREFIX)?;
    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .ok()
}

/// Resolves the browser source's interaction settings and opens a persistent
/// headless-Chromium CDP session for it. Lock-scoped settings reading and the
/// session launch both run on the backend worker thread.
#[cfg(feature = "obs-backend")]
pub fn open_interaction_session(
    backend: &super::AnyBackend,
    source_id: &str,
) -> Result<(), BackendError> {
    let source_id = source_id.to_owned();
    match backend {
        super::AnyBackend::Obs(service) => {
            service.call(move |backend: &mut ObsBackendRuntime| backend.interact_open(&source_id))
        }
        super::AnyBackend::Mock(_) => Err(BackendError::Runtime(
            "interaction sessions require the obs backend".into(),
        )),
    }
}

/// Fetches the newest browser-source frame as a data-URI string from the
/// source's CDP session (backend worker thread; blocks on the session's
/// screencast wait, never on the GPUI foreground executor).
#[cfg(feature = "obs-backend")]
pub fn interact_frame_png(
    backend: &super::AnyBackend,
    source_id: &str,
) -> Result<String, BackendError> {
    let source_id = source_id.to_owned();
    match backend {
        super::AnyBackend::Obs(service) => service.call(move |backend: &mut ObsBackendRuntime| {
            backend
                .interact_frame(&source_id)?
                .ok_or_else(|| BackendError::Runtime("no interaction frame yet".into()))
        }),
        super::AnyBackend::Mock(_) => Err(BackendError::Runtime(
            "interaction frames require the obs backend".into(),
        )),
    }
}

/// Forwards one typed mouse interaction to the browser source's CDP session
/// with the Svelte `obs_interact_mouse` invoke payload fields.
#[cfg(feature = "obs-backend")]
pub fn interact_dispatch_mouse(
    backend: &super::AnyBackend,
    payload: super::InteractMousePayload,
) -> Result<(), BackendError> {
    match backend {
        super::AnyBackend::Obs(service) => service.call(move |backend: &mut ObsBackendRuntime| {
            backend.interact_mouse(
                &payload.source_id,
                payload.kind,
                payload.x,
                payload.y,
                payload.button,
                payload.buttons,
                payload.click_count,
                payload.delta_x,
                payload.delta_y,
            )
        }),
        super::AnyBackend::Mock(_) => Err(BackendError::Runtime(
            "interaction input requires the obs backend".into(),
        )),
    }
}

/// Forwards one typed key interaction to the browser source's CDP session
/// with the Svelte `obs_interact_key` invoke payload fields.
#[cfg(feature = "obs-backend")]
pub fn interact_dispatch_key(
    backend: &super::AnyBackend,
    payload: super::InteractKeyPayload,
) -> Result<(), BackendError> {
    match backend {
        super::AnyBackend::Obs(service) => service.call(move |backend: &mut ObsBackendRuntime| {
            backend.interact_key(
                &payload.source_id,
                payload.kind,
                &payload.key,
                &payload.code,
                payload.key_code,
                &payload.text,
            )
        }),
        super::AnyBackend::Mock(_) => Err(BackendError::Runtime(
            "interaction input requires the obs backend".into(),
        )),
    }
}

/// Re-runs the session's custom CSS/JS injection (Svelte
/// `obs_interact_reapply`).
#[cfg(feature = "obs-backend")]
pub fn interact_reapply(backend: &super::AnyBackend, source_id: &str) -> Result<(), BackendError> {
    let source_id = source_id.to_owned();
    match backend {
        super::AnyBackend::Obs(service) => service
            .call(move |backend: &mut ObsBackendRuntime| backend.interact_reapply(&source_id)),
        super::AnyBackend::Mock(_) => Err(BackendError::Runtime(
            "interaction reapply requires the obs backend".into(),
        )),
    }
}

/// Closes the source's CDP session and its headless browser (Svelte
/// `obs_interact_close`). Fire-and-forget: errors cannot reach the UI once
/// the window is gone.
#[cfg(feature = "obs-backend")]
pub fn interact_close_session(backend: &super::AnyBackend, source_id: &str) {
    if let super::AnyBackend::Obs(service) = backend {
        let source_id = source_id.to_owned();
        let _ = service.call(move |backend: &mut ObsBackendRuntime| {
            backend.interact_close(&source_id);
            Ok::<(), BackendError>(())
        });
    }
}
