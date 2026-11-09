//! Typed device catalog for capture sources (Svelte `listQuickDeviceOptions`
//! parity): pulse input/output devices, V4L2/video capture devices, and
//! window picker items. An unknown source type yields an empty catalog, and
//! the UI then falls back to the source's own OBS property options.

fn audio_items(devices: Vec<crate::AudioDevice>) -> Vec<crate::SourceTypeItem> {
    devices
        .into_iter()
        .map(|device| crate::SourceTypeItem {
            id: device.id,
            label: device.name,
            audio_capable: true,
        })
        .collect()
}

/// Enumerate selectable devices for one source type string.
pub fn list_device_catalog(source_type: &str) -> Result<Vec<crate::SourceTypeItem>, String> {
    let t = source_type.to_ascii_lowercase();
    if t == "pulse_input_capture"
        || t.contains("audio_input")
        || t.contains("audio_capture")
        || t.contains("audio_line")
    {
        return crate::devices::audio::list_pulse_devices("input").map(audio_items);
    }
    if t == "pulse_output_capture" || t.contains("audio_output") {
        return crate::devices::audio::list_pulse_devices("output").map(audio_items);
    }
    if t.contains("video") || t.contains("v4l2") {
        return crate::sources::video_devices::list_video_device_picker_items();
    }
    if t.contains("window") || t.contains("xcomposite") {
        return crate::sources::window_picker::list_window_picker_items();
    }
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::list_device_catalog;

    #[test]
    fn source_types_without_a_lister_yield_an_empty_catalog() {
        // Display/pulse-less or text sources fall back to the UI's property
        // options; the catalog itself stays empty (and never errors here).
        let devices = list_device_catalog("text_ft2_source_v2").expect("plain types never error");
        assert!(devices.is_empty());
    }
}
