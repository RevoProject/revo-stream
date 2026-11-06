pub mod mock;

#[cfg(feature = "obs-backend")]
pub mod obs;

use revo_core::service::BackendService;

pub use mock::MockBackendRuntime;
#[cfg(feature = "obs-backend")]
pub use obs::{
    capture_preview_png, decode_png_data_uri, interact_close_session, interact_dispatch_key,
    interact_dispatch_mouse, interact_frame_png, interact_reapply, open_interaction_session,
    ObsBackendRuntime,
};
pub use revo_core::service::{BackendCommand, BackendError, BackendRuntime, BackendSnapshot};

/// One forwarded mouse interaction for a browser source, mirroring the
/// Svelte `obs_interact_mouse` invoke payload fields exactly.
#[derive(Clone, Debug, PartialEq)]
pub struct InteractMousePayload {
    pub source_id: String,
    /// CDP mouse-event kind: `mousePressed`, `mouseReleased`, `mouseMoved`,
    /// or `mouseWheel`.
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

/// One forwarded key interaction for a browser source, mirroring the Svelte
/// `obs_interact_key` invoke payload fields exactly.
#[derive(Clone, Debug, PartialEq)]
pub struct InteractKeyPayload {
    pub source_id: String,
    /// CDP key-event kind: `keyDown`, `char`, or `keyUp`.
    pub kind: &'static str,
    pub key: String,
    pub code: String,
    pub key_code: i64,
    pub text: String,
}

/// Single backend handle for the GPUI application so `app.rs` can hold one
/// type regardless of which runtime is linked in. The `Obs` variant only
/// exists with the `obs-backend` feature.
#[derive(Clone)]
#[cfg(feature = "obs-backend")]
pub enum AnyBackend {
    Mock(BackendService<MockBackendRuntime>),
    Obs(BackendService<ObsBackendRuntime>),
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
    pub fn obs(root_dir: Option<String>) -> Self {
        Self::Obs(
            BackendService::new(8, move || ObsBackendRuntime::new(root_dir.clone()))
                .expect("failed to start obs backend service"),
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
        F: FnOnce(&mut ObsBackendRuntime) -> Result<T, BackendError> + Send + 'static,
    {
        match self {
            Self::Mock(_) => Err(BackendError::Runtime(
                "obs runtime call requires the obs backend".into(),
            )),
            Self::Obs(service) => service.call(operation),
        }
    }
}
