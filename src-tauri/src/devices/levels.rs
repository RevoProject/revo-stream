use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use revo_lib::obs;

static OBS_AUDIO_LEVELS: OnceLock<Mutex<HashMap<String, f64>>> = OnceLock::new();
static VOLTETERS: OnceLock<Mutex<Vec<VolmeterEntry>>> = OnceLock::new();

/// (source, volmeter) pair tracked so the volmeter can be detached before its
/// source is destroyed. Raw pointers are only ever used under the global
/// `VOLTETERS` mutex on the OBS command threads.
struct VolmeterEntry {
    source: *mut obs::obs_source_t,
    volmeter: *mut obs::obs_volmeter_t,
}

// SAFETY: entries are only accessed while holding the VOLTETERS mutex, and
// libobs calls serialize the pointer use.
unsafe impl Send for VolmeterEntry {}

fn levels_map() -> &'static Mutex<HashMap<String, f64>> {
    OBS_AUDIO_LEVELS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn volmeters() -> &'static Mutex<Vec<VolmeterEntry>> {
    VOLTETERS.get_or_init(|| Mutex::new(Vec::new()))
}

pub(crate) fn get_levels() -> &'static Mutex<HashMap<String, f64>> {
    levels_map()
}

pub(crate) fn init_audio_volmeters() {
    if let Ok(mut levels) = levels_map().lock() {
        levels.clear();
    }

    unsafe {
        obs::obs_enum_sources(Some(enum_sources_callback), std::ptr::null_mut());
    }
}

pub(crate) fn cleanup_audio_volmeters() {
    let drained: Vec<VolmeterEntry> = if let Ok(mut list) = volmeters().lock() {
        list.drain(..).collect()
    } else {
        Vec::new()
    };
    for entry in drained {
        unsafe {
            destroy_volmeter(entry.source, entry.volmeter);
        }
    }
    if let Ok(mut levels) = levels_map().lock() {
        levels.clear();
    }
}

/// Detach and destroy every volmeter bound to `source`. Must be called before
/// the source is destroyed: the volmeter callback keeps a raw pointer to it.
pub(crate) unsafe fn detach_volmeters_for_source(source: *mut obs::obs_source_t) {
    if source.is_null() {
        return;
    }

    let removed: Vec<VolmeterEntry> = if let Ok(mut list) = volmeters().lock() {
        let mut i = 0;
        let mut removed = Vec::new();
        while i < list.len() {
            if list[i].source == source {
                removed.push(list.remove(i));
            } else {
                i += 1;
            }
        }
        removed
    } else {
        Vec::new()
    };

    if removed.is_empty() {
        return;
    }

    let name_ptr = obs::obs_source_get_name(source);
    if !name_ptr.is_null() {
        let name = std::ffi::CStr::from_ptr(name_ptr)
            .to_string_lossy()
            .to_string();
        if let Ok(mut levels) = levels_map().lock() {
            levels.remove(&name);
        }
    }

    for entry in removed {
        destroy_volmeter(entry.source, entry.volmeter);
    }
}

unsafe fn destroy_volmeter(source: *mut obs::obs_source_t, volmeter: *mut obs::obs_volmeter_t) {
    if volmeter.is_null() {
        return;
    }
    obs::obs_volmeter_remove_callback(
        volmeter,
        Some(volmeter_updated_callback),
        source as *mut std::os::raw::c_void,
    );
    obs::obs_volmeter_detach_source(volmeter);
    obs::obs_volmeter_destroy(volmeter);
}

pub(crate) unsafe fn attach_volmeter(source: *mut obs::obs_source_t) {
    if source.is_null() {
        return;
    }

    let flags = obs::obs_source_get_output_flags(source);
    if flags & obs::OBS_SOURCE_AUDIO == 0 {
        return;
    }

    if let Ok(list) = volmeters().lock() {
        if list.iter().any(|e| e.source == source) {
            return;
        }
    }

    let volmeter = obs::obs_volmeter_create(obs::obs_fader_type_OBS_FADER_LOG);
    if volmeter.is_null() {
        return;
    }

    obs::obs_volmeter_set_peak_meter_type(volmeter, obs::obs_peak_meter_type_SAMPLE_PEAK_METER);

    obs::obs_volmeter_add_callback(
        volmeter,
        Some(volmeter_updated_callback),
        source as *mut std::os::raw::c_void,
    );

    if !obs::obs_volmeter_attach_source(volmeter, source) {
        obs::obs_volmeter_remove_callback(
            volmeter,
            Some(volmeter_updated_callback),
            source as *mut std::os::raw::c_void,
        );
        obs::obs_volmeter_destroy(volmeter);
        return;
    }

    if let Ok(mut list) = volmeters().lock() {
        list.push(VolmeterEntry { source, volmeter });
    } else {
        destroy_volmeter(source, volmeter);
    }
}

unsafe extern "C" fn enum_sources_callback(
    _param: *mut std::os::raw::c_void,
    source: *mut obs::obs_source_t,
) -> bool {
    if source.is_null() {
        return true;
    }

    attach_volmeter(source);

    true
}

unsafe extern "C" fn volmeter_updated_callback(
    param: *mut std::os::raw::c_void,
    _magnitude: *const f32,
    peak: *const f32,
    _input_peak: *const f32,
) {
    if param.is_null() || peak.is_null() {
        return;
    }

    let source = param as *mut obs::obs_source_t;

    // Guard against a callback racing with detach/destroy of the source.
    let registered = volmeters()
        .lock()
        .map(|list| list.iter().any(|e| e.source == source))
        .unwrap_or(false);
    if !registered {
        return;
    }

    let name_ptr = obs::obs_source_get_name(source);
    if name_ptr.is_null() {
        return;
    }

    let name = std::ffi::CStr::from_ptr(name_ptr)
        .to_string_lossy()
        .to_string();

    let mut max_peak = 0.0f32;
    for i in 0..obs::MAX_AUDIO_CHANNELS as usize {
        let val = *peak.add(i);
        if val > max_peak {
            max_peak = val;
        }
    }

    let percent = (max_peak * 100.0) as f64;

    if let Ok(mut levels) = levels_map().lock() {
        levels.insert(name, percent);
    }
}
