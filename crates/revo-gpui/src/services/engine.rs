//! IPC client for the native engine process.
//!
//! The UI process owns no libobs code on this path: [`EngineClient`] spawns
//! (or connects to) the `revo-engine` binary and speaks
//! `revo_core::ipc` over a local Unix socket. Preview frames stream from the
//! engine on its own cadence into a bounded newest-wins slot; commands and
//! correlated requests (preview captures, browser interaction) round-trip
//! synchronously on the backend worker thread, never blocking the GPUI
//! foreground executor.
//!
//! Binary resolution order:
//! 1. `REVO_ENGINE_BIN` — explicit path,
//! 2. a `revo-engine` sibling of the running UI binary (cargo builds every
//!    package binary into the same target directory),
//! 3. `../revo-backend/target/{debug,release}/revo-engine` next to this
//!    crate's manifest.
//!
//! An externally started engine is also supported through `REVO_ENGINE_SOCK`
//! (connect without spawning; the engine still exits on `Shutdown`).

use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use revo_core::ipc::{decode_frame, encode_frame, DebugLogEntry, EngineToUi, UiToEngine};
use revo_core::service::{BackendCommand, BackendError, BackendRuntime, BackendSnapshot};

/// UI-side preview poll cadence; the engine also pushes frames between
/// requests so this only bounds the loop, it does not drive the capture.
pub const UI_PREVIEW_INTERVAL_MS: u64 = 150;

/// How long a request waits for its reply before declaring the engine lost.
const REPLY_TIMEOUT: Duration = Duration::from_secs(10);
/// Extra drain window after a reply arrives, so stale pushes (a snapshot
/// pushed right after this client's own reply) are consumed instead of
/// leaking into the next request's read.
const SETTLE_WINDOW: Duration = Duration::from_millis(30);
/// Connect window when spawning the engine process.
const SPAWN_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// What the read loop is waiting for.
#[derive(Clone, Copy, PartialEq)]
enum DrainTarget {
    /// The next [`EngineToUi::Snapshot`] (command settle or snapshot reply).
    Snapshot,
    /// A correlated reply carrying this request id (or, for
    /// [`DrainTarget::SceneFrame`], any pushed scene frame).
    Reply(u64),
    /// A scene-composite preview frame: correlated reply or engine push.
    SceneFrame,
}

impl DrainTarget {
    fn matches_id(self, id: u64) -> bool {
        match self {
            Self::Reply(expected) => expected == id,
            _ => false,
        }
    }
}

/// What one drain collected for its caller.
#[derive(Default)]
struct Inbox {
    snapshot: Option<BackendSnapshot>,
    reply_data_uri: Option<String>,
    reply_log_entries: Option<Vec<DebugLogEntry>>,
    reply_source_settings: Option<SourceSettingsData>,
    reply_error: Option<String>,
    /// A correlated reply arrived — `true` even when it carried no frame
    /// (source offline), so the drain does not wait for data that will
    /// never come.
    reply_seen: bool,
    uncorrelated_error: Option<String>,
}

impl Inbox {
    fn satisfied(&self, target: DrainTarget) -> bool {
        match target {
            DrainTarget::Snapshot => self.snapshot.is_some(),
            DrainTarget::Reply(_) | DrainTarget::SceneFrame => {
                self.reply_seen || self.reply_error.is_some()
            }
        }
    }
}

/// Shared engine connection state. The backend worker thread performs all
/// blocking I/O through the mutex-guarded stream; slots are shared so the
/// foreground can drain the newest preview frame without touching I/O.
struct Connection {
    stream: Mutex<Option<UnixStream>>,
    child: Mutex<Option<Child>>,
    next_request_id: AtomicU64,
    /// Newest engine-pushed scene frame, newest-wins.
    preview_frame: Mutex<Option<String>>,
}

impl Connection {
    fn next_request_id(&self) -> u64 {
        self.next_request_id.fetch_add(1, Ordering::Relaxed)
    }

    fn take_preview_frame(&self) -> Option<String> {
        self.preview_frame
            .lock()
            .expect("engine preview slot mutex poisoned")
            .take()
    }

    fn send_request(&self, request: &UiToEngine) -> Result<(), BackendError> {
        let frame = encode_frame(request).map_err(|error| {
            BackendError::Runtime(format!("engine frame encode failed: {error}"))
        })?;
        let mut guard = self.stream.lock().expect("engine stream mutex poisoned");
        let outcome = match guard.as_mut() {
            Some(stream) => stream
                .write_all(&frame)
                .and_then(|()| stream.flush())
                .map(|()| ()),
            None => Err(std::io::Error::new(
                ErrorKind::NotConnected,
                "engine stream closed",
            )),
        };
        match outcome {
            Ok(()) => Ok(()),
            Err(_) => {
                *guard = None;
                Err(BackendError::Disconnected)
            }
        }
    }

    /// Reads one full frame; `Ok(None)` when the read window elapsed. Marks
    /// the connection dead on EOF so every later request reports
    /// [`BackendError::Disconnected`].
    fn read_message(&self, timeout: Duration) -> std::io::Result<Option<EngineToUi>> {
        let mut guard = self.stream.lock().expect("engine stream mutex poisoned");
        let Some(stream) = guard.as_mut() else {
            return Err(std::io::Error::new(
                ErrorKind::NotConnected,
                "engine stream closed",
            ));
        };
        stream.set_read_timeout(Some(timeout))?;
        let mut header = [0u8; 4];
        match stream.read_exact(&mut header) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                ) =>
            {
                return Ok(None)
            }
            Err(_) => {
                *guard = None;
                return Err(std::io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "engine stream closed",
                ));
            }
        }
        let payload_len = u32::from_le_bytes(header) as usize;
        let mut payload = vec![0u8; payload_len];
        if stream.read_exact(&mut payload).is_err() {
            *guard = None;
            return Err(std::io::Error::new(
                ErrorKind::UnexpectedEof,
                "engine frame truncated",
            ));
        }
        let mut framed = header.to_vec();
        framed.extend_from_slice(&payload);
        match decode_frame::<EngineToUi>(&framed) {
            Ok((message, _)) => Ok(Some(message)),
            Err(_) => {
                *guard = None;
                Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "undecodable engine message",
                ))
            }
        }
    }

    /// Reads until `target` is satisfied, absorbing everything else into
    /// bounded slots. After the first satisfying message the loop keeps
    /// draining for [`SETTLE_WINDOW`] so the newest state wins; a peer EOF
    /// inside that window only ends the drain, it does not fail it.
    fn drain(&self, target: DrainTarget) -> Result<Inbox, BackendError> {
        let mut inbox = Inbox::default();
        let mut satisfied_at: Option<Instant> = None;
        let deadline = Instant::now() + REPLY_TIMEOUT;
        loop {
            let settled = satisfied_at
                .map(|at| at.elapsed() >= SETTLE_WINDOW)
                .unwrap_or(false);
            if settled {
                return Ok(inbox);
            }
            if Instant::now() > deadline {
                return Err(BackendError::Disconnected);
            }
            let timeout = if satisfied_at.is_some() {
                SETTLE_WINDOW
            } else {
                REPLY_TIMEOUT.min(deadline.saturating_duration_since(Instant::now()))
            };
            match self.read_message(timeout) {
                Ok(Some(message)) => {
                    self.absorb(message, target, &mut inbox);
                    if satisfied_at.is_none() && inbox.satisfied(target) {
                        satisfied_at = Some(Instant::now());
                    }
                }
                // Read window elapsed with nothing pending.
                Ok(None) if satisfied_at.is_some() => return Ok(inbox),
                Ok(None) => {}
                // Stream closed. Anything already collected still counts.
                Err(_) if satisfied_at.is_some() => return Ok(inbox),
                Err(_) => return Err(BackendError::Disconnected),
            }
        }
    }

    fn absorb(&self, message: EngineToUi, target: DrainTarget, inbox: &mut Inbox) {
        match message {
            EngineToUi::Snapshot(snapshot) => inbox.snapshot = Some(snapshot),
            EngineToUi::PreviewFrame { data_uri } => {
                if target == DrainTarget::SceneFrame {
                    // This call is actively asking for a scene frame;
                    // consume the push directly instead of parking it.
                    inbox.reply_seen = true;
                    inbox.reply_data_uri = Some(data_uri);
                } else {
                    // Newest-wins slot for a later capture call.
                    *self
                        .preview_frame
                        .lock()
                        .expect("engine preview slot mutex poisoned") = Some(data_uri);
                }
            }
            EngineToUi::PreviewFrameData {
                request_id,
                data_uri,
            } => {
                if target.matches_id(request_id) || target == DrainTarget::SceneFrame {
                    inbox.reply_seen = true;
                    inbox.reply_data_uri = data_uri;
                }
            }
            EngineToUi::InteractReply {
                request_id,
                data_uri,
            } => {
                if target.matches_id(request_id) {
                    inbox.reply_seen = true;
                    inbox.reply_data_uri = data_uri;
                }
            }
            EngineToUi::DebugLogHistoryData {
                request_id,
                entries,
            } => {
                if target.matches_id(request_id) {
                    inbox.reply_seen = true;
                    inbox.reply_log_entries = Some(entries);
                }
            }
            EngineToUi::DebugLogCleared { request_id } => {
                if target.matches_id(request_id) {
                    inbox.reply_seen = true;
                }
            }
            EngineToUi::SourceSettingsData {
                request_id,
                settings,
            } => {
                if target.matches_id(request_id) {
                    inbox.reply_seen = true;
                    inbox.reply_source_settings = settings;
                }
            }
            EngineToUi::Status(_) => {}
            EngineToUi::Error {
                request_id,
                message,
            } => {
                let correlated = match request_id {
                    Some(id) => target.matches_id(id),
                    None => false,
                };
                if correlated {
                    inbox.reply_error = Some(message);
                } else {
                    inbox.uncorrelated_error = Some(message);
                }
            }
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // Best-effort orderly exit; the engine also exits on client EOF.
        if self.send_request(&UiToEngine::Shutdown).is_ok() {
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                match self.read_message(Duration::from_millis(200)) {
                    Ok(Some(_)) => continue,
                    Ok(None) => continue,
                    Err(_) => break,
                }
            }
        }
        if let Some(child) = self
            .child
            .lock()
            .expect("engine child mutex poisoned")
            .as_mut()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// UI-side handle to the engine process. Implements
/// [`revo_core::service::BackendRuntime`] so it slots straight into
/// [`revo_core::service::BackendService`] with the same worker-thread
/// semantics the in-process libobs runtime had.
#[derive(Clone)]
pub struct EngineClient {
    connection: Arc<Connection>,
}

impl EngineClient {
    /// Connects (or spawns) the engine process.
    pub fn new() -> Result<Self, BackendError> {
        if let Ok(path) = std::env::var("REVO_ENGINE_SOCK") {
            if let Ok(stream) = UnixStream::connect(&path) {
                return Ok(Self::over_stream(stream, None));
            }
        }
        let binary = engine_binary_path().ok_or_else(|| {
            BackendError::Runtime(
                "engine binary not found; build it with `cargo build --manifest-path \
                 crates/revo-backend/Cargo.toml --bin revo-engine`"
                    .into(),
            )
        })?;
        let socket =
            std::env::temp_dir().join(format!("revo-engine-ui-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&socket);
        let mut child = Command::new(&binary)
            .arg("--socket")
            .arg(&socket)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| BackendError::Runtime(format!("engine spawn failed: {error}")))?;
        let deadline = Instant::now() + SPAWN_CONNECT_TIMEOUT;
        while Instant::now() < deadline {
            if let Ok(Some(_)) = child.try_wait() {
                return Err(BackendError::Runtime(
                    "engine process exited before accepting a connection".into(),
                ));
            }
            match UnixStream::connect(&socket) {
                Ok(stream) => {
                    return Ok(Self::over_stream(stream, Some(child)));
                }
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        Err(BackendError::Runtime(
            "engine socket never became ready".into(),
        ))
    }

    fn over_stream(stream: UnixStream, child: Option<Child>) -> Self {
        Self {
            connection: Arc::new(Connection {
                stream: Mutex::new(Some(stream)),
                child: Mutex::new(child),
                next_request_id: AtomicU64::new(1),
                preview_frame: Mutex::new(None),
            }),
        }
    }

    /// Captures one preview frame as a data URI. Without a `source_id` the
    /// newest engine-pushed scene frame is drained first (bounded, newest
    /// wins); only an empty slot issues a correlated capture request.
    pub fn capture_preview_png(&mut self, source_id: Option<&str>) -> Result<String, BackendError> {
        if source_id.is_none() {
            if let Some(data_uri) = self.connection.take_preview_frame() {
                return Ok(data_uri);
            }
        }
        let request_id = self.connection.next_request_id();
        self.connection
            .send_request(&UiToEngine::PreviewFrameOnce {
                request_id,
                source_id: source_id.map(str::to_owned),
            })?;
        let target = if source_id.is_none() {
            DrainTarget::SceneFrame
        } else {
            DrainTarget::Reply(request_id)
        };
        let inbox = self.connection.drain(target)?;
        if let Some(error) = inbox.reply_error.or(inbox.uncorrelated_error) {
            return Err(BackendError::Runtime(error));
        }
        inbox
            .reply_data_uri
            .ok_or_else(|| BackendError::Runtime("no frame captured".into()))
    }

    /// Opens the browser-interaction CDP session for one source.
    pub fn interact_open(&mut self, source_id: &str) -> Result<(), BackendError> {
        self.interact_fire(source_id, |request_id, source_id| {
            UiToEngine::InteractOpen {
                request_id,
                source_id: source_id.to_owned(),
            }
        })
    }

    /// Fetches the newest browser-source frame from its CDP session.
    pub fn interact_frame(&mut self, source_id: &str) -> Result<Option<String>, BackendError> {
        let request_id = self.connection.next_request_id();
        self.connection.send_request(&UiToEngine::InteractFrame {
            request_id,
            source_id: source_id.to_owned(),
        })?;
        let inbox = self.connection.drain(DrainTarget::Reply(request_id))?;
        if let Some(error) = inbox.reply_error {
            return Err(BackendError::Runtime(error));
        }
        Ok(inbox.reply_data_uri)
    }

    /// Forwards one typed mouse interaction (Svelte payload fields).
    pub fn interact_mouse(
        &mut self,
        source_id: &str,
        kind: &str,
        x: f64,
        y: f64,
        button: &str,
        buttons: i64,
        click_count: i64,
        delta_x: f64,
        delta_y: f64,
    ) -> Result<(), BackendError> {
        let (kind, button) = (kind.to_owned(), button.to_owned());
        self.interact_fire(source_id, move |request_id, source_id| {
            UiToEngine::InteractMouse {
                request_id,
                source_id: source_id.to_owned(),
                kind,
                x,
                y,
                button,
                buttons,
                click_count,
                delta_x,
                delta_y,
            }
        })
    }

    /// Forwards one typed key interaction (Svelte payload fields).
    pub fn interact_key(
        &mut self,
        source_id: &str,
        kind: &str,
        key: &str,
        code: &str,
        key_code: i64,
        text: &str,
    ) -> Result<(), BackendError> {
        let (kind, key, code, text) = (
            kind.to_owned(),
            key.to_owned(),
            code.to_owned(),
            text.to_owned(),
        );
        self.interact_fire(source_id, move |request_id, source_id| {
            UiToEngine::InteractKey {
                request_id,
                source_id: source_id.to_owned(),
                kind,
                key,
                code,
                key_code,
                text,
            }
        })
    }

    /// Re-runs the session's custom CSS/JS injection.
    pub fn interact_reapply(&mut self, source_id: &str) -> Result<(), BackendError> {
        self.interact_fire(source_id, |request_id, source_id| {
            UiToEngine::InteractReapply {
                request_id,
                source_id: source_id.to_owned(),
            }
        })
    }

    /// Closes the source's CDP session and its headless browser.
    /// Fire-and-forget: errors cannot reach the UI once the window is gone.
    pub fn interact_close(&mut self, source_id: &str) {
        let _ = self.connection.send_request(&UiToEngine::InteractClose {
            source_id: source_id.to_owned(),
        });
    }

    /// Fetches the engine process's libobs debug-log history so the debug
    /// console shows revo-lib entries even though the registry lives in the
    /// engine process.
    pub fn debug_log_history(&mut self) -> Result<Vec<DebugLogEntry>, BackendError> {
        let request_id = self.connection.next_request_id();
        self.connection
            .send_request(&UiToEngine::DebugLogHistory { request_id })?;
        let inbox = self.connection.drain(DrainTarget::Reply(request_id))?;
        if let Some(error) = inbox.reply_error {
            return Err(BackendError::Runtime(error));
        }
        Ok(inbox.reply_log_entries.unwrap_or_default())
    }

    /// Fetches one source's full settings (params + typed property specs)
    /// over IPC; `None` when the engine cannot resolve the source.
    pub fn source_settings(
        &mut self,
        id: &str,
    ) -> Result<SourceSettingsData, BackendError> {
        let request_id = self.connection.next_request_id();
        self.connection
            .send_request(&UiToEngine::SourceSettings {
                request_id,
                id: id.to_owned(),
            })?;
        let inbox = self.connection.drain(DrainTarget::Reply(request_id))?;
        if let Some(error) = inbox.reply_error {
            return Err(BackendError::Runtime(error));
        }
        inbox
            .reply_source_settings
            .ok_or_else(|| BackendError::Runtime("source settings unavailable".into()))
    }

    /// Clears the engine process's libobs debug-log registry.
    pub fn debug_log_clear(&mut self) -> Result<(), BackendError> {
        let request_id = self.connection.next_request_id();
        self.connection
            .send_request(&UiToEngine::DebugLogClear { request_id })?;
        let inbox = self.connection.drain(DrainTarget::Reply(request_id))?;
        if let Some(error) = inbox.reply_error {
            return Err(BackendError::Runtime(error));
        }
        Ok(())
    }

    fn interact_fire(
        &mut self,
        source_id: &str,
        build: impl FnOnce(u64, &str) -> UiToEngine,
    ) -> Result<(), BackendError> {
        let request_id = self.connection.next_request_id();
        self.connection
            .send_request(&build(request_id, source_id))?;
        let inbox = self.connection.drain(DrainTarget::Reply(request_id))?;
        if let Some(error) = inbox.reply_error {
            return Err(BackendError::Runtime(error));
        }
        Ok(())
    }
}

fn engine_binary_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("REVO_ENGINE_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    let executable = std::env::current_exe().ok()?;
    let sibling = executable.with_file_name("revo-engine");
    if sibling.is_file() {
        return Some(sibling);
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for profile in ["debug", "release"] {
        let candidate = manifest.join(format!("../revo-backend/target/{profile}/revo-engine"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

impl BackendRuntime for EngineClient {
    fn execute(&mut self, command: BackendCommand) -> Result<(), BackendError> {
        self.connection
            .send_request(&UiToEngine::Command(command))?;
        let inbox = self.connection.drain(DrainTarget::Snapshot)?;
        if let Some(error) = inbox.uncorrelated_error {
            return Err(BackendError::Runtime(error));
        }
        Ok(())
    }

    fn snapshot(&self) -> Result<BackendSnapshot, BackendError> {
        self.connection.send_request(&UiToEngine::RequestSnapshot)?;
        let inbox = self.connection.drain(DrainTarget::Snapshot)?;
        inbox.snapshot.ok_or(BackendError::Disconnected)
    }

    fn shutdown(&mut self) -> Result<(), BackendError> {
        self.connection.send_request(&UiToEngine::Shutdown)?;
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            match self.connection.read_message(Duration::from_millis(200)) {
                Ok(Some(_)) => continue,
                Ok(None) => continue,
                Err(_) => break,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revo_core::ipc::{decode_frame, encode_frame};

    /// Minimal in-test engine: replies to every request like the real
    /// serve loop so the client's request/reply mechanics are exercised
    /// without libobs.
    struct StubEngine {
        stream: UnixStream,
    }

    impl StubEngine {
        fn read_request(&mut self) -> Option<UiToEngine> {
            let mut header = [0u8; 4];
            self.stream.read_exact(&mut header).ok()?;
            let payload_len = u32::from_le_bytes(header) as usize;
            let mut payload = vec![0u8; payload_len];
            self.stream.read_exact(&mut payload).ok()?;
            let mut framed = header.to_vec();
            framed.extend_from_slice(&payload);
            decode_frame::<UiToEngine>(&framed).ok().map(|(m, _)| m)
        }

        fn send(&mut self, message: &EngineToUi) {
            let frame = encode_frame(message).expect("stub frame");
            self.stream.write_all(&frame).expect("stub send");
            self.stream.flush().expect("stub flush");
        }

        fn push_snapshot(&mut self, running: bool) {
            self.send(&EngineToUi::Snapshot(BackendSnapshot {
                obs_running: running,
                ..BackendSnapshot::default()
            }));
        }
    }

    /// Builds a client over a socket pair plus the stub's half.
    fn client_and_stub() -> (EngineClient, StubEngine) {
        let (client_half, server_half) = UnixStream::pair().expect("socket pair");
        let client = EngineClient::over_stream(client_half, None);
        (
            client,
            StubEngine {
                stream: server_half,
            },
        )
    }

    #[test]
    fn execute_round_trips_command_and_reports_engine_errors() {
        let (mut client, mut stub) = client_and_stub();

        // A command reply carries a snapshot; success surfaces as Ok.
        std::thread::spawn(move || {
            let request = stub.read_request().expect("command request");
            assert!(matches!(request, UiToEngine::Command(_)));
            stub.push_snapshot(true);
        });
        client
            .execute(BackendCommand::StartObs)
            .expect("execute ok");
    }

    #[test]
    fn execute_surfaces_uncorrelated_engine_errors() {
        let (mut client, mut stub) = client_and_stub();
        std::thread::spawn(move || {
            let _ = stub.read_request().expect("command request");
            stub.send(&EngineToUi::Error {
                request_id: None,
                message: "obs_startup failed".into(),
            });
            stub.push_snapshot(false);
        });
        let error = client
            .execute(BackendCommand::StartObs)
            .expect_err("engine error propagates");
        assert!(error.to_string().contains("obs_startup failed"));
    }

    #[test]
    fn snapshot_reads_the_reply_and_settles_stale_pushes() {
        let (client, mut stub) = client_and_stub();
        std::thread::spawn(move || {
            let _ = stub.read_request().expect("snapshot request");
            // A stale engine-pushed preview frame arrives before the reply.
            stub.send(&EngineToUi::PreviewFrame {
                data_uri: "data:image/png;base64,ZmFrZQ==".into(),
            });
            stub.push_snapshot(true);
        });
        let snapshot = client.snapshot().expect("snapshot reply");
        assert!(snapshot.obs_running);
    }

    #[test]
    fn preview_capture_prefers_the_pushed_slot() {
        let (client, mut stub) = client_and_stub();
        // The engine pushes a frame unprompted (its own cadence).
        stub.send(&EngineToUi::PreviewFrame {
            data_uri: "data:image/png;base64,c2xvdA==".into(),
        });
        let mut client = client;
        let frame = client
            .capture_preview_png(None)
            .expect("pushed frame drained");
        assert_eq!(frame, "data:image/png;base64,c2xvdA==");
        // The slot is consumed (newest wins); the next push is picked up the
        // same way, and the second call never issued a request.
        stub.send(&EngineToUi::PreviewFrame {
            data_uri: "data:image/png;base64,cHVzaGVk".into(),
        });
        let frame = client.capture_preview_png(None).expect("pushed again");
        assert_eq!(frame, "data:image/png;base64,cHVzaGVk");
    }

    #[test]
    fn preview_capture_correlates_the_request_id() {
        let (mut client, stub) = client_and_stub();
        std::thread::spawn(move || {
            let mut stub = stub;
            let request = stub.read_request().expect("capture request");
            let request_id = match request {
                UiToEngine::PreviewFrameOnce { request_id, .. } => request_id,
                other => panic!("expected PreviewFrameOnce, got {other:?}"),
            };
            // A stale reply carrying a different request id must be
            // ignored...
            stub.send(&EngineToUi::PreviewFrameData {
                request_id: request_id.wrapping_add(100),
                data_uri: Some("data:image/png;base64,c3RhbGU=".into()),
            });
            // ...and the correlated one wins (source offline: no frame).
            stub.send(&EngineToUi::PreviewFrameData {
                request_id,
                data_uri: None,
            });
        });
        let error = client
            .capture_preview_png(Some("camera"))
            .expect_err("missing frame errors");
        assert!(error.to_string().contains("no frame captured"));
    }

    #[test]
    fn eof_disables_the_connection() {
        let (client, stub) = client_and_stub();
        drop(stub);
        let mut client = client;
        let error = client.snapshot().expect_err("eof disconnects");
        assert_eq!(error, BackendError::Disconnected);
        // Later requests keep reporting the dead connection.
        let error = client
            .execute(BackendCommand::StartObs)
            .expect_err("still disconnected");
        assert_eq!(error, BackendError::Disconnected);
    }
}
