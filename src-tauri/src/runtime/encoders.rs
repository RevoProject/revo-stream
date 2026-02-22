// Encoder/output-related runtime bridge module.

pub(crate) fn set_encoder_preference(
	state: tauri::State<crate::ObsState>,
	preference: String,
) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	let normalized = preference.trim().to_lowercase();
	runtime.video_encoder_preference = match normalized.as_str() {
		"software" | "x264" | "obs_x264" | "legacy" | "legacy_x264" => {
			crate::VideoEncoderPreference::Software
		}
		_ => crate::VideoEncoderPreference::Hardware,
	};
	Ok(format!("Encoder preference set to {}", normalized))
}

pub(crate) fn create_video_encoder(
	preference: crate::VideoEncoderPreference,
) -> *mut revo_lib::obs::obs_encoder {
	match preference {
		crate::VideoEncoderPreference::Software => create_x264_encoder(),
		crate::VideoEncoderPreference::Hardware => {
			let hw = create_hardware_encoder();
			if !hw.is_null() {
				hw
			} else {
				create_x264_encoder()
			}
		}
	}
}

pub(crate) fn create_hardware_encoder() -> *mut revo_lib::obs::obs_encoder {
	let candidates = [
		"h264_nvenc",
		"ffmpeg_nvenc",
		"h264_qsv",
		"h264_vaapi",
		"h264_amf",
	];
	for id in candidates.iter() {
		if !encoder_id_available(id) {
			continue;
		}
		let enc = try_create_encoder(id, "revo_video_hw");
		if !enc.is_null() {
			return enc;
		}
	}
	std::ptr::null_mut()
}

pub(crate) fn encoder_id_available(id: &str) -> bool {
	let mut idx: usize = 0;
	unsafe {
		loop {
			let mut id_ptr: *const std::os::raw::c_char = std::ptr::null();
			if !revo_lib::obs::obs_enum_encoder_types(
				idx,
				&mut id_ptr as *mut *const std::os::raw::c_char,
			) {
				break;
			}
			if !id_ptr.is_null() {
				let value = std::ffi::CStr::from_ptr(id_ptr).to_string_lossy();
				if value.eq_ignore_ascii_case(id) {
					return true;
				}
			}
			idx += 1;
		}
	}
	false
}

pub(crate) fn try_create_encoder(
	id: &str,
	name: &str,
) -> *mut revo_lib::obs::obs_encoder {
	unsafe {
		let settings = revo_lib::obs::obs_data_create();
		if !settings.is_null() {
			let bitrate_key = std::ffi::CString::new("bitrate").unwrap();
			revo_lib::obs::obs_data_set_int(settings, bitrate_key.as_ptr(), 2500);

			let rc_key = std::ffi::CString::new("rate_control").unwrap();
			let rc_val = std::ffi::CString::new("CBR").unwrap();
			revo_lib::obs::obs_data_set_string(settings, rc_key.as_ptr(), rc_val.as_ptr());

			let keyint_key = std::ffi::CString::new("keyint_sec").unwrap();
			revo_lib::obs::obs_data_set_int(settings, keyint_key.as_ptr(), 2);
		}
		let id_c = std::ffi::CString::new(id).unwrap();
		let name_c = std::ffi::CString::new(name).unwrap();
		let enc = revo_lib::obs::obs_video_encoder_create(
			id_c.as_ptr(),
			name_c.as_ptr(),
			settings,
			std::ptr::null_mut(),
		);
		if !settings.is_null() {
			revo_lib::obs::obs_data_release(settings);
		}
		enc
	}
}

pub(crate) fn create_x264_encoder() -> *mut revo_lib::obs::obs_encoder {
	unsafe {
		let settings = revo_lib::obs::obs_data_create();
		if !settings.is_null() {
			let key = std::ffi::CString::new("bitrate").unwrap();
			revo_lib::obs::obs_data_set_int(settings, key.as_ptr(), 2500);

			let rc_key = std::ffi::CString::new("rate_control").unwrap();
			let rc_val = std::ffi::CString::new("CBR").unwrap();
			revo_lib::obs::obs_data_set_string(settings, rc_key.as_ptr(), rc_val.as_ptr());

			let buf_key = std::ffi::CString::new("buffer_size").unwrap();
			revo_lib::obs::obs_data_set_int(settings, buf_key.as_ptr(), 2500);

			let keyint_key = std::ffi::CString::new("keyint_sec").unwrap();
			revo_lib::obs::obs_data_set_int(settings, keyint_key.as_ptr(), 2);

			let bframes_key = std::ffi::CString::new("bframes").unwrap();
			revo_lib::obs::obs_data_set_int(settings, bframes_key.as_ptr(), 0);

			let profile_key = std::ffi::CString::new("profile").unwrap();
			let profile_val = std::ffi::CString::new("baseline").unwrap();
			revo_lib::obs::obs_data_set_string(settings, profile_key.as_ptr(), profile_val.as_ptr());

			let tune_key = std::ffi::CString::new("tune").unwrap();
			let tune_val = std::ffi::CString::new("zerolatency").unwrap();
			revo_lib::obs::obs_data_set_string(settings, tune_key.as_ptr(), tune_val.as_ptr());

			let repeat_key = std::ffi::CString::new("repeat_headers").unwrap();
			revo_lib::obs::obs_data_set_bool(settings, repeat_key.as_ptr(), true);

			let opts_key = std::ffi::CString::new("x264opts").unwrap();
			let opts_val = std::ffi::CString::new("nal-hrd=cbr:force-cfr=1:open-gop=0:aud=1").unwrap();
			revo_lib::obs::obs_data_set_string(settings, opts_key.as_ptr(), opts_val.as_ptr());
		}
		let mut enc = std::ptr::null_mut();
		for encoder_id in ["obs_x264", "x264"] {
			if !encoder_id_available(encoder_id) {
				continue;
			}
			let id = std::ffi::CString::new(encoder_id).unwrap();
			let name = std::ffi::CString::new("revo_video").unwrap();
			enc = revo_lib::obs::obs_video_encoder_create(
				id.as_ptr(),
				name.as_ptr(),
				settings,
				std::ptr::null_mut(),
			);
			if !enc.is_null() {
				break;
			}
		}
		if !settings.is_null() {
			revo_lib::obs::obs_data_release(settings);
		}
		enc
	}
}

pub(crate) fn create_audio_encoder() -> *mut revo_lib::obs::obs_encoder {
	unsafe {
		let settings = revo_lib::obs::obs_data_create();
		if !settings.is_null() {
			let key = std::ffi::CString::new("bitrate").unwrap();
			revo_lib::obs::obs_data_set_int(settings, key.as_ptr(), 160);
		}
		let id = std::ffi::CString::new("ffmpeg_aac").unwrap();
		let name = std::ffi::CString::new("revo_audio").unwrap();
		let enc = revo_lib::obs::obs_audio_encoder_create(
			id.as_ptr(),
			name.as_ptr(),
			settings,
			0,
			std::ptr::null_mut(),
		);
		if !settings.is_null() {
			revo_lib::obs::obs_data_release(settings);
		}
		enc
	}
}

pub(crate) fn create_muxer_output_with_path(
	output_path: &str,
	prefer_ffmpeg_muxer: bool,
) -> *mut revo_lib::obs::obs_output {
	unsafe {
		let name = std::ffi::CString::new("revo_record").unwrap();

		if prefer_ffmpeg_muxer {
			let out_muxer = create_output_with_id("ffmpeg_muxer", output_path, true, &name);
			if !out_muxer.is_null() {
				return out_muxer;
			}
		}

		let out_ffmpeg = create_output_with_id("ffmpeg_output", output_path, false, &name);
		if !out_ffmpeg.is_null() {
			return out_ffmpeg;
		}

		std::ptr::null_mut()
	}
}

pub(crate) unsafe fn create_output_with_id(
	id: &str,
	output_path: &str,
	prefer_ffmpeg_muxer: bool,
	name: &std::ffi::CString,
) -> *mut revo_lib::obs::obs_output {
	let id_c = std::ffi::CString::new(id).unwrap();
	let settings = revo_lib::obs::obs_output_defaults(id_c.as_ptr());
	let settings = if settings.is_null() {
		revo_lib::obs::obs_data_create()
	} else {
		settings
	};

	if !settings.is_null() {
		let path_key = std::ffi::CString::new("path").unwrap();
		let path_val = std::ffi::CString::new(output_path).unwrap();
		revo_lib::obs::obs_data_set_string(settings, path_key.as_ptr(), path_val.as_ptr());

		let url_key = std::ffi::CString::new("url").unwrap();
		revo_lib::obs::obs_data_set_string(settings, url_key.as_ptr(), path_val.as_ptr());

		let format_key = std::ffi::CString::new("format").unwrap();
		let format_val = std::ffi::CString::new("mp4").unwrap();
		revo_lib::obs::obs_data_set_string(settings, format_key.as_ptr(), format_val.as_ptr());

		let format_name_key = std::ffi::CString::new("format_name").unwrap();
		revo_lib::obs::obs_data_set_string(
			settings,
			format_name_key.as_ptr(),
			format_val.as_ptr(),
		);

		if !prefer_ffmpeg_muxer {
			let venc_key = std::ffi::CString::new("video_encoder").unwrap();
			let venc_val = std::ffi::CString::new("libx264").unwrap();
			revo_lib::obs::obs_data_set_string(settings, venc_key.as_ptr(), venc_val.as_ptr());

			let aenc_key = std::ffi::CString::new("audio_encoder").unwrap();
			let aenc_val = std::ffi::CString::new("aac").unwrap();
			revo_lib::obs::obs_data_set_string(settings, aenc_key.as_ptr(), aenc_val.as_ptr());

			let vbit_key = std::ffi::CString::new("video_bitrate").unwrap();
			revo_lib::obs::obs_data_set_int(settings, vbit_key.as_ptr(), 2500);

			let abit_key = std::ffi::CString::new("audio_bitrate").unwrap();
			revo_lib::obs::obs_data_set_int(settings, abit_key.as_ptr(), 160);

			let gop_key = std::ffi::CString::new("gop_size").unwrap();
			revo_lib::obs::obs_data_set_int(settings, gop_key.as_ptr(), 250);
		}
	}

	let out = revo_lib::obs::obs_output_create(
		id_c.as_ptr(),
		name.as_ptr(),
		settings,
		std::ptr::null_mut(),
	);
	if !settings.is_null() {
		revo_lib::obs::obs_data_release(settings);
	}
	out
}

pub(crate) fn create_ffmpeg_stream_output(rtmp_url: &str) -> *mut revo_lib::obs::obs_output {
	unsafe {
		let settings = revo_lib::obs::obs_data_create();
		if !settings.is_null() {
			let path_key = std::ffi::CString::new("path").unwrap();
			let url_key = std::ffi::CString::new("url").unwrap();
			let path_val = std::ffi::CString::new(rtmp_url).unwrap();
			revo_lib::obs::obs_data_set_string(settings, path_key.as_ptr(), path_val.as_ptr());
			revo_lib::obs::obs_data_set_string(settings, url_key.as_ptr(), path_val.as_ptr());

			let format_key = std::ffi::CString::new("format").unwrap();
			let format_val = std::ffi::CString::new("flv").unwrap();
			revo_lib::obs::obs_data_set_string(settings, format_key.as_ptr(), format_val.as_ptr());

			let format_name_key = std::ffi::CString::new("format_name").unwrap();
			revo_lib::obs::obs_data_set_string(
				settings,
				format_name_key.as_ptr(),
				format_val.as_ptr(),
			);

			let protocol_key = std::ffi::CString::new("protocol").unwrap();
			let protocol_val = std::ffi::CString::new("rtmp").unwrap();
			revo_lib::obs::obs_data_set_string(settings, protocol_key.as_ptr(), protocol_val.as_ptr());

			let venc_key = std::ffi::CString::new("video_encoder").unwrap();
			let venc_val = std::ffi::CString::new("libx264").unwrap();
			revo_lib::obs::obs_data_set_string(settings, venc_key.as_ptr(), venc_val.as_ptr());

			let aenc_key = std::ffi::CString::new("audio_encoder").unwrap();
			let aenc_val = std::ffi::CString::new("aac").unwrap();
			revo_lib::obs::obs_data_set_string(settings, aenc_key.as_ptr(), aenc_val.as_ptr());

			let vbit_key = std::ffi::CString::new("video_bitrate").unwrap();
			revo_lib::obs::obs_data_set_int(settings, vbit_key.as_ptr(), 2500);

			let abit_key = std::ffi::CString::new("audio_bitrate").unwrap();
			revo_lib::obs::obs_data_set_int(settings, abit_key.as_ptr(), 160);

			let gop_key = std::ffi::CString::new("gop_size").unwrap();
			revo_lib::obs::obs_data_set_int(settings, gop_key.as_ptr(), 60);
		}

		let output_name = std::ffi::CString::new("revo_stream").unwrap();
		let output_id = std::ffi::CString::new("ffmpeg_output").unwrap();
		let output = revo_lib::obs::obs_output_create(
			output_id.as_ptr(),
			output_name.as_ptr(),
			settings,
			std::ptr::null_mut(),
		);
		if !settings.is_null() {
			revo_lib::obs::obs_data_release(settings);
		}
		output
	}
}

pub(crate) fn create_rtmp_stream_output(
	rtmp_url: &str,
	out_service: &mut *mut revo_lib::obs::obs_service,
	out_video_encoder: &mut *mut revo_lib::obs::obs_encoder,
	out_audio_encoder: &mut *mut revo_lib::obs::obs_encoder,
	preference: crate::VideoEncoderPreference,
) -> *mut revo_lib::obs::obs_output {
	unsafe {
		let (server, key) = split_rtmp_url(rtmp_url);
		let settings = revo_lib::obs::obs_data_create();
		if settings.is_null() {
			return std::ptr::null_mut();
		}

		let server_key = std::ffi::CString::new("server").unwrap();
		let server_val = std::ffi::CString::new(server).unwrap();
		revo_lib::obs::obs_data_set_string(settings, server_key.as_ptr(), server_val.as_ptr());

		let key_key = std::ffi::CString::new("key").unwrap();
		let key_val = std::ffi::CString::new(key).unwrap();
		revo_lib::obs::obs_data_set_string(settings, key_key.as_ptr(), key_val.as_ptr());

		let service_id = std::ffi::CString::new("rtmp_custom").unwrap();
		let service_name = std::ffi::CString::new("revo_service").unwrap();
		let service = revo_lib::obs::obs_service_create(
			service_id.as_ptr(),
			service_name.as_ptr(),
			settings,
			std::ptr::null_mut(),
		);
		revo_lib::obs::obs_data_release(settings);
		if service.is_null() {
			return std::ptr::null_mut();
		}

		let output_id = std::ffi::CString::new("rtmp_output").unwrap();
		let output_name = std::ffi::CString::new("revo_stream").unwrap();
		let output = revo_lib::obs::obs_output_create(
			output_id.as_ptr(),
			output_name.as_ptr(),
			std::ptr::null_mut(),
			std::ptr::null_mut(),
		);
		if output.is_null() {
			revo_lib::obs::obs_service_release(service);
			return std::ptr::null_mut();
		}

		let video_encoder = create_video_encoder(preference);
		let audio_encoder = create_audio_encoder();
		if video_encoder.is_null() || audio_encoder.is_null() {
			if !video_encoder.is_null() {
				revo_lib::obs::obs_encoder_release(video_encoder);
			}
			if !audio_encoder.is_null() {
				revo_lib::obs::obs_encoder_release(audio_encoder);
			}
			revo_lib::obs::obs_output_release(output);
			revo_lib::obs::obs_service_release(service);
			return std::ptr::null_mut();
		}

		revo_lib::obs::obs_encoder_set_video(video_encoder, revo_lib::obs::obs_get_video());
		revo_lib::obs::obs_encoder_set_audio(audio_encoder, revo_lib::obs::obs_get_audio());
		revo_lib::obs::obs_output_set_video_encoder(output, video_encoder);
		revo_lib::obs::obs_output_set_audio_encoder(output, audio_encoder, 0);
		revo_lib::obs::obs_output_set_service(output, service);

		*out_service = service;
		*out_video_encoder = video_encoder;
		*out_audio_encoder = audio_encoder;

		output
	}
}

pub(crate) fn split_rtmp_url(rtmp_url: &str) -> (String, String) {
	if let Some(idx) = rtmp_url.rfind('/') {
		let (server, key) = rtmp_url.split_at(idx);
		let key = key.trim_start_matches('/');
		if server.is_empty() {
			(rtmp_url.to_string(), String::new())
		} else {
			(server.to_string(), key.to_string())
		}
	} else {
		(rtmp_url.to_string(), String::new())
	}
}
