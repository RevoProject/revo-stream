pub(crate) fn start_recording(
	state: tauri::State<crate::ObsState>,
	output_path: String,
) -> Result<String, String> {
	eprintln!("[record] start_recording...");
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}
	if !runtime.output_record.is_null() {
		return Ok("Recording already active".to_string());
	}

	let resolved_path = super::helpers::resolve_record_path(&output_path)?;
	eprintln!("[record] path={:?}", resolved_path);
	let output = super::encoders::create_muxer_output_with_path(
		resolved_path.to_string_lossy().as_ref(),
		true,
	);
	if output.is_null() {
		eprintln!("[record] FAILED to create output");
		return Err("failed to create recording output".to_string());
	}

	let flags = unsafe { revo_lib::obs::obs_output_get_flags(output) };
	eprintln!("[record] output flags={}", flags);
	unsafe {
		// Ensure recording uses at least mix 1 (bit 0), otherwise some outputs may start with video-only.
		revo_lib::obs::obs_output_set_mixers(output, 1);
	}
	let mut pending_video_encoder: *mut revo_lib::obs::obs_encoder = std::ptr::null_mut();
	let mut pending_audio_encoder: *mut revo_lib::obs::obs_encoder = std::ptr::null_mut();
	if (flags & revo_lib::obs::OBS_OUTPUT_ENCODED) != 0 {
		let video_encoder = super::encoders::create_video_encoder(runtime.video_encoder_preference);
		let audio_encoder = super::encoders::create_audio_encoder();
		if video_encoder.is_null() || audio_encoder.is_null() {
			unsafe {
				if !video_encoder.is_null() {
					revo_lib::obs::obs_encoder_release(video_encoder);
				}
				if !audio_encoder.is_null() {
					revo_lib::obs::obs_encoder_release(audio_encoder);
				}
				revo_lib::obs::obs_output_release(output);
			}
			return Err("failed to create encoders".to_string());
		}
		unsafe {
			revo_lib::obs::obs_encoder_set_video(video_encoder, revo_lib::obs::obs_get_video());
			revo_lib::obs::obs_encoder_set_audio(audio_encoder, revo_lib::obs::obs_get_audio());
			revo_lib::obs::obs_output_set_video_encoder(output, video_encoder);
			revo_lib::obs::obs_output_set_audio_encoder(output, audio_encoder, 0);
		}
		// Publish to runtime only after a successful start (see below).
		pending_video_encoder = video_encoder;
		pending_audio_encoder = audio_encoder;
	} else {
		unsafe {
			revo_lib::obs::obs_output_set_media(
				output,
				revo_lib::obs::obs_get_video(),
				revo_lib::obs::obs_get_audio(),
			);
		}
	}

	let started = unsafe { revo_lib::obs::obs_output_start(output) };
	if !started {
		let last_error = unsafe {
			super::helpers::cstr_to_string(revo_lib::obs::obs_output_get_last_error(output))
		};
		unsafe {
			revo_lib::obs::obs_output_release(output);
			if !pending_video_encoder.is_null() {
				revo_lib::obs::obs_encoder_release(pending_video_encoder);
			}
			if !pending_audio_encoder.is_null() {
				revo_lib::obs::obs_encoder_release(pending_audio_encoder);
			}
		}
		pending_video_encoder = std::ptr::null_mut();
		pending_audio_encoder = std::ptr::null_mut();
		let fallback = super::encoders::create_muxer_output_with_path(
			resolved_path.to_string_lossy().as_ref(),
			false,
		);
		if fallback.is_null() {
			return Err(format!("recording start failed: {last_error}"));
		}
		let flags = unsafe { revo_lib::obs::obs_output_get_flags(fallback) };
		unsafe {
			// Ensure fallback recording output uses at least mix 1 as well.
			revo_lib::obs::obs_output_set_mixers(fallback, 1);
		}
		if (flags & revo_lib::obs::OBS_OUTPUT_ENCODED) != 0 {
			let video_encoder = super::encoders::create_video_encoder(runtime.video_encoder_preference);
			let audio_encoder = super::encoders::create_audio_encoder();
			if video_encoder.is_null() || audio_encoder.is_null() {
				unsafe {
					if !video_encoder.is_null() {
						revo_lib::obs::obs_encoder_release(video_encoder);
					}
					if !audio_encoder.is_null() {
						revo_lib::obs::obs_encoder_release(audio_encoder);
					}
					revo_lib::obs::obs_output_release(fallback);
				}
				return Err("failed to create encoders".to_string());
			}
			unsafe {
				revo_lib::obs::obs_encoder_set_video(video_encoder, revo_lib::obs::obs_get_video());
				revo_lib::obs::obs_encoder_set_audio(audio_encoder, revo_lib::obs::obs_get_audio());
				revo_lib::obs::obs_output_set_video_encoder(fallback, video_encoder);
				revo_lib::obs::obs_output_set_audio_encoder(fallback, audio_encoder, 0);
			}
			pending_video_encoder = video_encoder;
			pending_audio_encoder = audio_encoder;
		} else {
			unsafe {
				revo_lib::obs::obs_output_set_media(
					fallback,
					revo_lib::obs::obs_get_video(),
					revo_lib::obs::obs_get_audio(),
				);
			}
		}

		let started_fallback = unsafe { revo_lib::obs::obs_output_start(fallback) };
		if !started_fallback {
			let last_error = unsafe {
				super::helpers::cstr_to_string(revo_lib::obs::obs_output_get_last_error(fallback))
			};
			unsafe {
				revo_lib::obs::obs_output_release(fallback);
				if !pending_video_encoder.is_null() {
					revo_lib::obs::obs_encoder_release(pending_video_encoder);
				}
				if !pending_audio_encoder.is_null() {
					revo_lib::obs::obs_encoder_release(pending_audio_encoder);
				}
			}
			return Err(format!("recording start failed: {last_error}"));
		}
		runtime.output_record = fallback;
		runtime.record_video_encoder = pending_video_encoder;
		runtime.record_audio_encoder = pending_audio_encoder;
		runtime.last_record_path = Some(resolved_path.clone());
		return Ok(format!("Recording started: {}", resolved_path.to_string_lossy()));
	}

	runtime.output_record = output;
	runtime.record_video_encoder = pending_video_encoder;
	runtime.record_audio_encoder = pending_audio_encoder;
	runtime.last_record_path = Some(resolved_path.clone());
	Ok(format!("Recording started: {}", resolved_path.to_string_lossy()))
}

pub(crate) fn stop_recording(state: tauri::State<crate::ObsState>) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if runtime.output_record.is_null() {
		return Ok("Recording not active".to_string());
	}
	let last_path = runtime.last_record_path.clone();
	super::helpers::stop_recording_internal(&mut runtime);
	if let Some(path) = last_path {
		Ok(format!("Recording stopped: {}", path.to_string_lossy()))
	} else {
		Ok("Recording stopped".to_string())
	}
}
