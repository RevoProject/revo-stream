//! Native RevoStream engine process.
//!
//! Owns the real libobs runtime (through `revo_backend::obs_service`) in a
//! dedicated OS process and serves exactly one UI client over the platform
//! local transport (`revo_core::transport`): a Unix socket on Unix, a
//! loopback TCP socket on Windows. Keeping libobs OpenGL out of the UI
//! process is what lets the GPUI Vulkan renderer present without the
//! driver-level conflict observed on legacy NVIDIA stacks.
//!
//! Usage:
//!   revo-engine [--socket <path>] [--endpoint-file <path>]
//!
//! `--socket` pins a Unix socket path (legacy callers and `REVO_ENGINE_SOCK`).
//! With no `--socket` the process binds its platform default endpoint and,
//! when `--endpoint-file` is given, publishes the bound endpoint there so the
//! spawning UI can read it — the ephemeral TCP port is unknowable until bind.
//!
//! Lifecycle: the process exits when the connected client disconnects or
//! sends `UiToEngine::Shutdown`.

use std::path::{Path, PathBuf};

use revo_core::transport::{write_endpoint_file, EngineEndpoint, EngineListener};

fn parse_arguments(arguments: impl Iterator<Item = String>) -> (Option<String>, Option<PathBuf>) {
    let mut socket: Option<String> = None;
    let mut endpoint_file: Option<PathBuf> = None;
    let mut arguments = arguments.peekable();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--socket" => socket = arguments.next(),
            "--endpoint-file" => {
                endpoint_file = arguments.next().map(PathBuf::from);
            }
            other => {
                eprintln!("usage: revo-engine [--socket <path>] [--endpoint-file <path>] (unknown argument: {other})");
                std::process::exit(2);
            }
        }
    }
    (socket, endpoint_file)
}

fn main() {
    let (socket, endpoint_file) = parse_arguments(std::env::args().skip(1));

    // A stale socket file from a crashed engine would block bind(); the
    // listener removes it before binding, the same way the UI used to.
    // `--socket` accepts the same endpoint forms as REVO_ENGINE_SOCK so a
    // pinned address works on every platform.
    let endpoint = match socket {
        Some(value) => match EngineEndpoint::parse(&value) {
            Ok(endpoint) => endpoint,
            Err(error) => {
                eprintln!("[engine] invalid --socket value {value:?}: {error}");
                std::process::exit(2);
            }
        },
        None => EngineEndpoint::auto(),
    };

    let listener = match EngineListener::bind(&endpoint) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("[engine] failed to bind {}: {error}", endpoint.to_string());
            std::process::exit(1);
        }
    };

    // The bound endpoint, not the requested one: an ephemeral TCP port only
    // exists after bind.
    let bound = listener.endpoint();
    eprintln!("[engine] listening on {}", bound.to_string());
    if let Some(file) = &endpoint_file {
        if let Err(error) = publish_endpoint(file, &bound) {
            eprintln!("[engine] failed to publish {file:?}: {error}");
            std::process::exit(1);
        }
    }

    let mut stream = match listener.accept() {
        Ok(stream) => stream,
        Err(error) => {
            eprintln!("[engine] accept failed: {error}");
            std::process::exit(1);
        }
    };
    listener.release_endpoint_file(); // one client per engine lifetime
    if let Some(file) = &endpoint_file {
        let _ = std::fs::remove_file(file);
    }
    eprintln!("[engine] client connected");

    // A packaged release build refuses to write settings or logs until the
    // GUI adapter has supplied paths; the engine has no adapter, so it derives
    // them from the platform standards (REVO_DATA_DIR overrides).
    if let Err(error) = revo_backend::utils::fs::init_standalone_app_paths() {
        eprintln!("[engine] app paths unavailable: {error}");
    }

    let mut backend = revo_backend::obs_service::ObsBackendRuntime::new(
        None,
        Box::new(revo_backend::obs_service::NativeObsPlatform),
    );
    revo_backend::engine::serve_connection(&mut stream, &mut backend);
    eprintln!("[engine] client disconnected; exiting");
}

fn publish_endpoint(file: &Path, endpoint: &EngineEndpoint) -> std::io::Result<()> {
    write_endpoint_file(file, endpoint)
}
