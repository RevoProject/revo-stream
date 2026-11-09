//! GPUI-side helpers for the real libobs backend, gated behind the
//! `obs-backend` feature so default builds (and `scripts/core/verify.sh`)
//! stay green without libobs/openssl host packages.
//!
//! The libobs runtime lives in the separate `revo-engine` process
//! (`revo_backend::engine` server + `revo_lib` bindings); this module keeps
//! only the typed call helpers the workspace and auxiliary windows use,
//! routing each one through [`crate::services::EngineClient`] on the
//! backend worker thread.

use revo_core::ipc::DebugLogEntry;
use revo_core::service::{BackendError, BackendService};

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
/// Fetches the engine process's libobs debug-log history over IPC so the
/// debug console shows revo-lib entries (the registry lives in the engine,
/// not in the UI process). Runs on the backend worker thread.
pub fn debug_log_history(
    service: &BackendService<EngineClient>,
) -> Result<Vec<DebugLogEntry>, BackendError> {
    service.call(|client: &mut EngineClient| client.debug_log_history())
}

/// Fetches one source's full settings over IPC (params + typed property
/// specs) for the source editor and the undo history of settings edits.
/// Runs on the backend worker thread.
pub fn source_settings(
    service: &BackendService<EngineClient>,
    id: &str,
) -> Result<revo_core::models::SourceSettingsData, BackendError> {
    let id = id.to_owned();
    service.call(move |client: &mut EngineClient| client.source_settings(&id))
}

/// Captures one source-or-scene PNG frame over IPC and decodes it into
/// raw PNG bytes; scene thumbnails use this slow path (worker thread).
pub fn capture_source_png(
    backend: &super::AnyBackend,
    source_name: &str,
) -> Result<Vec<u8>, BackendError> {
    let source_name = source_name.to_owned();
    let data_uri = capture_preview_png(backend, Some(&source_name))?;
    crate::services::decode_png_data_uri(&data_uri)
        .ok_or_else(|| BackendError::Runtime("failed to decode thumbnail PNG".into()))
}

/// Clears the engine process's libobs debug-log registry over IPC.
/// Fetches one source's filter list over IPC for the editor's Filters tab.
pub fn source_filters(
    service: &BackendService<EngineClient>,
    id: &str,
) -> Result<Vec<revo_core::models::SourceFilterItem>, BackendError> {
    let id = id.to_owned();
    service.call(move |client: &mut EngineClient| client.source_filters(&id))
}

/// Fetches one filter's full settings over IPC (typed property specs from
/// the filter's libobs property API) for the editor's per-filter settings.
/// Runs on the backend worker thread.
pub fn filter_settings(
    service: &BackendService<EngineClient>,
    source_id: &str,
    filter_name: &str,
    kind: &str,
) -> Result<Option<revo_core::models::SourceSettingsData>, BackendError> {
    let (source_id, filter_name, kind) = (
        source_id.to_owned(),
        filter_name.to_owned(),
        kind.to_owned(),
    );
    service.call(move |client: &mut EngineClient| {
        client.filter_settings(&source_id, &filter_name, &kind)
    })
}

/// Enumerates the engine's system font families over IPC (engine-side
/// `fc-list`). Runs on the backend worker thread.
pub fn system_fonts(
    service: &BackendService<EngineClient>,
) -> Result<Vec<revo_core::models::SourceTypeItem>, BackendError> {
    service.call(|client: &mut EngineClient| client.system_fonts())
}

/// Enumerates the engine's device catalog for one source type over IPC
/// (engine-side pulse/video/window listers). Runs on the backend worker
/// thread.
pub fn source_devices(
    service: &BackendService<EngineClient>,
    source_type: &str,
) -> Result<Vec<revo_core::models::SourceTypeItem>, BackendError> {
    let source_type = source_type.to_owned();
    service.call(move |client: &mut EngineClient| client.source_devices(&source_type))
}

pub fn clear_debug_logs(service: &BackendService<EngineClient>) -> Result<(), BackendError> {
    service.call(|client: &mut EngineClient| client.debug_log_clear())
}

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
