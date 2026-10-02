use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use revo_lib::obs;

static OBS_AUDIO_LEVELS: OnceLock<Mutex<HashMap<String, f64>>> = OnceLock::new();

fn levels_map() -> &'static Mutex<HashMap<String, f64>> {
    OBS_AUDIO_LEVELS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn get_levels() -> &'static Mutex<HashMap<String, f64>> {
    levels_map()
}

pub(crate) fn init_audio_volmeters(runtime: &mut crate::ObsRuntime) {
    if let Ok(mut levels) = levels_map().lock() {
        levels.clear();
    }

    unsafe {
        obs::obs_enum_sources(
            Some(enum_sources_callback),
            runtime as *mut _ as *mut std::os::raw::c_void,
        );
    }
}

pub(crate) fn cleanup_audio_volmeters(runtime: &mut crate::ObsRuntime) {
    unsafe {
        for volmeter in runtime.audio_volmeters.drain(..) {
            if !volmeter.is_null() {
                obs::obs_volmeter_detach_source(volmeter);
                obs::obs_volmeter_destroy(volmeter);
            }
        }
    }
    if let Ok(mut levels) = levels_map().lock() {
        levels.clear();
    }
}

pub(crate) unsafe fn attach_volmeter(
    runtime: &mut crate::ObsRuntime,
    source: *mut obs::obs_source_t,
) {
    if source.is_null() {
        return;
    }

    let flags = obs::obs_source_get_output_flags(source);
    if flags & obs::OBS_SOURCE_AUDIO == 0 {
        return;
    }

    let volmeter = obs::obs_volmeter_create(obs::obs_fader_type_OBS_FADER_LOG);
    if volmeter.is_null() {
        return;
    }

    obs::obs_volmeter_set_peak_meter_type(
        volmeter,
        obs::obs_peak_meter_type_SAMPLE_PEAK_METER,
    );

    obs::obs_volmeter_add_callback(
        volmeter,
        Some(volmeter_updated_callback),
        source as *mut std::os::raw::c_void,
    );

    obs::obs_volmeter_attach_source(volmeter, source);

    runtime.audio_volmeters.push(volmeter);
}

unsafe extern "C" fn enum_sources_callback(
    param: *mut std::os::raw::c_void,
    source: *mut obs::obs_source_t,
) -> bool {
    if source.is_null() {
        return true;
    }

    let runtime = &mut *(param as *mut crate::ObsRuntime);

    attach_volmeter(runtime, source);

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
