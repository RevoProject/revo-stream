pub(crate) fn start(
	state: tauri::State<crate::ObsState>,
	root_dir: Option<String>,
) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if runtime.initialized {
		return Ok("OBS already initialized".to_string());
	}

	let has_display = std::env::var_os("DISPLAY").is_some()
		|| std::env::var_os("WAYLAND_DISPLAY").is_some();
	if !has_display {
		return Err("No display found (DISPLAY/WAYLAND_DISPLAY). Run under a desktop session or set DISPLAY.".to_string());
	}

	if std::env::var_os("REVO_FORCE_SOFTWARE_GL").is_some() {
		std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
		std::env::set_var("MESA_LOADER_DRIVER_OVERRIDE", "llvmpipe");
	}

	if let Ok(settings) = crate::settings::core::settings_get() {
		let pref = settings
			.encoder_preference
			.unwrap_or_else(|| "hardware".to_string())
			.trim()
			.to_lowercase();
		runtime.video_encoder_preference = match pref.as_str() {
			"software" | "x264" | "obs_x264" | "legacy" | "legacy_x264" => {
				crate::VideoEncoderPreference::Software
			}
			_ => crate::VideoEncoderPreference::Hardware,
		};
	}

	let root = crate::settings::core::resolve_root_dir(root_dir)?;
	let data_dir = if root.join("data/share/obs/libobs/default.effect").exists() {
		root.join("data")
	} else {
		root.clone()
	};
	let core_dir = root.join("core");
	let data_ffmpeg_bin = data_dir.join("bin");
	let core_ffmpeg_bin = core_dir.join("bin");
	let ffmpeg_bin = if data_ffmpeg_bin.is_dir() {
		data_ffmpeg_bin.clone()
	} else {
		core_ffmpeg_bin.clone()
	};
	let conf_dir = data_dir.join("conf");
	let data_plugins_dir = data_dir.join("lib/obs-plugins");
	let data_plugin_data_dir = data_dir.join("share/obs/obs-plugins/%module%");
	let core_plugins_dir = core_dir.join("lib/obs-plugins");
	let core_plugin_data_dir = core_dir.join("share/obs/obs-plugins/%module%");
	let bundled_plugins_dir = if core_plugins_dir.is_dir() {
		core_plugins_dir.clone()
	} else {
		data_plugins_dir.clone()
	};
	let bundled_plugin_data_dir = if core_plugins_dir.is_dir() {
		core_plugin_data_dir.clone()
	} else {
		data_plugin_data_dir.clone()
	};
	let runtime_plugins_dir = crate::settings::core::runtime_plugins_dir()
		.unwrap_or_else(|_| std::path::PathBuf::from("data/plugins"));
	let legacy_runtime_plugins_dir = crate::settings::core::legacy_runtime_plugins_dir().unwrap_or_else(|_| {
		if cfg!(debug_assertions) {
			std::path::PathBuf::from("src-tauri/data/plugins")
		} else {
			std::path::PathBuf::from("data/plugins")
		}
	});
	let runtime_plugin_data_dir = if runtime_plugins_dir.join("data").is_dir() {
		runtime_plugins_dir.join("data").join("%module%")
	} else {
		runtime_plugins_dir.join("%module%")
	};
	let legacy_runtime_plugin_data_dir = if legacy_runtime_plugins_dir.join("data").is_dir() {
		legacy_runtime_plugins_dir.join("data").join("%module%")
	} else {
		legacy_runtime_plugins_dir.join("%module%")
	};

	let runtime_plugins = crate::settings::plugins_profiles::list_runtime_plugins_internal().unwrap_or_default();
	eprintln!(
		"[plugins] obs_start runtime_plugins_dir={} discovered_count={}",
		runtime_plugins_dir.to_string_lossy(),
		runtime_plugins.len()
	);
	let active_plugin_profile = crate::settings::plugins_profiles::plugin_profile_active_name_internal()
		.unwrap_or_else(|_| "default".to_string());
	let enabled_plugin_modules: std::collections::HashSet<String> =
		crate::settings::plugins_profiles::plugin_profile_get_internal(&active_plugin_profile)
			.ok()
			.map(|p| p.enabled_modules.into_iter().collect::<std::collections::HashSet<_>>())
			.filter(|set| !set.is_empty())
			.unwrap_or_else(|| runtime_plugins.iter().map(|p| p.module_name.clone()).collect());
	eprintln!(
		"[plugins] obs_start active_profile={} enabled_count={}",
		active_plugin_profile,
		enabled_plugin_modules.len()
	);
	let core_data_dir = core_dir.join("share/obs");
	let data_share_dir = data_dir.join("share/obs");

	ensure_libobs_effects(&data_dir, &core_dir)?;
	let _ = std::env::set_current_dir(&data_dir);

	let locale = std::ffi::CString::new("en-US").map_err(|_| "locale string".to_string())?;
	let conf = std::ffi::CString::new(conf_dir.to_string_lossy().as_bytes())
		.map_err(|_| "conf path".to_string())?;

	let data_path = format!(
		"{}:{}:{}",
		data_share_dir.to_string_lossy(),
		data_share_dir.join("libobs").to_string_lossy(),
		core_data_dir.to_string_lossy()
	);
	std::env::set_var("OBS_DATA_PATH", data_path);
	let mut plugin_path_parts = vec![
		bundled_plugins_dir.to_string_lossy().to_string(),
		runtime_plugins_dir.to_string_lossy().to_string(),
	];
	if legacy_runtime_plugins_dir != runtime_plugins_dir {
		plugin_path_parts.push(legacy_runtime_plugins_dir.to_string_lossy().to_string());
	}
	let plugin_path = plugin_path_parts.join(":");
	std::env::set_var("OBS_PLUGIN_PATH", plugin_path);
	if ffmpeg_bin.exists() {
		let mut path_parts: Vec<String> = Vec::new();
		path_parts.push(ffmpeg_bin.to_string_lossy().to_string());
		if core_ffmpeg_bin != ffmpeg_bin && core_ffmpeg_bin.exists() {
			path_parts.push(core_ffmpeg_bin.to_string_lossy().to_string());
		}
		if data_ffmpeg_bin != ffmpeg_bin && data_ffmpeg_bin.exists() {
			path_parts.push(data_ffmpeg_bin.to_string_lossy().to_string());
		}
		if let Ok(old_path) = std::env::var("PATH") {
			path_parts.push(old_path);
			let new_path = path_parts.join(":");
			std::env::set_var("PATH", new_path);
		}
		std::env::set_var("OBS_FFMPEG_PATH", &ffmpeg_bin);
		let mux = ffmpeg_bin.join("obs-ffmpeg-mux");
		if mux.is_file() {
			std::env::set_var("OBS_FFMPEG_MUX_PATH", mux);
		}
	}

	unsafe {
		revo_lib::obs::base_set_log_handler(Some(crate::logging::obs_logger::obs_log_handler), std::ptr::null_mut());

		if !revo_lib::obs::obs_startup(locale.as_ptr(), conf.as_ptr(), std::ptr::null_mut()) {
			return Err("obs_startup failed".to_string());
		}

		let data_share_c = std::ffi::CString::new(data_share_dir.to_string_lossy().as_bytes())
			.map_err(|_| "data share path".to_string())?;
		revo_lib::obs::obs_add_data_path(data_share_c.as_ptr());

		let data_share_libobs =
			std::ffi::CString::new(data_share_dir.join("libobs").to_string_lossy().as_bytes())
				.map_err(|_| "data share libobs path".to_string())?;
		revo_lib::obs::obs_add_data_path(data_share_libobs.as_ptr());

		let core_data_c = std::ffi::CString::new(core_data_dir.to_string_lossy().as_bytes())
			.map_err(|_| "core data path".to_string())?;
		revo_lib::obs::obs_add_data_path(core_data_c.as_ptr());

		let libobs_core =
			std::ffi::CString::new(core_data_dir.join("libobs").to_string_lossy().as_bytes())
				.map_err(|_| "libobs core path".to_string())?;
		revo_lib::obs::obs_add_data_path(libobs_core.as_ptr());

		let libobs_data =
			std::ffi::CString::new(data_share_dir.join("libobs").to_string_lossy().as_bytes())
				.map_err(|_| "libobs data path".to_string())?;
		revo_lib::obs::obs_add_data_path(libobs_data.as_ptr());

		let bin_dir_c = std::ffi::CString::new(bundled_plugins_dir.to_string_lossy().as_bytes())
			.map_err(|_| "plugins path".to_string())?;
		let data_dir_c = std::ffi::CString::new(bundled_plugin_data_dir.to_string_lossy().as_bytes())
			.map_err(|_| "plugin data path".to_string())?;
		revo_lib::obs::obs_add_module_path(bin_dir_c.as_ptr(), data_dir_c.as_ptr());

		let runtime_bin_dir_c =
			std::ffi::CString::new(runtime_plugins_dir.to_string_lossy().as_bytes())
				.map_err(|_| "runtime plugins path".to_string())?;
		let runtime_data_dir_c =
			std::ffi::CString::new(runtime_plugin_data_dir.to_string_lossy().as_bytes())
				.map_err(|_| "runtime plugin data path".to_string())?;
		revo_lib::obs::obs_add_module_path(runtime_bin_dir_c.as_ptr(), runtime_data_dir_c.as_ptr());

		if legacy_runtime_plugins_dir != runtime_plugins_dir {
			let legacy_runtime_bin_dir_c =
				std::ffi::CString::new(legacy_runtime_plugins_dir.to_string_lossy().as_bytes())
					.map_err(|_| "legacy runtime plugins path".to_string())?;
			let legacy_runtime_data_dir_c = std::ffi::CString::new(
				legacy_runtime_plugin_data_dir.to_string_lossy().as_bytes(),
			)
			.map_err(|_| "legacy runtime plugin data path".to_string())?;
			revo_lib::obs::obs_add_module_path(
				legacy_runtime_bin_dir_c.as_ptr(),
				legacy_runtime_data_dir_c.as_ptr(),
			);
		}

		let disabled = [
			"decklink",
			"obs-websocket",
			"linux-capture",
			"linux-pipewire",
			"frontend-tools",
		];
		for name in disabled {
			let module = std::ffi::CString::new(name).map_err(|_| "module name".to_string())?;
			revo_lib::obs::obs_add_disabled_module(module.as_ptr());
		}

		for plugin in &runtime_plugins {
			if enabled_plugin_modules.contains(&plugin.module_name) {
				continue;
			}

			let module_exact = std::ffi::CString::new(plugin.module_name.as_str())
				.map_err(|_| "plugin module name".to_string())?;
			revo_lib::obs::obs_add_disabled_module(module_exact.as_ptr());

			if let Some(stripped) = plugin.module_name.strip_prefix("lib") {
				if !stripped.is_empty() {
					let module_stripped = std::ffi::CString::new(stripped)
						.map_err(|_| "plugin module name".to_string())?;
					revo_lib::obs::obs_add_disabled_module(module_stripped.as_ptr());
				}
			}
		}

		revo_lib::obs::obs_load_all_modules();
		revo_lib::obs::obs_post_load_modules();
	}

	if !reset_video_audio() {
		return Err("obs_reset_video/obs_reset_audio failed".to_string());
	}

	ensure_scene(&mut runtime, &root)?;

	runtime.initialized = true;
	revo_lib::runtime::set_initialized(true);
	Ok("OBS initialized".to_string())
}

pub(crate) fn shutdown(state: tauri::State<crate::ObsState>) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Ok("OBS already shut down".to_string());
	}
	super::helpers::stop_recording_internal(&mut runtime);
	super::helpers::stop_streaming_internal(&mut runtime);
	cleanup_scene(&mut runtime);
	unsafe {
		revo_lib::obs::obs_shutdown();
	}
	runtime.initialized = false;
	revo_lib::runtime::set_initialized(false);
	Ok("OBS shut down".to_string())
}

pub(crate) fn ensure_scene(
	runtime: &mut crate::ObsRuntime,
	root: &std::path::PathBuf,
) -> Result<(), String> {
	if !runtime.scenes.is_empty() {
		return Ok(());
	}

	let scene_name = std::ffi::CString::new("revo_scene").map_err(|_| "scene name".to_string())?;
	let scene = unsafe { revo_lib::obs::obs_scene_create(scene_name.as_ptr()) };
	if scene.is_null() {
		return Err("failed to create scene".to_string());
	}

	let mut accent_item: *mut revo_lib::obs::obs_scene_item = std::ptr::null_mut();
	let mut text_item: *mut revo_lib::obs::obs_scene_item = std::ptr::null_mut();

	unsafe {
		if cfg!(debug_assertions) {
			let accent = create_color_source("revo_accent", 0xFF1E90FF, 420, 220);
			let text = create_text_source("RevoStream", 32);

			accent_item = revo_lib::obs::obs_scene_add(scene, accent);
			if !accent_item.is_null() {
				let pos = revo_lib::obs::vec2 {
					__bindgen_anon_1: revo_lib::obs::vec2__bindgen_ty_1 {
						__bindgen_anon_1: revo_lib::obs::vec2__bindgen_ty_1__bindgen_ty_1 {
							x: 60.0,
							y: 60.0,
						},
					},
				};
				revo_lib::obs::obs_sceneitem_set_pos(accent_item, &pos);
				revo_lib::obs::obs_sceneitem_set_visible(accent_item, true);
			}
			text_item = revo_lib::obs::obs_scene_add(scene, text);
			if !text_item.is_null() {
				let pos = revo_lib::obs::vec2 {
					__bindgen_anon_1: revo_lib::obs::vec2__bindgen_ty_1 {
						__bindgen_anon_1: revo_lib::obs::vec2__bindgen_ty_1__bindgen_ty_1 {
							x: 640.0,
							y: 360.0,
						},
					},
				};
				revo_lib::obs::obs_sceneitem_set_pos(text_item, &pos);
				revo_lib::obs::obs_sceneitem_set_alignment(
					text_item,
					revo_lib::obs::OBS_ALIGN_CENTER,
				);
				revo_lib::obs::obs_sceneitem_set_visible(text_item, true);
			}

			if !accent.is_null() {
				revo_lib::obs::obs_source_release(accent);
			}
			if !text.is_null() {
				revo_lib::obs::obs_source_release(text);
			}
		}

		let scene_source = revo_lib::obs::obs_scene_get_source(scene);
		revo_lib::obs::obs_set_output_source(0, scene_source);
		revo_lib::obs::obs_source_inc_showing(scene_source);

		let mut state = crate::SceneState::new("revo_scene".to_string(), scene, scene_source);
		state.item_accent = accent_item;
		state.item_text = text_item;
		runtime.scenes.insert(state.name.clone(), state);
		if !runtime.scene_order.iter().any(|n| n == "revo_scene") {
			runtime.scene_order.push("revo_scene".to_string());
		}
		runtime.current_scene = Some("revo_scene".to_string());
	}

	let _ = root;
	Ok(())
}

pub(crate) fn cleanup_scene(runtime: &mut crate::ObsRuntime) {
	unsafe {
		if !runtime.preview_texrender.is_null() {
			revo_lib::obs::gs_texrender_destroy(runtime.preview_texrender);
		}
		if !runtime.preview_view.is_null() {
			revo_lib::obs::obs_view_destroy(runtime.preview_view);
		}
		revo_lib::obs::obs_set_output_source(0, std::ptr::null_mut());
		for state in runtime.scenes.values() {
			if !state.scene_source.is_null() {
				revo_lib::obs::obs_source_dec_showing(state.scene_source);
			}
			if !state.scene.is_null() {
				revo_lib::obs::obs_scene_enum_items(
					state.scene,
					Some(crate::scenes::scene_items::remove_scene_item_cb),
					std::ptr::null_mut(),
				);
				revo_lib::obs::obs_scene_release(state.scene);
			}
		}
	}
	runtime.scenes.clear();
	runtime.current_scene = None;
	runtime.locked_scenes.clear();
	runtime.scene_order.clear();
	runtime.preview_view = std::ptr::null_mut();
	runtime.preview_texrender = std::ptr::null_mut();
	runtime.last_record_path = None;
}

pub(crate) fn reset_video_audio() -> bool {
	unsafe {
		let graphics_module = std::ffi::CString::new("libobs-opengl").unwrap();
		let mut ovi: revo_lib::obs::obs_video_info = std::mem::zeroed();
		ovi.graphics_module = graphics_module.as_ptr();
		ovi.fps_num = 30;
		ovi.fps_den = 1;
		ovi.base_width = 1280;
		ovi.base_height = 720;
		ovi.output_width = 1280;
		ovi.output_height = 720;
		ovi.output_format = revo_lib::obs::video_format_VIDEO_FORMAT_NV12;
		ovi.adapter = 0;
		ovi.gpu_conversion = true;
		ovi.colorspace = revo_lib::obs::video_colorspace_VIDEO_CS_709;
		ovi.range = revo_lib::obs::video_range_type_VIDEO_RANGE_PARTIAL;
		ovi.scale_type = revo_lib::obs::obs_scale_type_OBS_SCALE_BICUBIC;

		let video_ok = revo_lib::obs::obs_reset_video(&mut ovi as *mut _)
			== revo_lib::obs::OBS_VIDEO_SUCCESS as i32;

		let mut oai: revo_lib::obs::obs_audio_info = std::mem::zeroed();
		oai.samples_per_sec = 48_000;
		oai.speakers = revo_lib::obs::speaker_layout_SPEAKERS_STEREO;
		let audio_ok = revo_lib::obs::obs_reset_audio(&oai as *const _);

		video_ok && audio_ok
	}
}

fn ensure_libobs_effects(
	data_dir: &std::path::PathBuf,
	core_dir: &std::path::PathBuf,
) -> Result<(), String> {
	let dst = data_dir.join("share/obs/libobs");
	if dst.join("default.effect").exists() {
		return Ok(());
	}
	let src = core_dir.join("share/obs/libobs");
	if !src.exists() || !src.join("default.effect").exists() {
		return Err(
			"Missing libobs shader effects: core/share/obs/libobs/default.effect not found"
				.to_string(),
		);
	}
	std::fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
	copy_dir_recursive(&src, &dst)
}

fn copy_dir_recursive(src: &std::path::PathBuf, dst: &std::path::PathBuf) -> Result<(), String> {
	for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
		let entry = entry.map_err(|e| e.to_string())?;
		let path = entry.path();
		let target = dst.join(entry.file_name());
		let meta = entry.metadata().map_err(|e| e.to_string())?;
		if meta.is_dir() {
			std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
			copy_dir_recursive(&path, &target)?;
		} else if meta.is_file() {
			std::fs::copy(&path, &target).map_err(|e| e.to_string())?;
		}
	}
	Ok(())
}

fn create_color_source(
	name: &str,
	abgr: u32,
	width: i64,
	height: i64,
) -> *mut revo_lib::obs::obs_source {
	unsafe {
		let settings = revo_lib::obs::obs_data_create();
		if !settings.is_null() {
			let key = std::ffi::CString::new("color").unwrap();
			revo_lib::obs::obs_data_set_int(settings, key.as_ptr(), abgr as i64);

			let width_key = std::ffi::CString::new("width").unwrap();
			revo_lib::obs::obs_data_set_int(settings, width_key.as_ptr(), width);
			let height_key = std::ffi::CString::new("height").unwrap();
			revo_lib::obs::obs_data_set_int(settings, height_key.as_ptr(), height);
		}
		let id = std::ffi::CString::new("color_source").unwrap();
		let name = std::ffi::CString::new(name).unwrap();
		let source = revo_lib::obs::obs_source_create(
			id.as_ptr(),
			name.as_ptr(),
			settings,
			std::ptr::null_mut(),
		);
		if !settings.is_null() {
			revo_lib::obs::obs_data_release(settings);
		}
		source
	}
}

fn create_text_source(text: &str, size: i64) -> *mut revo_lib::obs::obs_source {
	unsafe {
		let settings = revo_lib::obs::obs_data_create();
		if !settings.is_null() {
			let text_key = std::ffi::CString::new("text").unwrap();
			let text_val = std::ffi::CString::new(text).unwrap();
			revo_lib::obs::obs_data_set_string(settings, text_key.as_ptr(), text_val.as_ptr());

			let color_key = std::ffi::CString::new("color1").unwrap();
			revo_lib::obs::obs_data_set_int(settings, color_key.as_ptr(), 0xFFFFFFFFu32 as i64);

			let font_key = std::ffi::CString::new("font").unwrap();
			let font_settings = revo_lib::obs::obs_data_create();
			if !font_settings.is_null() {
				let face_key = std::ffi::CString::new("face").unwrap();
				let face_val = std::ffi::CString::new("DejaVu Sans").unwrap();
				revo_lib::obs::obs_data_set_string(
					font_settings,
					face_key.as_ptr(),
					face_val.as_ptr(),
				);

				let size_key = std::ffi::CString::new("size").unwrap();
				revo_lib::obs::obs_data_set_int(font_settings, size_key.as_ptr(), size);
				revo_lib::obs::obs_data_set_obj(settings, font_key.as_ptr(), font_settings);
				revo_lib::obs::obs_data_release(font_settings);
			}
		}
		let id = std::ffi::CString::new("text_ft2_source").unwrap();
		let name = std::ffi::CString::new("revo_text").unwrap();
		let source = revo_lib::obs::obs_source_create(
			id.as_ptr(),
			name.as_ptr(),
			settings,
			std::ptr::null_mut(),
		);
		if !settings.is_null() {
			revo_lib::obs::obs_data_release(settings);
		}
		source
	}
}
