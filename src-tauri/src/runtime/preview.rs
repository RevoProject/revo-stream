use base64::Engine as _;

pub(crate) fn planner_init_temp_path() -> Result<std::path::PathBuf, String> {
	let dir = std::env::temp_dir().join("revostream");
	std::fs::create_dir_all(&dir)
		.map_err(|e| format!("failed to create planner temp dir: {e}"))?;
	Ok(dir.join("graphic_planner_init.json"))
}

pub(crate) fn set_graphic_planner_init(
	state: tauri::State<crate::ObsState>,
	init: crate::PlannerInitInput,
) -> Result<(), String> {
	let planner_init = crate::PlannerInit {
		sources: init.sources,
		scene_resolution: init.scene_resolution,
		selected_source_id: init.selected_source_id,
		scene_name: init.scene_name,
	};

	if let Ok(path) = planner_init_temp_path() {
		if let Ok(raw) = serde_json::to_string_pretty(&planner_init) {
			let _ = std::fs::write(path, raw);
		}
	}

	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	runtime.planner_init = Some(planner_init);
	Ok(())
}

pub(crate) fn get_graphic_planner_init(
	state: tauri::State<crate::ObsState>,
) -> Result<crate::PlannerInit, String> {
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if let Some(init) = runtime.planner_init.clone() {
		return Ok(init);
	}

	if let Ok(path) = planner_init_temp_path() {
		if path.is_file() {
			if let Ok(raw) = std::fs::read_to_string(path) {
				if let Ok(parsed) = serde_json::from_str::<crate::PlannerInit>(&raw) {
					return Ok(parsed);
				}
			}
		}
	}

	let scene_resolution = crate::live_scene_resolution().unwrap_or_else(|| {
		if runtime.scene_resolution.trim().is_empty() {
			"1920x1080".to_string()
		} else {
			runtime.scene_resolution.clone()
		}
	});
	Ok(crate::PlannerInit {
		sources: crate::collect_sources(&runtime),
		scene_resolution,
		selected_source_id: None,
		scene_name: runtime.current_scene.clone(),
	})
}

pub(crate) fn get_current_scene_resolution(
	state: tauri::State<crate::ObsState>,
) -> Result<String, String> {
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	Ok(crate::live_scene_resolution().unwrap_or_else(|| {
		if runtime.scene_resolution.trim().is_empty() {
			"1920x1080".to_string()
		} else {
			runtime.scene_resolution.clone()
		}
	}))
}

pub(crate) fn take_screenshot(
	state: tauri::State<crate::ObsState>,
	width: Option<u32>,
	height: Option<u32>,
	source_id: Option<String>,
) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}
	let capture_scene_output = source_id
		.as_ref()
		.map(|v| v.trim().is_empty())
		.unwrap_or(true);
	let render_source = if let Some(raw_id) = source_id {
		let source_key = raw_id.trim().to_string();
		if source_key.is_empty() {
			let scene_source = match crate::current_scene(&runtime) {
				Ok(scene) => scene.scene_source,
				Err(_) => return Err("no active scene".to_string()),
			};
			if scene_source.is_null() {
				return Err("scene source unavailable".to_string());
			}
			scene_source
		} else {
			let scene = crate::current_scene(&runtime)?;
			let item = crate::resolve_scene_item(scene, source_key.as_str())
				.ok_or_else(|| format!("unknown source id: {source_key}"))?;
			unsafe {
				let source = revo_lib::obs::obs_sceneitem_get_source(item);
				if source.is_null() {
					return Err("source not available".to_string());
				}
				source
			}
		}
	} else {
		let scene_source = match crate::current_scene(&runtime) {
			Ok(scene) => scene.scene_source,
			Err(_) => return Err("no active scene".to_string()),
		};
		if scene_source.is_null() {
			return Err("scene source unavailable".to_string());
		}
		scene_source
	};
	ensure_preview_renderer(&mut runtime, render_source)?;

	let parse_resolution = |raw: &str| -> Option<(u32, u32)> {
		let mut parts = raw.split('x');
		let w = parts.next()?.trim().parse::<u32>().ok()?;
		let h = parts.next()?.trim().parse::<u32>().ok()?;
		if parts.next().is_some() || w == 0 || h == 0 {
			return None;
		}
		Some((w, h))
	};

	let (req_width, req_height) = if capture_scene_output {
		let live_resolution = crate::live_scene_resolution()
			.or_else(|| {
				if runtime.scene_resolution.trim().is_empty() {
					None
				} else {
					Some(runtime.scene_resolution.clone())
				}
			})
			.and_then(|raw| parse_resolution(&raw));

		let (base_w, base_h) = live_resolution.unwrap_or((640, 360));
		(base_w.max(160).min(7680), base_h.max(90).min(4320))
	} else {
		(width.unwrap_or(640).max(160).min(3840), height.unwrap_or(360).max(90).min(2160))
	};
	capture_view_png(&mut runtime, req_width, req_height, render_source).map_err(|err| {
		eprintln!("obs: preview render failed: {err}");
		err
	})
}

pub(crate) fn ensure_preview_renderer(
	runtime: &mut crate::ObsRuntime,
	render_source: *mut revo_lib::obs::obs_source,
) -> Result<(), String> {
	if runtime.preview_view.is_null() {
		unsafe {
			runtime.preview_view = revo_lib::obs::obs_view_create();
		}
		if runtime.preview_view.is_null() {
			return Err("failed to create obs view".to_string());
		}
	}

	if runtime.preview_texrender.is_null() {
		unsafe {
			revo_lib::obs::obs_enter_graphics();
			runtime.preview_texrender = revo_lib::obs::gs_texrender_create(
				revo_lib::obs::gs_color_format_GS_RGBA,
				revo_lib::obs::gs_zstencil_format_GS_ZS_NONE,
			);
			revo_lib::obs::obs_leave_graphics();
		}
		if runtime.preview_texrender.is_null() {
			return Err("failed to create texrender".to_string());
		}
	}

	unsafe {
		revo_lib::obs::obs_view_set_source(runtime.preview_view, 0, render_source);
	}

	Ok(())
}

pub(crate) fn capture_view_png(
	runtime: &mut crate::ObsRuntime,
	width: u32,
	height: u32,
	render_source: *mut revo_lib::obs::obs_source,
) -> Result<String, String> {
	let mut pixels: Vec<u8> = Vec::new();
	let mut width = width;
	let mut height = height;

	unsafe {
		revo_lib::obs::obs_view_set_source(runtime.preview_view, 0, render_source);
		revo_lib::obs::obs_enter_graphics();

		let tex = if revo_lib::obs::gs_texrender_begin(runtime.preview_texrender, width, height) {
			revo_lib::obs::obs_view_render(runtime.preview_view);
			revo_lib::obs::gs_texrender_end(runtime.preview_texrender);
			revo_lib::obs::gs_texrender_get_texture(runtime.preview_texrender)
		} else {
			if !runtime.preview_texrender.is_null() {
				revo_lib::obs::gs_texrender_destroy(runtime.preview_texrender);
				runtime.preview_texrender = std::ptr::null_mut();
			}

			runtime.preview_texrender = revo_lib::obs::gs_texrender_create(
				revo_lib::obs::gs_color_format_GS_RGBA,
				revo_lib::obs::gs_zstencil_format_GS_ZS_NONE,
			);

			if !runtime.preview_texrender.is_null()
				&& revo_lib::obs::gs_texrender_begin(runtime.preview_texrender, width, height)
			{
				revo_lib::obs::obs_view_render(runtime.preview_view);
				revo_lib::obs::gs_texrender_end(runtime.preview_texrender);
				revo_lib::obs::gs_texrender_get_texture(runtime.preview_texrender)
			} else {
				eprintln!("obs: texrender begin failed, falling back to main texture");
				revo_lib::obs::obs_render_main_texture();
				revo_lib::obs::obs_get_main_texture()
			}
		};
		if tex.is_null() {
			revo_lib::obs::obs_leave_graphics();
			return Err("preview texture unavailable".to_string());
		}

		width = revo_lib::obs::gs_texture_get_width(tex);
		height = revo_lib::obs::gs_texture_get_height(tex);
		if width == 0 || height == 0 {
			revo_lib::obs::obs_leave_graphics();
			return Err("invalid preview texture size".to_string());
		}

		let format = revo_lib::obs::gs_texture_get_color_format(tex);
		let stagesurf = revo_lib::obs::gs_stagesurface_create(width, height, format);
		if stagesurf.is_null() {
			revo_lib::obs::obs_leave_graphics();
			return Err("failed to create stagesurface".to_string());
		}

		revo_lib::obs::gs_stage_texture(stagesurf, tex);
		revo_lib::obs::gs_flush();

		let mut data_ptr: *mut u8 = std::ptr::null_mut();
		let mut linesize: u32 = 0;
		if !revo_lib::obs::gs_stagesurface_map(stagesurf, &mut data_ptr, &mut linesize) {
			revo_lib::obs::gs_stagesurface_destroy(stagesurf);
			revo_lib::obs::obs_leave_graphics();
			return Err("failed to map stagesurface".to_string());
		}

		pixels.resize((width * height * 4) as usize, 0);
		let row_bytes = (width * 4) as usize;
		let mut dst_offset = 0usize;
		for y in 0..height {
			let src = std::slice::from_raw_parts(data_ptr.add((y * linesize) as usize), row_bytes);
			match format {
				revo_lib::obs::gs_color_format_GS_BGRA
				| revo_lib::obs::gs_color_format_GS_BGRA_UNORM => {
					for x in 0..width as usize {
						let b = src[x * 4];
						let g = src[x * 4 + 1];
						let r = src[x * 4 + 2];
						let a = src[x * 4 + 3];
						let base = dst_offset + x * 4;
						pixels[base] = r;
						pixels[base + 1] = g;
						pixels[base + 2] = b;
						pixels[base + 3] = a;
					}
				}
				revo_lib::obs::gs_color_format_GS_BGRX
				| revo_lib::obs::gs_color_format_GS_BGRX_UNORM => {
					for x in 0..width as usize {
						let b = src[x * 4];
						let g = src[x * 4 + 1];
						let r = src[x * 4 + 2];
						let base = dst_offset + x * 4;
						pixels[base] = r;
						pixels[base + 1] = g;
						pixels[base + 2] = b;
						pixels[base + 3] = 0xFF;
					}
				}
				revo_lib::obs::gs_color_format_GS_RGBA
				| revo_lib::obs::gs_color_format_GS_RGBA_UNORM => {
					pixels[dst_offset..dst_offset + row_bytes].copy_from_slice(src);
				}
				_ => {
					revo_lib::obs::gs_stagesurface_unmap(stagesurf);
					revo_lib::obs::gs_stagesurface_destroy(stagesurf);
					revo_lib::obs::obs_leave_graphics();
					return Err("unsupported texture format".to_string());
				}
			}
			dst_offset += row_bytes;
		}

		revo_lib::obs::gs_stagesurface_unmap(stagesurf);
		revo_lib::obs::gs_stagesurface_destroy(stagesurf);
		revo_lib::obs::obs_leave_graphics();
	}

	let image = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, pixels)
		.ok_or_else(|| "failed to build image buffer".to_string())?;

	let mut png_bytes = Vec::new();
	let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
	image::ImageEncoder::write_image(
		encoder,
		image.as_raw(),
		width,
		height,
		image::ColorType::Rgba8.into(),
	)
	.map_err(|e| format!("failed to encode screenshot: {e}"))?;

	let b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes);
	Ok(format!("data:image/png;base64,{b64}"))
}
