pub(crate) fn defaults_obs_settings_select_options() -> crate::ObsSettingsSelectOptions {
	crate::ObsSettingsSelectOptions {
		streaming_audio_encoders: vec![
			crate::SelectOption {
				value: "aac".to_string(),
				label: "AAC".to_string(),
			},
			crate::SelectOption {
				value: "opus".to_string(),
				label: "Opus".to_string(),
			},
		],
		streaming_video_encoders: vec![
			crate::SelectOption {
				value: "x264".to_string(),
				label: "x264".to_string(),
			},
			crate::SelectOption {
				value: "h264_nvenc".to_string(),
				label: "NVIDIA NVENC".to_string(),
			},
			crate::SelectOption {
				value: "h264_vaapi".to_string(),
				label: "VAAPI".to_string(),
			},
		],
		recording_audio_encoders: vec![
			crate::SelectOption {
				value: "aac".to_string(),
				label: "AAC".to_string(),
			},
			crate::SelectOption {
				value: "opus".to_string(),
				label: "Opus".to_string(),
			},
		],
		recording_video_encoders: vec![
			crate::SelectOption {
				value: "x264".to_string(),
				label: "x264".to_string(),
			},
			crate::SelectOption {
				value: "h264_nvenc".to_string(),
				label: "NVIDIA NVENC".to_string(),
			},
			crate::SelectOption {
				value: "h264_vaapi".to_string(),
				label: "VAAPI".to_string(),
			},
		],
		recording_formats: vec![
			crate::SelectOption {
				value: "mkv".to_string(),
				label: "mkv".to_string(),
			},
			crate::SelectOption {
				value: "mp4".to_string(),
				label: "mp4".to_string(),
			},
			crate::SelectOption {
				value: "mov".to_string(),
				label: "mov".to_string(),
			},
			crate::SelectOption {
				value: "flv".to_string(),
				label: "flv".to_string(),
			},
			crate::SelectOption {
				value: "ts".to_string(),
				label: "ts".to_string(),
			},
			crate::SelectOption {
				value: "m3u8".to_string(),
				label: "m3u8".to_string(),
			},
			crate::SelectOption {
				value: "webm".to_string(),
				label: "webm".to_string(),
			},
			crate::SelectOption {
				value: "avi".to_string(),
				label: "avi".to_string(),
			},
		],
		audio_sample_rates: vec![
			crate::SelectOption {
				value: "44100".to_string(),
				label: "44.1 kHz".to_string(),
			},
			crate::SelectOption {
				value: "48000".to_string(),
				label: "48 kHz".to_string(),
			},
		],
		audio_channels: vec![
			crate::SelectOption {
				value: "mono".to_string(),
				label: "Mono".to_string(),
			},
			crate::SelectOption {
				value: "stereo".to_string(),
				label: "Stereo".to_string(),
			},
			crate::SelectOption {
				value: "2.1".to_string(),
				label: "2.1".to_string(),
			},
			crate::SelectOption {
				value: "4.1".to_string(),
				label: "4.1".to_string(),
			},
			crate::SelectOption {
				value: "5.1".to_string(),
				label: "5.1".to_string(),
			},
			crate::SelectOption {
				value: "7.1".to_string(),
				label: "7.1".to_string(),
			},
		],
		video_resolutions: vec![
			crate::SelectOption {
				value: "1280x720".to_string(),
				label: "1280x720".to_string(),
			},
			crate::SelectOption {
				value: "1600x900".to_string(),
				label: "1600x900".to_string(),
			},
			crate::SelectOption {
				value: "1920x1080".to_string(),
				label: "1920x1080".to_string(),
			},
			crate::SelectOption {
				value: "2560x1440".to_string(),
				label: "2560x1440".to_string(),
			},
			crate::SelectOption {
				value: "3840x2160".to_string(),
				label: "3840x2160".to_string(),
			},
		],
		video_downscale_filters: vec![
			crate::SelectOption {
				value: "bilinear".to_string(),
				label: "Bilinear (Fastest, soft)".to_string(),
			},
			crate::SelectOption {
				value: "bicubic".to_string(),
				label: "Bicubic (Sharpened scaling, 16 samples)".to_string(),
			},
			crate::SelectOption {
				value: "lanczos".to_string(),
				label: "Lanczos (Sharpened scaling, 32 samples)".to_string(),
			},
		],
		video_common_fps: vec![
			crate::SelectOption {
				value: "24".to_string(),
				label: "24".to_string(),
			},
			crate::SelectOption {
				value: "25".to_string(),
				label: "25".to_string(),
			},
			crate::SelectOption {
				value: "30".to_string(),
				label: "30".to_string(),
			},
			crate::SelectOption {
				value: "48".to_string(),
				label: "48".to_string(),
			},
			crate::SelectOption {
				value: "50".to_string(),
				label: "50".to_string(),
			},
			crate::SelectOption {
				value: "60".to_string(),
				label: "60".to_string(),
			},
		],
	}
}

pub(crate) fn collect_encoder_options(
	encoder_type: revo_lib::obs::obs_encoder_type,
) -> Vec<crate::SelectOption> {
	let mut out: Vec<crate::SelectOption> = Vec::new();
	let mut idx: usize = 0;

	unsafe {
		loop {
			let mut id_ptr: *const std::os::raw::c_char = std::ptr::null();
			let ok = revo_lib::obs::obs_enum_encoder_types(
				idx,
				&mut id_ptr as *mut *const std::os::raw::c_char,
			);
			if !ok {
				break;
			}

			if !id_ptr.is_null() && revo_lib::obs::obs_get_encoder_type(id_ptr) == encoder_type {
				let value = std::ffi::CStr::from_ptr(id_ptr).to_string_lossy().to_string();
				if !value.trim().is_empty() {
					let label_ptr = revo_lib::obs::obs_encoder_get_display_name(id_ptr);
					let label = if label_ptr.is_null() {
						value.clone()
					} else {
						std::ffi::CStr::from_ptr(label_ptr).to_string_lossy().to_string()
					};
					out.push(crate::SelectOption { value, label });
				}
			}

			idx += 1;
		}
	}

	out.sort_by(|a, b| a.label.cmp(&b.label).then(a.value.cmp(&b.value)));
	out.dedup_by(|a, b| a.value == b.value);
	out
}

pub(crate) fn collect_output_format_options(output_id: &str) -> Vec<crate::SelectOption> {
	let mut out: Vec<crate::SelectOption> = Vec::new();
	let Ok(output_id_c) = std::ffi::CString::new(output_id) else {
		return out;
	};

	unsafe {
		let props = revo_lib::obs::obs_get_output_properties(output_id_c.as_ptr());
		if props.is_null() {
			return out;
		}

		let mut prop = revo_lib::obs::obs_properties_first(props);
		while !prop.is_null() {
			let key_ptr = revo_lib::obs::obs_property_name(prop);
			let key = if key_ptr.is_null() {
				String::new()
			} else {
				std::ffi::CStr::from_ptr(key_ptr).to_string_lossy().to_string()
			};

			let is_format_key = matches!(key.as_str(), "format" | "format_name" | "muxer_format");
			if is_format_key
				&& revo_lib::obs::obs_property_get_type(prop)
					== revo_lib::obs::obs_property_type_OBS_PROPERTY_LIST
			{
				let format = revo_lib::obs::obs_property_list_format(prop);
				let count = revo_lib::obs::obs_property_list_item_count(prop);
				for i in 0..count {
					let label_ptr = revo_lib::obs::obs_property_list_item_name(prop, i);
					let label = if label_ptr.is_null() {
						String::new()
					} else {
						std::ffi::CStr::from_ptr(label_ptr).to_string_lossy().to_string()
					};

					let value = if format
						== revo_lib::obs::obs_combo_format_OBS_COMBO_FORMAT_STRING
					{
						let value_ptr = revo_lib::obs::obs_property_list_item_string(prop, i);
						if value_ptr.is_null() {
							String::new()
						} else {
							std::ffi::CStr::from_ptr(value_ptr).to_string_lossy().to_string()
						}
					} else if format
						== revo_lib::obs::obs_combo_format_OBS_COMBO_FORMAT_FLOAT
					{
						revo_lib::obs::obs_property_list_item_float(prop, i).to_string()
					} else if format
						== revo_lib::obs::obs_combo_format_OBS_COMBO_FORMAT_BOOL
					{
						if revo_lib::obs::obs_property_list_item_bool(prop, i) {
							"true".to_string()
						} else {
							"false".to_string()
						}
					} else {
						revo_lib::obs::obs_property_list_item_int(prop, i).to_string()
					};

					if !value.trim().is_empty() {
						out.push(crate::SelectOption {
							value: value.trim().to_string(),
							label: if label.trim().is_empty() {
								value.trim().to_string()
							} else {
								label.trim().to_string()
							},
						});
					}
				}
			}

			if !revo_lib::obs::obs_property_next(&mut prop as *mut _) {
				break;
			}
		}

		revo_lib::obs::obs_properties_destroy(props);
	}

	out.sort_by(|a, b| a.label.cmp(&b.label).then(a.value.cmp(&b.value)));
	out.dedup_by(|a, b| a.value == b.value);
	out
}

pub(crate) fn get_settings_select_options(
	state: tauri::State<crate::ObsState>,
) -> Result<crate::ObsSettingsSelectOptions, String> {
	let defaults = defaults_obs_settings_select_options();
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Ok(defaults);
	}
	drop(runtime);

	let audio_encoders = collect_encoder_options(revo_lib::obs::obs_encoder_type_OBS_ENCODER_AUDIO);
	let video_encoders = collect_encoder_options(revo_lib::obs::obs_encoder_type_OBS_ENCODER_VIDEO);
	let mut recording_formats = collect_output_format_options("ffmpeg_muxer");
	if recording_formats.is_empty() {
		recording_formats = collect_output_format_options("ffmpeg_output");
	}

	Ok(crate::ObsSettingsSelectOptions {
		streaming_audio_encoders: if audio_encoders.is_empty() {
			defaults.streaming_audio_encoders
		} else {
			audio_encoders.clone()
		},
		streaming_video_encoders: if video_encoders.is_empty() {
			defaults.streaming_video_encoders
		} else {
			video_encoders.clone()
		},
		recording_audio_encoders: if audio_encoders.is_empty() {
			defaults.recording_audio_encoders
		} else {
			audio_encoders
		},
		recording_video_encoders: if video_encoders.is_empty() {
			defaults.recording_video_encoders
		} else {
			video_encoders
		},
		recording_formats: if recording_formats.is_empty() {
			defaults.recording_formats
		} else {
			recording_formats
		},
		audio_sample_rates: defaults.audio_sample_rates,
		audio_channels: defaults.audio_channels,
		video_resolutions: defaults.video_resolutions,
		video_downscale_filters: defaults.video_downscale_filters,
		video_common_fps: defaults.video_common_fps,
	})
}
