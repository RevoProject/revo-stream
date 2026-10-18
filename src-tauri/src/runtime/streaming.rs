pub(crate) fn start_streaming(
	state: tauri::State<crate::ObsState>,
	stream_url: String,
) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}
	if !runtime.output_stream.is_null() {
		return Ok("Streaming already active".to_string());
	}

	let mut preference = runtime.video_encoder_preference;
	for _attempt in 0..3 {
		let mut service = std::ptr::null_mut();
		let mut video_encoder = std::ptr::null_mut();
		let mut audio_encoder = std::ptr::null_mut();
		let output = super::encoders::create_rtmp_stream_output(
			&stream_url,
			&mut service,
			&mut video_encoder,
			&mut audio_encoder,
			preference,
		);
		let output = if output.is_null() {
			super::encoders::create_ffmpeg_stream_output(&stream_url)
		} else {
			output
		};
		if output.is_null() {
			return Err("failed to create stream output".to_string());
		}

		let flags = unsafe { revo_lib::obs::obs_output_get_flags(output) };
		if (flags & revo_lib::obs::OBS_OUTPUT_ENCODED) == 0 {
			unsafe {
				revo_lib::obs::obs_output_set_media(
					output,
					revo_lib::obs::obs_get_video(),
					revo_lib::obs::obs_get_audio(),
				)
			};
		}

		if unsafe { revo_lib::obs::obs_output_start(output) } {
			runtime.output_stream = output;
			runtime.stream_service = service;
			runtime.stream_video_encoder = video_encoder;
			runtime.stream_audio_encoder = audio_encoder;
			return Ok("Streaming started".to_string());
		}

		let last_error = unsafe {
			super::helpers::cstr_to_string(revo_lib::obs::obs_output_get_last_error(output))
		};
		let hardware_encoder = super::encoders::is_hardware_video_encoder(video_encoder);
		let texture_nvenc = super::encoders::is_texture_nvenc_encoder(video_encoder);
		unsafe {
			revo_lib::obs::obs_output_release(output);
			if !service.is_null() {
				revo_lib::obs::obs_service_release(service);
			}
			if !video_encoder.is_null() {
				revo_lib::obs::obs_encoder_release(video_encoder);
			}
			if !audio_encoder.is_null() {
				revo_lib::obs::obs_encoder_release(audio_encoder);
			}
		}

		if texture_nvenc && !matches!(preference, crate::VideoEncoderPreference::NvencLegacy) {
			eprintln!(
				"[stream] texture NVENC start failed ({last_error}); retrying with legacy/non-texture NVENC"
			);
			preference = crate::VideoEncoderPreference::NvencLegacy;
			continue;
		}
		if hardware_encoder && !matches!(preference, crate::VideoEncoderPreference::Software) {
			eprintln!(
				"[stream] hardware encoder start failed ({last_error}); retrying with x264"
			);
			preference = crate::VideoEncoderPreference::Software;
			continue;
		}
		return Err(format!("stream start failed: {last_error}"));
	}
	unreachable!("streaming retry loop is bounded to three attempts")
}

pub(crate) fn stop_streaming(state: tauri::State<crate::ObsState>) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if runtime.output_stream.is_null() {
		return Ok("Streaming not active".to_string());
	}
	super::helpers::stop_streaming_internal(&mut runtime);
	Ok("Streaming stopped".to_string())
}
