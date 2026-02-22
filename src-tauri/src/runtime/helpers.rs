pub(crate) fn cstr_to_string(ptr: *const i8) -> String {
	if ptr.is_null() {
		return String::new();
	}
	unsafe { std::ffi::CStr::from_ptr(ptr).to_string_lossy().to_string() }
}

pub(crate) fn stop_recording_internal(runtime: &mut crate::ObsRuntime) {
	unsafe {
		if !runtime.output_record.is_null() {
			if let Ok(output) = revo_lib::recording::RecordingOutput::from_raw(runtime.output_record) {
				output.stop();
			}
			revo_lib::obs::obs_output_release(runtime.output_record);
		}
		if !runtime.record_video_encoder.is_null() {
			revo_lib::obs::obs_encoder_release(runtime.record_video_encoder);
		}
		if !runtime.record_audio_encoder.is_null() {
			revo_lib::obs::obs_encoder_release(runtime.record_audio_encoder);
		}
	}
	runtime.output_record = std::ptr::null_mut();
	runtime.record_video_encoder = std::ptr::null_mut();
	runtime.record_audio_encoder = std::ptr::null_mut();
	runtime.last_record_path = None;
}

pub(crate) fn stop_streaming_internal(runtime: &mut crate::ObsRuntime) {
	unsafe {
		if !runtime.output_stream.is_null() {
			if let Ok(output) = revo_lib::streaming::StreamingOutput::from_raw(runtime.output_stream) {
				output.stop();
			}
			revo_lib::obs::obs_output_release(runtime.output_stream);
		}
		if !runtime.stream_service.is_null() {
			revo_lib::obs::obs_service_release(runtime.stream_service);
		}
		if !runtime.stream_video_encoder.is_null() {
			revo_lib::obs::obs_encoder_release(runtime.stream_video_encoder);
		}
		if !runtime.stream_audio_encoder.is_null() {
			revo_lib::obs::obs_encoder_release(runtime.stream_audio_encoder);
		}
	}
	runtime.output_stream = std::ptr::null_mut();
	runtime.stream_service = std::ptr::null_mut();
	runtime.stream_video_encoder = std::ptr::null_mut();
	runtime.stream_audio_encoder = std::ptr::null_mut();
}

pub(crate) fn reset_video_with_resolution(width: u32, height: u32) -> bool {
	unsafe {
		let graphics_module = std::ffi::CString::new("libobs-opengl").unwrap();
		let mut ovi: revo_lib::obs::obs_video_info = std::mem::zeroed();
		ovi.graphics_module = graphics_module.as_ptr();
		ovi.fps_num = 30;
		ovi.fps_den = 1;
		ovi.base_width = width;
		ovi.base_height = height;
		ovi.output_width = width;
		ovi.output_height = height;
		ovi.output_format = revo_lib::obs::video_format_VIDEO_FORMAT_NV12;
		ovi.adapter = 0;
		ovi.gpu_conversion = true;
		ovi.colorspace = revo_lib::obs::video_colorspace_VIDEO_CS_709;
		ovi.range = revo_lib::obs::video_range_type_VIDEO_RANGE_PARTIAL;
		ovi.scale_type = revo_lib::obs::obs_scale_type_OBS_SCALE_BICUBIC;

		revo_lib::obs::obs_reset_video(&mut ovi as *mut _)
			== revo_lib::obs::OBS_VIDEO_SUCCESS as i32
	}
}

pub(crate) fn resolve_record_path(output_path: &str) -> Result<std::path::PathBuf, String> {
	let raw_input = output_path.trim();
	let mut base_path = std::path::PathBuf::from(raw_input);
	if base_path.as_os_str().is_empty() {
		return Err("Output path required".to_string());
	}
	if base_path.is_relative() {
		let base = std::env::current_dir().map_err(|e| format!("failed to resolve cwd: {e}"))?;
		base_path = base.join(base_path);
	}

	let unix_ts = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_secs())
		.unwrap_or(0);

	let looks_like_dir = raw_input.ends_with('/') || raw_input.ends_with('\\') || base_path.is_dir();

	let mut path = if looks_like_dir {
		base_path.join(format!("record_{unix_ts}.mp4"))
	} else {
		let parent = base_path
			.parent()
			.map(std::path::PathBuf::from)
			.unwrap_or_else(std::path::PathBuf::new);
		let stem = base_path
			.file_stem()
			.and_then(|s| s.to_str())
			.filter(|s| !s.trim().is_empty())
			.unwrap_or("record");
		let ext = base_path
			.extension()
			.and_then(|e| e.to_str())
			.filter(|e| !e.trim().is_empty())
			.unwrap_or("mp4");
		parent.join(format!("{stem}_{unix_ts}.{ext}"))
	};

	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent).map_err(|e| format!("failed to create output dir: {e}"))?;
	}

	if path.exists() {
		let parent = path
			.parent()
			.map(std::path::PathBuf::from)
			.unwrap_or_else(std::path::PathBuf::new);
		let stem = path
			.file_stem()
			.and_then(|s| s.to_str())
			.unwrap_or("record")
			.to_string();
		let ext = path
			.extension()
			.and_then(|e| e.to_str())
			.unwrap_or("mp4")
			.to_string();

		for idx in 1..1000 {
			let candidate = parent.join(format!("{stem}_{idx}.{ext}"));
			if !candidate.exists() {
				path = candidate;
				break;
			}
		}
	}

	Ok(path)
}
