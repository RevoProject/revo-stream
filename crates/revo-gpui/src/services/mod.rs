pub mod mock;

#[cfg(feature = "obs-backend")]
pub mod engine;
#[cfg(feature = "obs-backend")]
pub mod obs;

use revo_core::service::BackendService;

#[cfg(feature = "obs-backend")]
pub use engine::EngineClient;
pub use mock::MockBackendRuntime;
#[cfg(feature = "obs-backend")]
pub use obs::{
    capture_preview_frame, capture_source_frame, decode_png_data_uri, interact_close_session,
    interact_dispatch_key, interact_dispatch_mouse, interact_frame_png, interact_reapply,
    open_interaction_session, UI_PREVIEW_INTERVAL_MS,
};
pub use revo_core::service::{BackendCommand, BackendError, BackendRuntime, BackendSnapshot};

/// Uploads one raw RGBA frame as a GPUI texture.
///
/// Engine captures are already RGBA8 row-major, but [`gpui::RenderImage`]
/// frames hold BGRA (the same conversion gpui applies to every decoded
/// bitmap), so the red and blue channels are swapped here once, off the
/// foreground executor. Frames whose dimensions disagree with their payload
/// are rejected rather than uploaded as garbage.
pub fn preview_frame_image(
    frame: revo_core::ipc::PreviewFrame,
) -> Option<std::sync::Arc<gpui::RenderImage>> {
    let width = frame.width;
    let height = frame.height;
    let mut buffer = image::RgbaImage::from_raw(width, height, frame.rgba)?;
    for pixel in buffer.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Some(std::sync::Arc::new(gpui::RenderImage::new(vec![
        image::Frame::new(buffer),
    ])))
}

/// One forwarded mouse interaction for a browser source, mirroring the
/// Svelte `obs_interact_mouse` invoke payload fields exactly.
#[derive(Clone, Debug, PartialEq)]
pub struct InteractMousePayload {
    pub source_id: String,
    /// CDP mouse-event kind: `mousePressed`, `mouseReleased`,
    /// `mouseMoved`, or `mouseWheel`.
    pub kind: &'static str,
    pub x: f64,
    pub y: f64,
    /// CDP button label: `left`, `middle`, `right`, or `none`.
    pub button: &'static str,
    pub buttons: i64,
    pub click_count: i64,
    pub delta_x: f64,
    pub delta_y: f64,
}

/// One forwarded key interaction for a browser source, mirroring the
/// Svelte `obs_interact_key` invoke payload fields exactly.
#[derive(Clone, Debug, PartialEq)]
pub struct InteractKeyPayload {
    pub source_id: String,
    pub kind: &'static str,
    pub key: String,
    pub code: String,
    pub key_code: i64,
    pub text: String,
}

/// Single backend handle for the GPUI application so `app.rs` can hold one
/// type regardless of which runtime is linked in. The `Obs` variant only
/// exists with the `obs-backend` feature and hosts no libobs code in this
/// process — the runtime runs in the separate `revo-engine` process and is
/// reached through [`EngineClient`].
#[derive(Clone)]
#[cfg(feature = "obs-backend")]
pub enum AnyBackend {
    Mock(BackendService<MockBackendRuntime>),
    Obs(BackendService<EngineClient>),
}

#[derive(Clone)]
#[cfg(not(feature = "obs-backend"))]
pub enum AnyBackend {
    Mock(BackendService<MockBackendRuntime>),
}

impl AnyBackend {
    pub fn mock() -> Self {
        Self::Mock(
            BackendService::new(32, MockBackendRuntime::default)
                .expect("failed to start mock backend service"),
        )
    }

    #[cfg(feature = "obs-backend")]
    pub fn obs() -> Self {
        // The engine spawn + Unix-socket connect runs on the backend worker
        // thread (the constructor closure), keeping the GPUI main thread
        // free. A failed spawn panics the worker, which surfaces to every
        // later backend call with its message intact.
        Self::Obs(
            BackendService::new(8, move || {
                EngineClient::new().expect("failed to start revo-engine process")
            })
            .expect("failed to start engine backend service"),
        )
    }

    #[cfg(feature = "obs-backend")]
    pub fn is_obs(&self) -> bool {
        matches!(self, Self::Obs(_))
    }

    pub fn try_dispatch(&self, command: BackendCommand) -> Result<(), BackendError> {
        match self {
            Self::Mock(service) => service.try_dispatch(command),
            #[cfg(feature = "obs-backend")]
            Self::Obs(service) => service.try_dispatch(command),
        }
    }

    pub fn execute(&self, command: BackendCommand) -> Result<(), BackendError> {
        match self {
            Self::Mock(service) => service.execute(command),
            #[cfg(feature = "obs-backend")]
            Self::Obs(service) => service.execute(command),
        }
    }

    pub fn snapshot(&self) -> Result<BackendSnapshot, BackendError> {
        match self {
            Self::Mock(service) => service.snapshot(),
            #[cfg(feature = "obs-backend")]
            Self::Obs(service) => service.snapshot(),
        }
    }

    pub fn shutdown(&self) -> Result<(), BackendError> {
        match self {
            Self::Mock(service) => service.shutdown(),
            #[cfg(feature = "obs-backend")]
            Self::Obs(service) => service.shutdown(),
        }
    }

    /// Blocking call site for tests and background threads only; the runtime
    /// type is bound by the caller's closure signature.
    #[cfg(not(feature = "obs-backend"))]
    pub fn call<T, F>(&self, operation: F) -> Result<T, BackendError>
    where
        T: Send + 'static,
        F: FnOnce(&mut MockBackendRuntime) -> Result<T, BackendError> + Send + 'static,
    {
        match self {
            Self::Mock(service) => service.call(operation),
        }
    }

    #[cfg(feature = "obs-backend")]
    pub fn call_obs<T, F>(&self, operation: F) -> Result<T, BackendError>
    where
        T: Send + 'static,
        F: FnOnce(&mut EngineClient) -> Result<T, BackendError> + Send + 'static,
    {
        match self {
            Self::Mock(_) => Err(BackendError::Runtime(
                "obs runtime call requires the obs backend".into(),
            )),
            Self::Obs(service) => service.call(operation),
        }
    }
}

/// Fetches one source's filter list on whichever backend is available;
/// empty for mock builds.
pub fn fetch_source_filters(
    backend: &AnyBackend,
    id: &str,
) -> Vec<revo_core::models::SourceFilterItem> {
    #[cfg(feature = "obs-backend")]
    if let AnyBackend::Obs(service) = backend {
        return crate::services::obs::source_filters(service, id).unwrap_or_default();
    }
    #[cfg(not(feature = "obs-backend"))]
    {
        let _ = (backend, id);
    }
    Vec::new()
}

/// Fetches one source's full settings on whichever backend is available;
/// the mock runtime serves typed defaults per source type so the native
/// editor renders the same typeable fields as engine builds. Runs on the
/// calling context — callers should background this.
pub fn fetch_source_settings(
    backend: &AnyBackend,
    id: &str,
) -> Option<revo_core::models::SourceSettingsData> {
    #[cfg(feature = "obs-backend")]
    if let AnyBackend::Obs(service) = backend {
        return crate::services::obs::source_settings(service, id).ok();
    }
    #[cfg(feature = "obs-backend")]
    if let AnyBackend::Mock(service) = backend {
        let id = id.to_owned();
        return service
            .call(move |runtime| runtime.source_settings(&id))
            .ok()
            .flatten();
    }
    #[cfg(not(feature = "obs-backend"))]
    {
        let AnyBackend::Mock(service) = backend;
        let id = id.to_owned();
        return service
            .call(move |runtime| runtime.source_settings(&id))
            .ok()
            .flatten();
    }
    None
}

/// Fetches one filter's full settings (typed property specs) on whichever
/// backend is available; `None` for mock builds and when the engine cannot
/// resolve the filter (or build defaults for its kind). Runs on the calling
/// context — callers should background this.
pub fn fetch_filter_settings(
    backend: &AnyBackend,
    source_id: &str,
    filter_name: &str,
    kind: &str,
) -> Option<revo_core::models::SourceSettingsData> {
    #[cfg(feature = "obs-backend")]
    if let AnyBackend::Obs(service) = backend {
        return crate::services::obs::filter_settings(service, source_id, filter_name, kind)
            .ok()
            .flatten();
    }
    #[cfg(not(feature = "obs-backend"))]
    {
        let _ = (backend, source_id, filter_name, kind);
    }
    None
}

/// Static fallback font list for mock builds (no engine to run `fc-list`);
/// real builds enumerate the host through the engine instead.
pub const MOCK_SYSTEM_FONTS: [&str; 14] = [
    "Arial",
    "Courier New",
    "DejaVu Sans",
    "DejaVu Sans Mono",
    "DejaVu Serif",
    "FreeSans",
    "Helvetica",
    "Liberation Mono",
    "Liberation Sans",
    "Liberation Serif",
    "Noto Sans",
    "Space Grotesk",
    "Times New Roman",
    "Verdana",
];

/// Fetches the system font family list on whichever backend is available;
/// the static [`MOCK_SYSTEM_FONTS`] list for mock builds. Runs on the
/// calling context — callers should background this.
pub fn fetch_system_fonts(backend: &AnyBackend) -> Vec<String> {
    #[cfg(feature = "obs-backend")]
    if let AnyBackend::Obs(service) = backend {
        return crate::services::obs::system_fonts(service)
            .map(|items| {
                let mut labels: Vec<String> = items.into_iter().map(|item| item.label).collect();
                labels.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
                labels.dedup();
                labels
            })
            .unwrap_or_default();
    }
    #[cfg(not(feature = "obs-backend"))]
    {
        let _ = backend;
    }
    MOCK_SYSTEM_FONTS
        .iter()
        .map(|font| (*font).to_string())
        .collect()
}

/// Settings keys a "Select device" pick writes for one source type (Svelte
/// `quickDeviceParamKeysFor` parity). The first key is the primary one the
/// picker prefills and displays.
pub fn device_param_keys(source_type: &str) -> &'static [&'static str] {
    let t = source_type.to_ascii_lowercase();
    if t.contains("window") || t.contains("xcomposite") {
        &["window", "capture_window"]
    } else if t.contains("xshm")
        || t.contains("display")
        || t.contains("screen")
        || t.contains("monitor_capture")
    {
        &["device"]
    } else if t.contains("pulse")
        || t.contains("wasapi")
        || t.contains("coreaudio")
        || t.contains("audio_input")
        || t.contains("audio_output")
        || t.contains("audio_capture")
        || t.contains("audio_line")
    {
        &["device", "device_id"]
    } else {
        &["device_id", "device"]
    }
}

/// Enumerates selectable devices for one capture source: the backend device
/// catalog first (engine-side pulse/video/window listers, static mock lists
/// for mock builds), falling back to the source's own OBS property options
/// when the catalog is empty (Svelte `obs_get_source_settings` parity).
/// Runs on the calling context — callers should background this.
pub fn fetch_source_devices(
    backend: &AnyBackend,
    source_type: &str,
    id: &str,
) -> Vec<(String, String)> {
    let items: Vec<revo_core::models::SourceTypeItem> = {
        #[cfg(feature = "obs-backend")]
        if let AnyBackend::Obs(service) = backend {
            crate::services::obs::source_devices(service, source_type).unwrap_or_default()
        } else if let AnyBackend::Mock(service) = backend {
            let st = source_type.to_owned();
            service
                .call(move |runtime| runtime.source_devices(&st))
                .unwrap_or_default()
        } else {
            Vec::new()
        }
        #[cfg(not(feature = "obs-backend"))]
        {
            let st = source_type.to_owned();
            let AnyBackend::Mock(service) = backend;
            service
                .call(move |runtime| runtime.source_devices(&st))
                .unwrap_or_default()
        }
    };
    if !items.is_empty() {
        return items
            .into_iter()
            .map(|item| (item.id, item.label))
            .collect();
    }
    // Property-list fallback: a select-typed source property whose key looks
    // like a device chooser and carries options.
    fetch_source_settings(backend, id)
        .map(|data| {
            data.source_properties
                .iter()
                .find(|spec| {
                    let key = spec.key.to_ascii_lowercase();
                    !spec.options.is_empty()
                        && (key.contains("device")
                            || key.contains("window")
                            || key.contains("input")
                            || key.contains("output")
                            || key.contains("display")
                            || key.contains("screen"))
                })
                .map(|spec| {
                    spec.options
                        .iter()
                        .map(|option| (option.value.clone(), option.label.clone()))
                        .collect()
                })
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_param_keys_mirror_the_svelte_source_type_mapping() {
        assert_eq!(
            device_param_keys("v4l2_input"),
            &["device_id", "device"],
            "video capture prefers device_id"
        );
        assert_eq!(
            device_param_keys("pulse_input_capture"),
            &["device", "device_id"],
            "audio devices prefer the pulse device name"
        );
        assert_eq!(
            device_param_keys("xcomposite_input"),
            &["window", "capture_window"]
        );
        assert_eq!(device_param_keys("monitor_capture"), &["device"]);
    }
}
