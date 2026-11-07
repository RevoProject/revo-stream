//! Engine IPC server: hosts the typed obs command backend in a dedicated
//! process and speaks `revo_core::ipc` over a local Unix socket.
//!
//! The UI owns no libobs code at all on this path: it spawns the engine,
//! streams [`UiToEngine`] requests, and consumes [`EngineToUi`] pushes. The
//! serve loop is single-threaded with a read timeout so preview frames keep
//! flowing between requests — the same newest-frame-wins cadence the
//! in-process preview loop used, now on the engine side of the process
//! boundary where libobs OpenGL cannot disturb the UI's Vulkan renderer.

use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use revo_core::ipc::{decode_frame, encode_frame, EngineToUi, UiToEngine};
use revo_core::service::BackendRuntime;

use crate::obs_service::{ObsBackendRuntime, PREVIEW_CAPTURE_INTERVAL_MS};

/// Serves one connected UI client until it disconnects. Returns when the
/// client goes away or asks for [`UiToEngine::Shutdown`].
pub fn serve_connection(stream: &mut UnixStream, backend: &mut ObsBackendRuntime) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(PREVIEW_CAPTURE_INTERVAL_MS)));
    // First push: the UI boots its workspace on this snapshot before any
    // command arrives (parity with the in-process boot refresh).
    push_snapshot(stream, backend);
    loop {
        match read_request(stream) {
            Ok(Some(request)) => {
                if !apply_request(stream, backend, request) {
                    break;
                }
            }
            Ok(None) => break, // client disconnected
            Err(error) if timed_out(&error) => {
                push_preview_frame(stream, backend);
            }
            Err(_) => break,
        }
    }
    let _ = stream.flush();
}

fn timed_out(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
    )
}

/// Reads one full request frame; `Ok(None)` on orderly disconnect.
fn read_request(stream: &mut UnixStream) -> std::io::Result<Option<UiToEngine>> {
    let mut header = [0u8; 4];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if timed_out(&error) => return Err(error),
        Err(_) => return Ok(None),
    }
    let payload_len = u32::from_le_bytes(header) as usize;
    let mut buffer = vec![0u8; payload_len];
    match stream.read_exact(&mut buffer) {
        Ok(()) => {}
        Err(_) => return Ok(None),
    }
    let mut framed = header.to_vec();
    framed.extend_from_slice(&buffer);
    match decode_frame::<UiToEngine>(&framed) {
        Ok((message, _)) => Ok(Some(message)),
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "undecodable engine request",
        )),
    }
}

/// Applies one request; `false` when the serve loop should stop.
fn apply_request(
    stream: &mut UnixStream,
    backend: &mut ObsBackendRuntime,
    request: UiToEngine,
) -> bool {
    match request {
        UiToEngine::Shutdown => {
            let _ = backend.shutdown();
            push_snapshot(stream, backend);
            false
        }
        UiToEngine::Command(command) => {
            let result = backend.execute(command);
            // Command results surface through the next snapshot (including
            // last_error); the status text is informational only.
            if let Err(error) = result {
                let _ = send(stream, &EngineToUi::Status(error.to_string()));
            }
            push_snapshot(stream, backend);
            true
        }
        UiToEngine::RequestSnapshot => {
            push_snapshot(stream, backend);
            true
        }
        UiToEngine::InteractOpen { source_id } => {
            let outcome = backend.interact_open(&source_id);
            reply_interaction(stream, backend, outcome);
            true
        }
        UiToEngine::InteractFrame { source_id } => {
            let data_uri = backend.interact_frame(&source_id);
            let message = match data_uri {
                Ok(data_uri) => EngineToUi::InteractFrame {
                    source_id,
                    data_uri,
                },
                Err(error) => EngineToUi::Error(error.to_string()),
            };
            let _ = send(stream, &message);
            push_snapshot(stream, backend);
            true
        }
        UiToEngine::InteractMouse {
            source_id,
            kind,
            x,
            y,
            button,
            buttons,
            click_count,
            delta_x,
            delta_y,
        } => {
            let outcome = backend.interact_mouse(
                &source_id,
                &kind,
                x,
                y,
                &button,
                buttons,
                click_count,
                delta_x,
                delta_y,
            );
            reply_interaction(stream, backend, outcome);
            true
        }
        UiToEngine::InteractKey {
            source_id,
            kind,
            key,
            code,
            key_code,
            text,
        } => {
            let outcome = backend.interact_key(&source_id, &kind, &key, &code, key_code, &text);
            reply_interaction(stream, backend, outcome);
            true
        }
        UiToEngine::InteractReapply { source_id } => {
            let outcome = backend.interact_reapply(&source_id);
            reply_interaction(stream, backend, outcome);
            true
        }
        UiToEngine::InteractClose { source_id } => {
            backend.interact_close(&source_id);
            true
        }
    }
}

/// Sends one interaction outcome snapshot; errors surface through
/// `last_error` on the next snapshot rather than the status text.
fn reply_interaction(
    stream: &mut UnixStream,
    backend: &mut ObsBackendRuntime,
    outcome: Result<(), revo_core::service::BackendError>,
) {
    if let Err(revo_core::service::BackendError::Runtime(message)) = &outcome {
        let _ = send(stream, &EngineToUi::Status(message.clone()));
    }
    push_snapshot(stream, backend);
}

fn push_snapshot(stream: &mut UnixStream, backend: &mut ObsBackendRuntime) {
    if let Ok(snapshot) = backend.snapshot() {
        let _ = send(stream, &EngineToUi::Snapshot(snapshot));
    }
}

fn push_preview_frame(stream: &mut UnixStream, backend: &mut ObsBackendRuntime) {
    if !backend.runtime.initialized {
        return;
    }
    if let Ok(data_uri) = backend.capture_preview_png(None) {
        let _ = send(stream, &EngineToUi::PreviewFrame { data_uri });
    }
}

fn send(stream: &mut UnixStream, message: &EngineToUi) -> std::io::Result<()> {
    let frame = encode_frame(message)?;
    stream.write_all(&frame)?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream as Stream;

    fn connected_backend() -> ObsBackendRuntime {
        ObsBackendRuntime::new(None, Box::new(crate::obs_service::NativeObsPlatform))
    }

    #[test]
    fn serve_loop_pushes_snapshot_then_executes_commands_then_stops() {
        let (mut client, mut server) = Stream::pair().expect("socket pair");
        let mut backend = connected_backend();
        let worker = std::thread::spawn(move || serve_connection(&mut server, &mut backend));

        // The first push is the boot snapshot (engine not initialized yet).
        let boot = read_message(&mut client).expect("boot snapshot");
        match boot {
            EngineToUi::Snapshot(snapshot) => assert!(!snapshot.obs_running),
            other => panic!("expected snapshot, got {other:?}"),
        }

        // A command round-trip: request then fresh snapshot.
        send_message(
            &mut client,
            &UiToEngine::Command(revo_core::service::BackendCommand::SetStudioMode(true)),
        )
        .expect("command send");
        let reply = read_message(&mut client).expect("command snapshot");
        match reply {
            EngineToUi::Snapshot(snapshot) => assert!(snapshot.studio_mode),
            other => panic!("expected snapshot, got {other:?}"),
        }

        // RequestSnapshot echoes the current state.
        send_message(&mut client, &UiToEngine::RequestSnapshot).expect("snapshot request");
        assert!(matches!(
            read_message(&mut client).expect("requested snapshot"),
            EngineToUi::Snapshot(_)
        ));

        // Shutdown stops the serve loop.
        send_message(&mut client, &UiToEngine::Shutdown).expect("shutdown send");
        assert!(matches!(
            read_message(&mut client).expect("shutdown snapshot"),
            EngineToUi::Snapshot(_)
        ));
        worker.join().expect("serve loop exits cleanly");
    }

    fn send_message(stream: &mut Stream, message: &UiToEngine) -> std::io::Result<()> {
        let frame = encode_frame(message)?;
        stream.write_all(&frame)?;
        stream.flush()
    }

    fn read_message(stream: &mut Stream) -> std::io::Result<EngineToUi> {
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut header = [0u8; 4];
        stream.read_exact(&mut header)?;
        let payload_len = u32::from_le_bytes(header) as usize;
        let mut payload = vec![0u8; payload_len];
        stream.read_exact(&mut payload)?;
        let mut framed = header.to_vec();
        framed.extend_from_slice(&payload);
        let (message, _) = decode_frame::<EngineToUi>(&framed)?;
        Ok(message)
    }
}
