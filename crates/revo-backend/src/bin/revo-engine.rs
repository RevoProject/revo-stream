//! Native RevoStream engine process.
//!
//! Owns the real libobs runtime (through `revo_backend::obs_service`) in a
//! dedicated OS process and serves exactly one UI client over a local Unix
//! socket using the `revo_core::ipc` protocol. Keeping libobs OpenGL out of
//! the UI process is what lets the GPUI Vulkan renderer present without the
//! driver-level conflict observed on legacy NVIDIA stacks.
//!
//! Usage: `revo-engine --socket /path/to/engine.sock`
//!
//! Lifecycle: the process exits when the connected client disconnects or
//! sends `UiToEngine::Shutdown`.

fn main() {
    let mut socket_path: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--socket" => {
                socket_path = args.next();
            }
            other => {
                eprintln!("usage: revo-engine --socket <path> (unknown argument: {other})");
                std::process::exit(2);
            }
        }
    }
    let socket_path = socket_path.unwrap_or_else(|| {
        std::env::temp_dir()
            .join(format!("revo-engine-{}.sock", std::process::id()))
            .to_string_lossy()
            .into_owned()
    });

    // A stale socket file from a crashed engine would block bind(); remove it.
    let _ = std::fs::remove_file(&socket_path);
    let listener = match std::os::unix::net::UnixListener::bind(&socket_path) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("[engine] failed to bind {}: {error}", socket_path);
            std::process::exit(1);
        }
    };
    eprintln!("[engine] listening on {socket_path}");

    let (mut stream, _peer) = match listener.accept() {
        Ok(connection) => connection,
        Err(error) => {
            eprintln!("[engine] accept failed: {error}");
            std::process::exit(1);
        }
    };
    let _ = std::fs::remove_file(&socket_path); // one client per engine lifetime
    eprintln!("[engine] client connected");

    let mut backend = revo_backend::obs_service::ObsBackendRuntime::new(
        None,
        Box::new(revo_backend::obs_service::NativeObsPlatform),
    );
    revo_backend::engine::serve_connection(&mut stream, &mut backend);
    eprintln!("[engine] client disconnected; exiting");
}
