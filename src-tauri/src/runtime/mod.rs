pub use revo_backend::runtime::lifecycle::{version, ObsPlatform};

// Only the framework-neutral lifecycle entry points are part of the adapter
// surface; the remaining `revo-backend` runtime helpers stay crate-internal.
pub use revo_backend::runtime::init::{shutdown_with_platform, start_with_platform};

pub use revo_backend::runtime::{encoders, helpers, preview, recording, streaming};

pub mod init;
pub mod lifecycle;
