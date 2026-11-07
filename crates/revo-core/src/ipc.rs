//! Engine IPC protocol shared by the native engine process and GUI clients.
//!
//! The engine is a separate OS process owning the real libobs runtime so the
//! GPU stacks never collide inside the UI process (libobs OpenGL vs the UI's
//! Vulkan renderer). Both sides speak the same typed messages over a length
//! prefixed JSON stream on a local Unix socket:
//!
//! * [`UiToEngine`] — client requests: backend commands, snapshot requests,
//!   browser-interaction control, and engine shutdown.
//! * [`EngineToUi`] — engine pushes: state snapshots, preview frames (data
//!   URI PNGs), interaction replies, and error text.
//!
//! Frames are 4-byte little-endian lengths followed by the JSON payload;
//! [`encode_frame`] and [`decode_frame`] are the only framing primitives so
//! both processes stay byte-compatible.

use std::io;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::service::BackendCommand;

/// Maximum accepted frame payload (32 MiB) — a preview PNG at 4K with
/// base64-style JSON expansion stays far below this; anything larger is a
/// corrupted stream.
pub const MAX_FRAME_BYTES: u32 = 32 * 1024 * 1024;

/// One libobs debug-log row, mirroring the engine process's debug-log
/// registry (`revo-backend::logging::debug`). The detail JSON value travels
/// as its serialized string so the wire format stays JSON-friendly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DebugLogEntry {
    pub timestamp_ms: u64,
    pub action: String,
    pub detail_json: Option<String>,
}

/// Requests sent from the UI client to the engine process.
///
/// Requests carrying a `request_id` get a correlated reply (same id on
/// [`EngineToUi::PreviewFrameData`] / [`EngineToUi::InteractReply`] /
/// [`EngineToUi::DebugLogHistoryData`] / [`EngineToUi::Error`]);
/// fire-and-forget requests have none.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum UiToEngine {
    /// Apply one typed backend command; the engine replies with a fresh
    /// [`EngineToUi::Snapshot`] once the command settles.
    Command(BackendCommand),
    /// Ask for an immediate [`EngineToUi::Snapshot`].
    RequestSnapshot,
    /// Capture one preview frame now: `None` for the current scene
    /// composite, `Some(id)` for a single source; answered with
    /// [`EngineToUi::PreviewFrameData`].
    PreviewFrameOnce {
        request_id: u64,
        source_id: Option<String>,
    },
    /// Open the browser-interaction CDP session for one source.
    InteractOpen { request_id: u64, source_id: String },
    /// Capture the newest browser-source frame; answered with
    /// [`EngineToUi::InteractReply`].
    InteractFrame { request_id: u64, source_id: String },
    /// Forward one typed mouse event to the source's CDP session.
    InteractMouse {
        request_id: u64,
        source_id: String,
        kind: String,
        x: f64,
        y: f64,
        button: String,
        buttons: i64,
        click_count: i64,
        delta_x: f64,
        delta_y: f64,
    },
    /// Forward one typed key event to the source's CDP session.
    InteractKey {
        request_id: u64,
        source_id: String,
        kind: String,
        key: String,
        code: String,
        key_code: i64,
        text: String,
    },
    /// Re-run the session's custom CSS/JS injection.
    InteractReapply { request_id: u64, source_id: String },
    /// Close the source's CDP session and its headless browser.
    InteractClose { source_id: String },
    /// Fetch the engine's libobs debug-log history; answered with
    /// [`EngineToUi::DebugLogHistoryData`]. The debug console reads libobs
    /// logs this way because the registry lives in the engine process.
    DebugLogHistory { request_id: u64 },
    /// Clear the engine's libobs debug-log registry; answered with
    /// [`EngineToUi::DebugLogCleared`].
    DebugLogClear { request_id: u64 },
    /// Fetch one source's full settings (name, type, params, typed property
    /// specs from the libobs property API); answered with
    /// [`EngineToUi::SourceSettingsData`]. Powers the source editor and the
    /// undo history for settings edits.
    SourceSettings { request_id: u64, id: String },
    /// Stop the engine: the libobs runtime shuts down and the process exits.
    Shutdown,
}

/// Messages pushed from the engine process to the UI client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EngineToUi {
    /// Fresh backend state (after every command and snapshot request).
    Snapshot(crate::service::BackendSnapshot),
    /// Newest preview frame as a `data:image/png;base64,...` URI. The engine
    /// pushes these on its own cadence so the UI never blocks the libobs
    /// graphics thread.
    PreviewFrame { data_uri: String },
    /// Correlated reply to [`UiToEngine::PreviewFrameOnce`]; `None` when no
    /// frame could be captured (engine not running, unknown source).
    PreviewFrameData {
        request_id: u64,
        data_uri: Option<String>,
    },
    /// Correlated reply to the browser-interaction requests; `None` when the
    /// session has no frame yet or the call produced no artifact.
    InteractReply {
        request_id: u64,
        data_uri: Option<String>,
    },
    /// Correlated reply to [`UiToEngine::DebugLogHistory`]: the engine
    /// process's libobs debug-log registry contents.
    DebugLogHistoryData {
        request_id: u64,
        entries: Vec<DebugLogEntry>,
    },
    /// Correlated reply to [`UiToEngine::DebugLogClear`].
    DebugLogCleared { request_id: u64 },
    /// Correlated reply to [`UiToEngine::SourceSettings`]; `None` when the
    /// source is unknown or OBS is not initialized.
    SourceSettingsData {
        request_id: u64,
        settings: Option<crate::models::SourceSettingsData>,
    },
    /// Text for the status line (command results and startup errors).
    Status(String),
    /// Protocol-level or request failure; `request_id` correlates it with
    /// the originating request when one exists.
    Error {
        request_id: Option<u64>,
        message: String,
    },
}

/// Serializes one message into a length-prefixed frame.
pub fn encode_frame<T: Serialize>(message: &T) -> io::Result<Vec<u8>> {
    let payload = serde_json::to_vec(message)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let payload_len = u32::try_from(payload.len())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if payload_len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame payload exceeds {MAX_FRAME_BYTES} bytes"),
        ));
    }
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

/// Decodes one length-prefixed frame from the front of `buffer`, returning
/// the message and the number of bytes consumed.
pub fn decode_frame<T: DeserializeOwned>(buffer: &[u8]) -> io::Result<(T, usize)> {
    if buffer.len() < 4 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "short frame header",
        ));
    }
    let payload_len = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
    if payload_len > MAX_FRAME_BYTES as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame payload exceeds {MAX_FRAME_BYTES} bytes"),
        ));
    }
    let total = 4 + payload_len;
    if buffer.len() < total {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "short frame payload",
        ));
    }
    let message: T = serde_json::from_slice(&buffer[4..total])
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok((message, total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::{BackendCommand, BackendSnapshot, MoveDirection};

    #[test]
    fn frames_roundtrip_commands_and_snapshots() {
        let command = UiToEngine::Command(BackendCommand::MoveSource {
            id: "camera".into(),
            direction: MoveDirection::Up,
        });
        let frame = encode_frame(&command).expect("command frame encodes");
        let (decoded, consumed) = decode_frame::<UiToEngine>(&frame).expect("command decodes");
        assert_eq!(consumed, frame.len());
        assert_eq!(decoded, command);

        let snapshot = EngineToUi::Snapshot(BackendSnapshot {
            obs_running: true,
            ..BackendSnapshot::default()
        });
        let frame = encode_frame(&snapshot).expect("snapshot frame encodes");
        let (decoded, consumed) = decode_frame::<EngineToUi>(&frame).expect("snapshot decodes");
        assert_eq!(consumed, frame.len());
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn frames_roundtrip_preview_payloads() {
        let message = EngineToUi::PreviewFrame {
            data_uri: "data:image/png;base64,aGVsbG8=".into(),
        };
        let frame = encode_frame(&message).expect("frame encodes");
        // serde_json struct-variant payload: {"PreviewFrame":{...}}
        assert!(frame[4..].starts_with(b"{\"PreviewFrame\"".as_slice()));
        let (decoded, consumed) = decode_frame::<EngineToUi>(&frame).expect("frame decodes");
        assert_eq!(consumed, frame.len());
        assert_eq!(decoded, message);
    }

    #[test]
    fn frames_roundtrip_debug_log_payloads() {
        let request = UiToEngine::DebugLogHistory { request_id: 7 };
        let frame = encode_frame(&request).expect("request frame encodes");
        let (decoded, _) = decode_frame::<UiToEngine>(&frame).expect("request decodes");
        assert_eq!(decoded, request);

        let reply = EngineToUi::DebugLogHistoryData {
            request_id: 7,
            entries: vec![DebugLogEntry {
                timestamp_ms: 12,
                action: "obs_output_start".into(),
                detail_json: Some("{\"key\":\"value\"}".into()),
            }],
        };
        let frame = encode_frame(&reply).expect("reply frame encodes");
        let (decoded, _) = decode_frame::<EngineToUi>(&frame).expect("reply decodes");
        assert_eq!(decoded, reply);
    }

    #[test]
    fn partial_buffers_report_eof_not_corruption() {
        let frame = encode_frame(&UiToEngine::RequestSnapshot).expect("frame encodes");
        assert!(decode_frame::<UiToEngine>(&frame[..2]).is_err());
        assert!(decode_frame::<UiToEngine>(&frame[..frame.len() - 1]).is_err());
        assert!(decode_frame::<UiToEngine>(&frame).is_ok());
    }

    #[test]
    fn oversized_lengths_are_rejected_without_allocation() {
        let evil = [0xff, 0xff, 0xff, 0x7f];
        assert!(decode_frame::<UiToEngine>(&evil).is_err());
    }
}
