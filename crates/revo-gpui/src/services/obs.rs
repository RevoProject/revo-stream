//! GPUI-side helpers for the real libobs backend, gated behind the
//! `obs-backend` feature so default builds (and `scripts/core/verify.sh`)
//! stay green without libobs/openssl host packages.
//!
//! The libobs runtime lives in the separate `revo-engine` process
//! (`revo_backend::engine` server + `revo_lib` bindings); this module keeps
//! only the typed call helpers the workspace and auxiliary windows use,
//! routing each one through [`crate::services::EngineClient`] on the
//! backend worker thread.

use revo_core::service::BackendError;

pub use crate::services::engine::{EngineClient, UI_PREVIEW_INTERVAL_MS};

/// Captures a preview frame as a PNG data-URI string.
///
/// Without a `source_id` the current scene composite is captured (the
/// workspace preview loop); with one, the matching source is captured
/// individually (the preview auxiliary window). Runs through
/// [`revo_core::service::BackendService::call`] on the backend worker
/// thread: the engine round-trip blocks only the worker, never the GPUI
/// foreground executor.
pub fn capture_preview_png(
    backend: &super::AnyBackend,
    source_id: Option<&str>,
) -> Result<String, BackendError> {
    let source_id = source_id.map(str::to_owned);
    match backend {
        super::AnyBackend::Obs(service) => service.call(move |client: &mut EngineClient| {
            client.capture_preview_png(source_id.as_deref())
        }),
        super::AnyBackend::Mock(_) => Err(BackendError::Runtime(
            "preview capture requires the obs backend".into(),
        )),
    }
}

/// Decodes a `data:image/png;base64,...` frame into raw PNG bytes so it can
/// be handed to GPUI as an in-memory image without touching the filesystem.
pub fn decode_png_data_uri(data_uri: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    const PREFIX: &str = "data:image/png;base64,";
    let payload = data_uri.strip_prefix(PREFIX)?;
    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .ok()
}

/// Resolves the browser source's interaction settings and opens a persistent
/// headless-Chromium CDP session for it (engine-side; this call blocks the
/// backend worker until the session is up).
pub fn open_interaction_session(
    backend: &super::AnyBackend,
    source_id: &str,
) -> Result<(), BackendError> {
    let source_id = source_id.to_owned();
    match backend {
        super::AnyBackend::Obs(service) => {
            service.call(move |client: &mut EngineClient| client.interact_open(&source_id))
        }
        super::AnyBackend::Mock(_) => Err(BackendError::Runtime(
            "interaction sessions require the obs backend".into(),
        )),
    }
}

/// Fetches the newest browser-source frame as a data-URI string from the
/// source's CDP session (backend worker thread; blocks on the engine's
/// screencast wait, never on the GPUI foreground executor).
pub fn interact_frame_png(
    backend: &super::AnyBackend,
    source_id: &str,
) -> Result<String, BackendError> {
    let source_id = source_id.to_owned();
    match backend {
        super::AnyBackend::Obs(service) => service.call(move |client: &mut EngineClient| {
            client
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
pub fn interact_dispatch_mouse(
    backend: &super::AnyBackend,
    payload: super::InteractMousePayload,
) -> Result<(), BackendError> {
    match backend {
        super::AnyBackend::Obs(service) => service.call(move |client: &mut EngineClient| {
            client.interact_mouse(
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
pub fn interact_dispatch_key(
    backend: &super::AnyBackend,
    payload: super::InteractKeyPayload,
) -> Result<(), BackendError> {
    match backend {
        super::AnyBackend::Obs(service) => service.call(move |client: &mut EngineClient| {
            client.interact_key(
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
pub fn interact_reapply(backend: &super::AnyBackend, source_id: &str) -> Result<(), BackendError> {
    let source_id = source_id.to_owned();
    match backend {
        super::AnyBackend::Obs(service) => {
            service.call(move |client: &mut EngineClient| client.interact_reapply(&source_id))
        }
        super::AnyBackend::Mock(_) => Err(BackendError::Runtime(
            "interaction sessions require the obs backend".into(),
        )),
    }
}

/// Closes the source's CDP session and its headless browser (Svelte
/// `obs_interact_close`). Fire-and-forget: errors cannot reach the UI once
/// the window is gone.
pub fn interact_close_session(backend: &super::AnyBackend, source_id: &str) {
    if let super::AnyBackend::Obs(service) = backend {
        let source_id = source_id.to_owned();
        let _ = service.call(move |client: &mut EngineClient| {
            client.interact_close(&source_id);
            Ok::<(), BackendError>(())
        });
    }
}
