//! Engine IPC protocol shared by the native engine process and GUI clients.
//!
//! The engine is a separate OS process owning the real libobs runtime so the
//! GPU stacks never collide inside the UI process (libobs OpenGL vs the UI's
//! Vulkan renderer). Both sides speak the same typed messages over a length
//! prefixed JSON stream on a local Unix socket:
//!
//! * [`UiToEngine`] — client requests: backend commands, snapshot requests,
//!   browser-interaction control, and engine shutdown.
//! * [`EngineToUi`] — engine pushes: state snapshots, interaction replies,
//!   and error text.
//!
//! Control frames are 4-byte little-endian lengths followed by the JSON
//! payload; [`encode_frame`] and [`decode_frame`] are the only JSON framing
//! primitives so both processes stay byte-compatible.
//!
//! Scene preview frames are the one exception: they travel as binary frames
//! ([`PREVIEW_FRAME_MAGIC`] header + raw RGBA pixels) because a JSON payload
//! would have to expand every pixel into a decimal array and every capture
//! would be PNG-encoded and base64-expanded on the engine side only to be
//! decoded again in the UI. [`is_preview_frame`] lets a reader tell the two
//! frame kinds apart by peeking at the first four bytes.

use std::io;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::service::BackendCommand;

/// Maximum accepted JSON frame payload (32 MiB); anything larger is a
/// corrupted stream.
pub const MAX_FRAME_BYTES: u32 = 32 * 1024 * 1024;

/// Magic prefix of a binary preview frame: `RVPF`.
pub const PREVIEW_FRAME_MAGIC: [u8; 4] = *b"RVPF";

/// Fixed preview-frame header size: magic, request id, width, height, and
/// the RGBA payload length (all little-endian).
pub const PREVIEW_FRAME_HEADER_BYTES: usize = 4 + 8 + 4 + 4 + 4;

/// Maximum accepted RGBA preview payload (64 MiB) — a 4K RGBA frame is
/// ~33 MiB, so this covers every preview size the capture path can request.
pub const MAX_PREVIEW_FRAME_BYTES: u32 = 64 * 1024 * 1024;

/// One captured preview frame in raw RGBA.
///
/// Transported as a binary frame ([`encode_preview_frame`] /
/// [`decode_preview_frame`]) so the engine never PNG-encodes or base64
/// expands a capture the UI is about to upload as a texture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewFrame {
    /// `0` for engine pushes; the originating request id for correlated
    /// replies to [`UiToEngine::PreviewFrameOnce`].
    pub request_id: u64,
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA8 pixels, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

impl PreviewFrame {
    /// A push frame (`request_id` 0).
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        Self {
            request_id: 0,
            width,
            height,
            rgba,
        }
    }

    /// Tags this frame as the reply to `request_id`.
    pub fn with_request_id(mut self, request_id: u64) -> Self {
        self.request_id = request_id;
        self
    }

    /// Byte length the RGBA payload must have for the declared dimensions.
    pub fn expected_rgba_len(&self) -> Option<usize> {
        (self.width as usize)
            .checked_mul(self.height as usize)?
            .checked_mul(4)
    }

    /// Rejects empty dimensions and payloads that do not match them.
    pub fn validate(&self) -> Result<(), String> {
        if self.width == 0 || self.height == 0 {
            return Err("preview frame has empty dimensions".to_string());
        }
        match self.expected_rgba_len() {
            Some(expected) if expected == self.rgba.len() => Ok(()),
            Some(expected) => Err(format!(
                "preview frame payload is {} bytes, expected {expected}",
                self.rgba.len()
            )),
            None => Err("preview frame dimensions overflow".to_string()),
        }
    }
}

/// Serializes one binary preview frame: magic, request id, dimensions, RGBA
/// payload length, then the pixels themselves.
pub fn encode_preview_frame(frame: &PreviewFrame) -> Vec<u8> {
    let mut out = Vec::with_capacity(PREVIEW_FRAME_HEADER_BYTES + frame.rgba.len());
    out.extend_from_slice(&PREVIEW_FRAME_MAGIC);
    out.extend_from_slice(&frame.request_id.to_le_bytes());
    out.extend_from_slice(&frame.width.to_le_bytes());
    out.extend_from_slice(&frame.height.to_le_bytes());
    out.extend_from_slice(&(frame.rgba.len() as u32).to_le_bytes());
    out.extend_from_slice(&frame.rgba);
    out
}

/// True when `prefix` starts a binary preview frame. Only the first four
/// bytes are inspected, so a reader can peek before committing to a payload
/// length.
pub fn is_preview_frame(prefix: &[u8]) -> bool {
    prefix.len() >= 4 && prefix[..4] == PREVIEW_FRAME_MAGIC
}

/// Decodes one binary preview frame from the front of `buffer`, returning
/// the frame and the number of bytes consumed.
pub fn decode_preview_frame(buffer: &[u8]) -> io::Result<(PreviewFrame, usize)> {
    if buffer.len() < PREVIEW_FRAME_HEADER_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "short preview frame header",
        ));
    }
    if !is_preview_frame(buffer) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a preview frame",
        ));
    }
    let mut request_id_bytes = [0u8; 8];
    request_id_bytes.copy_from_slice(&buffer[4..12]);
    let request_id = u64::from_le_bytes(request_id_bytes);
    let width = u32::from_le_bytes([buffer[12], buffer[13], buffer[14], buffer[15]]);
    let height = u32::from_le_bytes([buffer[16], buffer[17], buffer[18], buffer[19]]);
    let rgba_len = u32::from_le_bytes([buffer[20], buffer[21], buffer[22], buffer[23]]) as usize;
    if rgba_len > MAX_PREVIEW_FRAME_BYTES as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("preview frame payload exceeds {MAX_PREVIEW_FRAME_BYTES} bytes"),
        ));
    }
    let total = PREVIEW_FRAME_HEADER_BYTES + rgba_len;
    if buffer.len() < total {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "short preview frame payload",
        ));
    }
    let frame = PreviewFrame {
        request_id,
        width,
        height,
        rgba: buffer[PREVIEW_FRAME_HEADER_BYTES..total].to_vec(),
    };
    frame
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok((frame, total))
}

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
/// [`EngineToUi::PreviewUnavailable`] / [`EngineToUi::InteractReply`] /
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
    /// composite, `Some(id)` for a single source; answered with a binary
    /// preview frame ([`PreviewFrame`]) or
    /// [`EngineToUi::PreviewUnavailable`].
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
    /// Fetch one source's filter list; answered with
    /// [`EngineToUi::SourceFiltersData`].
    SourceFilters { request_id: u64, id: String },
    /// Fetch one filter's full settings (typed property specs from the
    /// filter's libobs property API); answered with
    /// [`EngineToUi::FilterSettingsData`]. An unknown filter name falls back
    /// to the `kind`'s default property set so locally added (unsaved)
    /// filters are editable too.
    FilterSettings {
        request_id: u64,
        source_id: String,
        filter_name: String,
        kind: String,
    },
    /// Enumerate system font families (engine side `fc-list`); answered
    /// with [`EngineToUi::SystemFontsData`]. Powers the font pickers.
    SystemFonts { request_id: u64 },
    /// Enumerate the device catalog for one capture source type; answered
    /// with [`EngineToUi::SourceDevicesData`]. Powers the "Select device"
    /// quick action picker.
    SourceDevices {
        request_id: u64,
        source_type: String,
    },
    /// Stop the engine: the libobs runtime shuts down and the process exits.
    Shutdown,
}

/// Messages pushed from the engine process to the UI client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EngineToUi {
    /// Fresh backend state (after every command and snapshot request).
    Snapshot(crate::service::BackendSnapshot),
    /// Correlated reply to [`UiToEngine::PreviewFrameOnce`] when no frame
    /// could be captured (engine not running, unknown source). A successful
    /// capture answers with a binary preview frame carrying the request id.
    PreviewUnavailable { request_id: u64 },
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
    /// Correlated reply to [`UiToEngine::SourceFilters`].
    SourceFiltersData {
        request_id: u64,
        filters: Vec<crate::models::SourceFilterItem>,
    },
    /// Correlated reply to [`UiToEngine::FilterSettings`]; `None` when the
    /// filter cannot be resolved and its `kind` has no default properties.
    FilterSettingsData {
        request_id: u64,
        settings: Option<crate::models::SourceSettingsData>,
    },
    /// Correlated reply to [`UiToEngine::SystemFonts`]; may be empty when no
    /// fonts could be enumerated.
    SystemFontsData {
        request_id: u64,
        fonts: Vec<crate::models::SourceTypeItem>,
    },
    /// Correlated reply to [`UiToEngine::SourceDevices`]; may be empty when
    /// the source type has no dedicated lister (the UI then falls back to
    /// the source's own OBS property options).
    SourceDevicesData {
        request_id: u64,
        devices: Vec<crate::models::SourceTypeItem>,
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
        // 2x2 solid red RGBA frame.
        let frame = PreviewFrame::new(2, 2, vec![255u8, 0, 0, 255].repeat(4));
        let encoded = encode_preview_frame(&frame);
        assert!(is_preview_frame(&encoded));
        // A JSON frame length prefix is far below the magic, so the two
        // frame kinds never collide.
        let json =
            encode_frame(&EngineToUi::PreviewUnavailable { request_id: 1 }).expect("json frame");
        assert!(!is_preview_frame(&json));
        let (decoded, consumed) = decode_preview_frame(&encoded).expect("preview decodes");
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded, frame);

        // The correlated reply tag survives the round trip.
        let reply = frame.clone().with_request_id(41);
        let encoded = encode_preview_frame(&reply);
        let (decoded, _) = decode_preview_frame(&encoded).expect("reply decodes");
        assert_eq!(decoded.request_id, 41);
        assert_eq!(decoded.rgba, reply.rgba);
    }

    #[test]
    fn malformed_preview_frames_are_rejected() {
        let frame = PreviewFrame::new(2, 2, vec![255u8, 0, 0, 255].repeat(4));
        let encoded = encode_preview_frame(&frame);

        // Truncated header and payload both report EOF, not corruption.
        assert!(decode_preview_frame(&encoded[..4]).is_err());
        assert!(decode_preview_frame(&encoded[..encoded.len() - 1]).is_err());

        // A JSON frame is not a preview frame.
        let json =
            encode_frame(&EngineToUi::PreviewUnavailable { request_id: 1 }).expect("json frame");
        assert!(decode_preview_frame(&json).is_err());

        // Dimensions that disagree with the payload are rejected.
        let lying = PreviewFrame {
            width: 3,
            height: 3,
            ..frame.clone()
        };
        assert!(decode_preview_frame(&encode_preview_frame(&lying)).is_err());

        // Empty dimensions are rejected.
        let empty = PreviewFrame::new(0, 0, Vec::new());
        assert!(decode_preview_frame(&encode_preview_frame(&empty)).is_err());
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
    fn frames_roundtrip_filter_settings_and_system_font_payloads() {
        let request = UiToEngine::FilterSettings {
            request_id: 9,
            source_id: "title".into(),
            filter_name: "Color Correction 2".into(),
            kind: "color_correction".into(),
        };
        let frame = encode_frame(&request).expect("filter settings request encodes");
        let (decoded, consumed) = decode_frame::<UiToEngine>(&frame).expect("request decodes");
        assert_eq!(consumed, frame.len());
        assert_eq!(decoded, request);

        let reply = EngineToUi::FilterSettingsData {
            request_id: 9,
            settings: Some(crate::models::SourceSettingsData::default()),
        };
        let frame = encode_frame(&reply).expect("filter settings reply encodes");
        let (decoded, _) = decode_frame::<EngineToUi>(&frame).expect("reply decodes");
        assert_eq!(decoded, reply);

        let fonts = EngineToUi::SystemFontsData {
            request_id: 10,
            fonts: vec![crate::models::SourceTypeItem {
                id: "DejaVu Sans".into(),
                label: "DejaVu Sans".into(),
                audio_capable: false,
            }],
        };
        let frame = encode_frame(&fonts).expect("font list frame encodes");
        let (decoded, consumed) = decode_frame::<EngineToUi>(&frame).expect("fonts decode");
        assert_eq!(consumed, frame.len());
        assert_eq!(decoded, fonts);
    }

    #[test]
    fn device_catalog_request_and_reply_roundtrip() {
        let request = UiToEngine::SourceDevices {
            request_id: 11,
            source_type: "pulse_input_capture".into(),
        };
        let frame = encode_frame(&request).expect("device request encodes");
        let (decoded, consumed) = decode_frame::<UiToEngine>(&frame).expect("request decodes");
        assert_eq!(consumed, frame.len());
        assert_eq!(decoded, request);

        let reply = EngineToUi::SourceDevicesData {
            request_id: 11,
            devices: vec![crate::models::SourceTypeItem {
                id: "alsa_input.pci-0000".into(),
                label: "Built-in Microphone".into(),
                audio_capable: true,
            }],
        };
        let frame = encode_frame(&reply).expect("device reply encodes");
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
