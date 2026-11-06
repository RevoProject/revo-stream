pub mod mock;

#[cfg(feature = "obs-backend")]
pub mod obs;

use revo_core::service::BackendService;

pub use mock::MockBackendRuntime;
#[cfg(feature = "obs-backend")]
pub use obs::{capture_preview_png, ObsBackendRuntime};
pub use revo_core::service::{BackendCommand, BackendError, BackendRuntime, BackendSnapshot};

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
