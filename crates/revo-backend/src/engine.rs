//! Engine IPC server: hosts the typed obs command backend in a dedicated
//! process and speaks `revo_core::ipc` over the platform local transport
//! ([`revo_core::transport::EngineStream`]).
//!
//! The UI owns no libobs code at all on this path: it spawns the engine,
//! streams [`UiToEngine`] requests, and consumes [`EngineToUi`] pushes. The
//! serve loop is single-threaded with a read timeout so preview frames keep
//! flowing between requests — the same newest-frame-wins cadence the
//! in-process preview loop used, now on the engine side of the process
//! boundary where libobs OpenGL cannot disturb the UI's Vulkan renderer.

use std::io::ErrorKind;
use std::time::Duration;

use revo_core::ipc::{decode_frame, encode_frame, EngineToUi, UiToEngine};
use revo_core::service::{BackendError, BackendRuntime};
use revo_core::transport::EngineStream;

use crate::obs_service::{ObsBackendRuntime, PREVIEW_CAPTURE_INTERVAL_MS};

/// Serves one connected UI client until it disconnects. Returns when the
/// client goes away or asks for [`UiToEngine::Shutdown`].
pub fn serve_connection<S: EngineStream + ?Sized>(stream: &mut S, backend: &mut ObsBackendRuntime) {
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
fn read_request<S: EngineStream + ?Sized>(stream: &mut S) -> std::io::Result<Option<UiToEngine>> {
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
fn apply_request<S: EngineStream + ?Sized>(
    stream: &mut S,
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
            // last_error); protocol errors travel separately.
            if let Err(error) = result {
                let _ = send(
                    stream,
                    &EngineToUi::Error {
                        request_id: None,
                        message: error.to_string(),
                    },
                );
            }
            push_snapshot(stream, backend);
            true
        }
        UiToEngine::RequestSnapshot => {
            push_snapshot(stream, backend);
            true
        }
        UiToEngine::PreviewFrameOnce {
            request_id,
            source_id,
        } => {
            let frame = backend.capture_preview_frame(source_id.as_deref()).ok();
            match frame {
                Some(frame) => {
                    let _ = send_preview_frame(stream, request_id, &frame);
                }
                None => {
                    let _ = send(stream, &EngineToUi::PreviewUnavailable { request_id });
                }
            }
            true
        }
        UiToEngine::InteractOpen {
            request_id,
            source_id,
        } => {
            let message = match backend.interact_open(&source_id) {
                Ok(()) => EngineToUi::InteractReply {
                    request_id,
                    data_uri: None,
                },
                Err(error) => error_to_reply(request_id, error),
            };
            let _ = send(stream, &message);
            push_snapshot(stream, backend);
            true
        }
        UiToEngine::InteractFrame {
            request_id,
            source_id,
        } => {
            let message = match backend.interact_frame(&source_id) {
                Ok(data_uri) => EngineToUi::InteractReply {
                    request_id,
                    data_uri,
                },
                Err(error) => error_to_reply(request_id, error),
            };
            let _ = send(stream, &message);
            true
        }
        UiToEngine::InteractMouse {
            request_id,
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
            let message = match backend.interact_mouse(
                &source_id,
                &kind,
                x,
                y,
                &button,
                buttons,
                click_count,
                delta_x,
                delta_y,
            ) {
                Ok(()) => EngineToUi::InteractReply {
                    request_id,
                    data_uri: None,
                },
                Err(error) => error_to_reply(request_id, error),
            };
            let _ = send(stream, &message);
            true
        }
        UiToEngine::InteractKey {
            request_id,
            source_id,
            kind,
            key,
            code,
            key_code,
            text,
        } => {
            let message =
                match backend.interact_key(&source_id, &kind, &key, &code, key_code, &text) {
                    Ok(()) => EngineToUi::InteractReply {
                        request_id,
                        data_uri: None,
                    },
                    Err(error) => error_to_reply(request_id, error),
                };
            let _ = send(stream, &message);
            true
        }
        UiToEngine::InteractReapply {
            request_id,
            source_id,
        } => {
            let message = match backend.interact_reapply(&source_id) {
                Ok(()) => EngineToUi::InteractReply {
                    request_id,
                    data_uri: None,
                },
                Err(error) => error_to_reply(request_id, error),
            };
            let _ = send(stream, &message);
            true
        }
        UiToEngine::InteractClose { source_id } => {
            backend.interact_close(&source_id);
            true
        }
        UiToEngine::DebugLogHistory { request_id } => {
            let entries = crate::logging::debug::get_debug_log_entries()
                .into_iter()
                .map(|event| revo_core::ipc::DebugLogEntry {
                    timestamp_ms: event.timestamp_ms,
                    action: event.action,
                    detail_json: event
                        .detail
                        .as_ref()
                        .map(serde_json::to_string)
                        .transpose()
                        .ok()
                        .flatten(),
                })
                .collect();
            let _ = send(
                stream,
                &EngineToUi::DebugLogHistoryData {
                    request_id,
                    entries,
                },
            );
            true
        }
        UiToEngine::DebugLogClear { request_id } => {
            crate::logging::debug::clear_debug_log_entries();
            let _ = send(stream, &EngineToUi::DebugLogCleared { request_id });
            true
        }
        UiToEngine::SourceFilters { request_id, id } => {
            let filters = crate::sources::source_ops::get_source_filters(&backend.runtime, id)
                .unwrap_or_default();
            let _ = send(
                stream,
                &EngineToUi::SourceFiltersData {
                    request_id,
                    filters,
                },
            );
            true
        }
        UiToEngine::SourceSettings { request_id, id } => {
            let settings = crate::sources::source_ops::get_source_settings(&backend.runtime, id)
                .ok()
                .and_then(|value| serde_json::from_value(value).ok());
            let _ = send(
                stream,
                &EngineToUi::SourceSettingsData {
                    request_id,
                    settings,
                },
            );
            true
        }
        UiToEngine::FilterSettings {
            request_id,
            source_id,
            filter_name,
            kind,
        } => {
            let settings = crate::sources::source_ops::get_filter_settings(
                &backend.runtime,
                source_id,
                filter_name,
                kind,
            )
            .ok()
            .and_then(|value| serde_json::from_value(value).ok());
            let _ = send(
                stream,
                &EngineToUi::FilterSettingsData {
                    request_id,
                    settings,
                },
            );
            true
        }
        UiToEngine::SystemFonts { request_id } => {
            let fonts = crate::sources::fonts::list_system_fonts().unwrap_or_default();
            let _ = send(stream, &EngineToUi::SystemFontsData { request_id, fonts });
            true
        }
        UiToEngine::SourceDevices {
            request_id,
            source_type,
        } => {
            let devices = crate::sources::device_catalog::list_device_catalog(&source_type)
                .unwrap_or_default();
            let _ = send(
                stream,
                &EngineToUi::SourceDevicesData {
                    request_id,
                    devices,
                },
            );
            true
        }
    }
}

/// Correlates a request failure back to the client; runtime failures carry
/// their message, queue/protocol failures their display form.
fn error_to_reply(request_id: u64, error: BackendError) -> EngineToUi {
    let message = match &error {
        BackendError::Runtime(message) => message.clone(),
        other => other.to_string(),
    };
    EngineToUi::Error {
        request_id: Some(request_id),
        message,
    }
}

fn push_snapshot<S: EngineStream + ?Sized>(stream: &mut S, backend: &mut ObsBackendRuntime) {
    if let Ok(snapshot) = backend.snapshot() {
        let _ = send(stream, &EngineToUi::Snapshot(snapshot));
    }
}

fn push_preview_frame<S: EngineStream + ?Sized>(stream: &mut S, backend: &mut ObsBackendRuntime) {
    if !backend.runtime.initialized {
        return;
    }
    // Push frames carry request id 0; the UI correlates none.
    if let Ok(frame) = backend.capture_preview_frame(None) {
        let _ = send_preview_frame(stream, 0, &frame);
    }
}

fn send<S: EngineStream + ?Sized>(stream: &mut S, message: &EngineToUi) -> std::io::Result<()> {
    let frame = encode_frame(message)?;
    stream.write_all(&frame)?;
    stream.flush()
}

/// Writes one raw RGBA frame as a binary preview frame
/// (`RVPF` magic + header + pixels) instead of a JSON control frame, so no
/// capture is ever PNG-encoded or base64-expanded on this side. The 4-byte
/// magic prefix keeps the two frame kinds distinguishable without a framing
/// header; `request_id` 0 marks engine pushes.
fn send_preview_frame<S: EngineStream + ?Sized>(
    stream: &mut S,
    request_id: u64,
    frame: &crate::runtime::preview::CapturedPreviewFrame,
) -> std::io::Result<()> {
    // The wire buffer needs its own copy: the retained `Arc` frame stays
    // available for the freeze and stale-fallback paths.
    let encoded = revo_core::ipc::encode_preview_frame(&revo_core::ipc::PreviewFrame {
        request_id,
        width: frame.width(),
        height: frame.height(),
        rgba: frame.pixels().to_vec(),
    });
    stream.write_all(&encoded)?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use revo_core::transport::test_pair;
    use std::io::{Read, Write};

    type Stream = Box<dyn EngineStream + Send>;

    fn socket_pair() -> (Stream, Stream) {
        test_pair().expect("engine transport pair")
    }

    fn connected_backend() -> ObsBackendRuntime {
        ObsBackendRuntime::new(None, Box::new(crate::obs_service::NativeObsPlatform))
    }

    #[test]
    fn serve_loop_pushes_snapshot_then_executes_commands_then_stops() {
        let (mut client, mut server) = socket_pair();
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

        // Correlated preview request; the engine is offline so the reply
        // reports unavailability with this request's id.
        send_message(
            &mut client,
            &UiToEngine::PreviewFrameOnce {
                request_id: 41,
                source_id: None,
            },
        )
        .expect("preview request");
        assert!(matches!(
            read_message(&mut client).expect("preview reply"),
            EngineToUi::PreviewUnavailable { request_id: 41 }
        ));

        // Shutdown stops the serve loop.
        send_message(&mut client, &UiToEngine::Shutdown).expect("shutdown send");
        assert!(matches!(
            read_message(&mut client).expect("shutdown snapshot"),
            EngineToUi::Snapshot(_)
        ));
        worker.join().expect("serve loop exits cleanly");
    }

    #[test]
    fn serve_loop_answers_debug_log_requests() {
        let (mut client, mut server) = socket_pair();
        let mut backend = connected_backend();
        let worker = std::thread::spawn(move || serve_connection(&mut server, &mut backend));

        // Boot snapshot first, then a debug-log history round trip.
        let _ = read_message(&mut client).expect("boot snapshot");
        send_message(&mut client, &UiToEngine::DebugLogHistory { request_id: 5 })
            .expect("history request");
        match read_message(&mut client).expect("history reply") {
            EngineToUi::DebugLogHistoryData {
                request_id,
                entries: _,
            } => {
                // The registry is process-global and other tests may have
                // written rows, so only the correlation contract is asserted
                // here; the correlated id is what the console relies on.
                assert_eq!(request_id, 5);
            }
            other => panic!("expected debug log history, got {other:?}"),
        }

        send_message(&mut client, &UiToEngine::DebugLogClear { request_id: 6 })
            .expect("clear request");
        assert!(matches!(
            read_message(&mut client).expect("clear reply"),
            EngineToUi::DebugLogCleared { request_id: 6 }
        ));

        send_message(&mut client, &UiToEngine::Shutdown).expect("shutdown send");
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

    #[test]
    fn preview_frames_travel_as_binary_frames_not_json() {
        // 2x2 solid red frame.
        let frame = crate::runtime::preview::CapturedPreviewFrame::from_parts(
            2,
            2,
            vec![
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
            ],
        );
        let (mut client, mut server) = socket_pair();

        send_preview_frame(&mut client, 7, &frame).expect("frame write");

        server
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("timeout");
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let read = server.read(&mut chunk).expect("frame read");
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
            if revo_core::ipc::is_preview_frame(&buffer) {
                break;
            }
        }

        let (decoded, consumed) =
            revo_core::ipc::decode_preview_frame(&buffer).expect("binary frame decodes");
        assert_eq!(consumed, buffer.len(), "whole frame read");
        assert_eq!(decoded.request_id, 7);
        assert_eq!((decoded.width, decoded.height), (2, 2));
        assert_eq!(decoded.rgba, vec![255u8, 0, 0, 255].repeat(4));
    }
}
