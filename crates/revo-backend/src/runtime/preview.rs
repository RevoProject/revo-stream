use base64::Engine as _;

#[cfg(not(target_os = "windows"))]
unsafe fn reset_texrender(texrender: *mut revo_lib::obs::gs_texrender_t) {
    extern "C" {
        fn gs_texrender_reset(texrender: *mut revo_lib::obs::gs_texrender_t);
    }
    gs_texrender_reset(texrender);
}

#[cfg(target_os = "windows")]
unsafe fn reset_texrender(texrender: *mut revo_lib::obs::gs_texrender_t) {
    revo_lib::obs::gs_texrender_reset(texrender);
}

unsafe fn clear_current_render_target() {
    let transparent: revo_lib::obs::vec4 = std::mem::zeroed();
    revo_lib::obs::gs_clear(
        revo_lib::obs::GS_CLEAR_COLOR,
        &transparent as *const _,
        0.0,
        0,
    );
}

pub fn planner_init_temp_path() -> Result<std::path::PathBuf, String> {
    let dir = std::env::temp_dir().join("revostream");
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create planner temp dir: {e}"))?;
    Ok(dir.join("graphic_planner_init.json"))
}

/// Build the planner-init payload and stage it in the temp file. Runs before
/// the runtime lock is taken (mirrors the original command ordering).
pub fn build_planner_init(init: crate::PlannerInitInput) -> crate::PlannerInit {
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

    planner_init
}

pub fn set_graphic_planner_init(
    runtime: &mut crate::ObsRuntime,
    planner_init: crate::PlannerInit,
) -> Result<(), String> {
    runtime.planner_init = Some(planner_init);
    Ok(())
}

/// Freeze/unfreeze scene-level preview rendering (command body of
/// `obs_preview_set_freeze`; the command owns the runtime lock).
pub fn set_preview_freeze(
    runtime: &mut crate::ObsRuntime,
    freeze: bool,
    reason: Option<String>,
) -> Result<(), String> {
    runtime.preview_freeze = freeze;
    if freeze {
        runtime.preview_freeze_requests += 1;
        runtime.preview_freeze_since = Some(std::time::Instant::now());
        runtime.preview_freeze_reason = Some(
            reason
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "unspecified".to_string()),
        );
    } else {
        runtime.preview_freeze_reason = None;
        runtime.preview_freeze_since = None;
    }
    Ok(())
}

pub fn get_graphic_planner_init(runtime: &crate::ObsRuntime) -> Result<crate::PlannerInit, String> {
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
        sources: crate::collect_sources(runtime),
        scene_resolution,
        selected_source_id: None,
        scene_name: runtime.current_scene.clone(),
    })
}

pub fn get_current_scene_resolution(runtime: &crate::ObsRuntime) -> Result<String, String> {
    Ok(crate::live_scene_resolution().unwrap_or_else(|| {
        if runtime.scene_resolution.trim().is_empty() {
            "1920x1080".to_string()
        } else {
            runtime.scene_resolution.clone()
        }
    }))
}

pub fn get_source_size(
    runtime: &crate::ObsRuntime,
    source_id: String,
) -> Result<(u32, u32), String> {
    let scene = crate::current_scene(runtime)?;
    let item = crate::resolve_scene_item(scene, source_id.trim())
        .ok_or_else(|| format!("unknown source id: {source_id}"))?;
    unsafe {
        let source = revo_lib::obs::obs_sceneitem_get_source(item);
        if source.is_null() {
            return Err("source not available".to_string());
        }
        let width = revo_lib::obs::obs_source_get_width(source);
        let height = revo_lib::obs::obs_source_get_height(source);
        if width == 0 || height == 0 {
            return Err("source has no base size yet".to_string());
        }
        Ok((width.max(1), height.max(1)))
    }
}

/// Outcome of the lock-scoped capture phase of `take_screenshot`.
pub enum ScreenshotCapture {
    /// Serve a retained frame immediately (frozen preview or stale fallback).
    Serve(String),
    /// Fresh capture: encode outside the lock, then publish.
    Encode(CapturedScreenshot),
}

/// State carried from the lock-scoped capture phase to the publish phase.
pub struct CapturedScreenshot {
    pub frame: CapturedPreviewFrame,
    pub capture_seq: u64,
    pub capture_generation: u64,
    pub capture_scene_output: bool,
}

/// Capture phase of `take_screenshot`: runs with the runtime lock held by
/// the command wrapper. `lock_wait_us` is the wrapper's measured lock wait,
/// recorded for scene-output requests as before the inversion.
pub fn capture_screenshot(
    runtime: &mut crate::ObsRuntime,
    width: Option<u32>,
    height: Option<u32>,
    source_id: Option<String>,
    lock_wait_us: u64,
) -> Result<ScreenshotCapture, String> {
    let capture_scene_output = source_id
        .as_ref()
        .map(|v| v.trim().is_empty())
        .unwrap_or(true);
    if capture_scene_output {
        crate::with_preview_stats(|stats| {
            stats.requests += 1;
            stats.lock_wait_us_max = stats.lock_wait_us_max.max(lock_wait_us);
        });
        crate::logging::trace::step("preview-loop", "request:enter");
    }

    // Preview freeze (Add Source workflow): serve the retained last good
    // frame without touching the graphics context, so source creation does
    // not contend with preview rendering. Only scene captures are frozen —
    // single-source screenshots (edit modal) keep rendering.
    if capture_scene_output && runtime.preview_freeze {
        if runtime.last_preview_frame_generation == runtime.scene_generation {
            if let Some(frame) = runtime.last_preview_frame.clone() {
                crate::with_preview_stats(|stats| stats.frozen_returns += 1);
                crate::logging::trace::step("preview-loop", "request:served-frozen");
                return Ok(ScreenshotCapture::Serve(frame));
            }
        }
    }

    let render_source = if let Some(raw_id) = source_id {
        let source_key = raw_id.trim().to_string();
        if source_key.is_empty() {
            let scene_source = match crate::current_scene(runtime) {
                Ok(scene) => scene.scene_source,
                Err(_) => return Err("no active scene".to_string()),
            };
            if scene_source.is_null() {
                return Err("scene source unavailable".to_string());
            }
            scene_source
        } else {
            // First try to resolve a scene with the requested name.
            if let Some(scene_state) = runtime.scenes.get(&source_key) {
                if scene_state.scene_source.is_null() {
                    return Err("scene source unavailable".to_string());
                }
                scene_state.scene_source
            } else {
                // Otherwise resolve it as an item in the current scene.
                let scene = crate::current_scene(runtime)?;
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
        }
    } else {
        let scene_source = match crate::current_scene(runtime) {
            Ok(scene) => scene.scene_source,
            Err(_) => return Err("no active scene".to_string()),
        };
        if scene_source.is_null() {
            return Err("scene source unavailable".to_string());
        }
        scene_source
    };
    ensure_preview_renderer(runtime, render_source)?;

    let (req_width, req_height) = if capture_scene_output {
        let live_resolution = crate::live_scene_resolution()
            .or_else(|| {
                if runtime.scene_resolution.trim().is_empty() {
                    None
                } else {
                    Some(runtime.scene_resolution.clone())
                }
            })
            .and_then(|raw| revo_core::planner::parse_resolution(&raw));

        let (canvas_w, canvas_h) = live_resolution.unwrap_or((640, 360));
        match (width, height) {
            // The frontend asks for its display box; render the canvas scaled
            // into that box while keeping the canvas aspect ratio, so the PNG
            // never exceeds the requested preview size (and never upscales
            // past the real canvas resolution).
            (Some(w), Some(h)) => {
                let box_w = w.max(160).min(3840) as f32;
                let box_h = h.max(90).min(2160) as f32;
                let scale = (box_w / canvas_w as f32)
                    .min(box_h / canvas_h as f32)
                    .min(1.0);
                let out_w = ((canvas_w as f32 * scale).round() as u32)
                    .max(160)
                    .min(7680);
                let out_h = ((canvas_h as f32 * scale).round() as u32).max(90).min(4320);
                (out_w, out_h)
            }
            // No size requested: keep the previous full-canvas behavior.
            _ => (canvas_w.max(160).min(7680), canvas_h.max(90).min(4320)),
        }
    } else {
        (
            width.unwrap_or(640).max(160).min(3840),
            height.unwrap_or(360).max(90).min(2160),
        )
    };
    let capture_seq = {
        runtime.preview_frame_seq = runtime.preview_frame_seq.wrapping_add(1);
        runtime.preview_frame_seq
    };
    let capture_generation = runtime.scene_generation;
    let capture_started = std::time::Instant::now();
    let captured = capture_view_pixels(runtime, req_width, req_height, render_source);
    let capture_us = capture_started.elapsed().as_micros() as u64;
    let captured = match captured {
        Ok(frame) => {
            crate::with_preview_stats(|stats| {
                stats.captures_ok += 1;
                stats.capture_us_max = stats.capture_us_max.max(capture_us);
                stats.last_error.clear();
            });
            crate::logging::trace::step("preview-loop", "request:captured");
            frame
        }
        Err(err) => {
            eprintln!("obs: preview render failed: {err}");
            runtime.last_preview_error_at = Some(std::time::Instant::now());
            crate::with_preview_stats(|stats| {
                stats.captures_err += 1;
                stats.capture_us_max = stats.capture_us_max.max(capture_us);
                stats.last_error = err.clone();
            });
            crate::logging::trace::step("preview-loop", "request:capture-failed");
            // Keep the last good frame instead of an error so the UI does not
            // flash or lose the preview on transient render failures.
            if !capture_scene_output
                || runtime.last_preview_frame_generation == runtime.scene_generation
            {
                if let Some(frame) = runtime.last_preview_frame.clone() {
                    crate::with_preview_stats(|stats| stats.stale_returns += 1);
                    crate::logging::trace::step("preview-loop", "request:served-stale");
                    return Ok(ScreenshotCapture::Serve(frame));
                }
            }
            return Err(err);
        }
    };

    Ok(ScreenshotCapture::Encode(CapturedScreenshot {
        frame: captured,
        capture_seq,
        capture_generation,
        capture_scene_output,
    }))
}

/// Encode phase of `take_screenshot`: PNG-encode + base64 the captured frame
/// while the runtime lock is released, updating encode stats exactly as
/// before the inversion.
pub fn encode_screenshot(frame: CapturedPreviewFrame) -> Result<String, String> {
    let encode_started = std::time::Instant::now();
    let encoded = encode_preview_frame(frame);
    let encode_us = encode_started.elapsed().as_micros() as u64;

    // Retain the frame for the freeze path and transient-failure fallback.
    crate::with_preview_stats(|stats| {
        stats.encode_us_max = stats.encode_us_max.max(encode_us);
        match &encoded {
            Ok(_) => {
                stats.encoded_ok += 1;
                stats.last_error.clear();
            }
            Err(error) => {
                stats.encoded_err += 1;
                stats.last_error = error.clone();
            }
        }
    });
    encoded
}

/// Publish phase of `take_screenshot`: runs with the runtime lock
/// re-acquired by the command wrapper. Returns whether the frame was
/// invalidated by a scene change (the command must then return an error).
pub fn publish_screenshot(
    runtime: &mut crate::ObsRuntime,
    capture_seq: u64,
    capture_generation: u64,
    capture_scene_output: bool,
    encoded: &Result<String, String>,
) -> bool {
    let mut generation_invalidated = false;
    if encoded.is_err() {
        runtime.last_preview_error_at = Some(std::time::Instant::now());
    }
    if let Ok(frame) = encoded {
        if capture_scene_output && capture_generation != runtime.scene_generation {
            generation_invalidated = true;
            crate::with_preview_stats(|stats| stats.stale_discards += 1);
            crate::logging::trace::step("preview-loop", "request:generation-discarded");
        } else if capture_seq >= runtime.last_preview_frame_seq {
            runtime.last_preview_frame = Some(frame.clone());
            runtime.last_preview_frame_generation = capture_generation;
            runtime.last_preview_frame_at = Some(std::time::Instant::now());
            runtime.last_preview_frame_seq = capture_seq;
            crate::logging::trace::step("preview-loop", "request:stored");
        } else {
            crate::with_preview_stats(|stats| stats.stale_discards += 1);
            crate::logging::trace::step("preview-loop", "request:stale-discarded");
        }
    }
    generation_invalidated
}

/// Capture phase of `take_source_thumbnail`: runs with the runtime lock held
/// by the command wrapper; the frame is encoded after the lock is released.
pub fn capture_source_thumbnail(
    runtime: &mut crate::ObsRuntime,
    source_name: &str,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<CapturedPreviewFrame, String> {
    if !runtime.initialized || runtime.preview_view.is_null() {
        return Err("OBS not initialized".to_string());
    }
    let name = std::ffi::CString::new(source_name.trim())
        .map_err(|_| "invalid source name".to_string())?;
    unsafe {
        let source = revo_lib::obs::obs_get_source_by_name(name.as_ptr());
        if source.is_null() {
            return Err(format!("existing source not found: {source_name}"));
        }
        if revo_lib::obs::obs_source_get_output_flags(source) & revo_lib::obs::OBS_SOURCE_VIDEO == 0
        {
            revo_lib::obs::obs_source_release(source);
            return Err("source has no video output".to_string());
        }
        let box_w = width.unwrap_or(320).clamp(160, 1280);
        let box_h = height.unwrap_or(180).clamp(90, 720);
        let base_w = revo_lib::obs::obs_source_get_width(source);
        let base_h = revo_lib::obs::obs_source_get_height(source);
        let (out_w, out_h) = if base_w > 0 && base_h > 0 {
            let scale = (box_w as f32 / base_w as f32)
                .min(box_h as f32 / base_h as f32)
                .min(1.0);
            (
                ((base_w as f32 * scale).round() as u32).max(1),
                ((base_h as f32 * scale).round() as u32).max(1),
            )
        } else {
            (box_w, box_h)
        };
        let captured = capture_view_pixels(runtime, out_w, out_h, source);
        revo_lib::obs::obs_source_release(source);
        captured
    }
}

/// PNG-encode + base64 a captured frame into a data URI. Pure function — no
/// OBS state, no locks. Consumes the frame.
pub fn encode_preview_frame(frame: CapturedPreviewFrame) -> Result<String, String> {
    let CapturedPreviewFrame {
        width,
        height,
        pixels,
    } = frame;
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

/// Raw RGBA frame read back from the preview texrender. GPU-side work and the
/// pixel copy run while the runtime lock is held; encoding happens later.
pub struct CapturedPreviewFrame {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[cfg(test)]
mod preview_tests {
    use base64::Engine as _;
    use image::GenericImageView;

    use super::{encode_preview_frame, CapturedPreviewFrame};

    #[test]
    fn encoded_preview_frame_is_a_png_data_uri() {
        // 2x2 solid red frame.
        let frame = CapturedPreviewFrame {
            width: 2,
            height: 2,
            pixels: vec![
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
            ],
        };
        let uri = encode_preview_frame(frame).expect("encoding must succeed");
        assert!(uri.starts_with("data:image/png;base64,"));

        // The payload must decode back to the same pixels.
        let raw = base64::engine::general_purpose::STANDARD
            .decode(uri.trim_start_matches("data:image/png;base64,"))
            .expect("valid base64");
        let decoded =
            image::load_from_memory_with_format(&raw, image::ImageFormat::Png).expect("valid png");
        assert_eq!(decoded.dimensions(), (2, 2));
        assert_eq!(
            decoded.to_rgba8().into_raw(),
            vec![255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255]
        );
    }

    #[test]
    fn zero_sized_frame_fails_gracefully() {
        let frame = CapturedPreviewFrame {
            width: 0,
            height: 0,
            pixels: vec![],
        };
        assert!(encode_preview_frame(frame).is_err());
    }
}

/// Destroy the preview texrender inside the graphics context and clear the
/// pointer. Safe to call when nothing is allocated. Must be called before
/// `obs_reset_video` — the reset frees graphics resources behind our back.
pub fn destroy_preview_renderer(runtime: &mut crate::ObsRuntime) {
    if !runtime.preview_texrender.is_null() {
        unsafe {
            revo_lib::obs::obs_enter_graphics();
            revo_lib::obs::gs_texrender_destroy(runtime.preview_texrender);
            revo_lib::obs::obs_leave_graphics();
        }
        runtime.preview_texrender = std::ptr::null_mut();
    }
}

pub fn ensure_preview_renderer(
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

pub fn capture_view_pixels(
    runtime: &mut crate::ObsRuntime,
    width: u32,
    height: u32,
    render_source: *mut revo_lib::obs::obs_source,
) -> Result<CapturedPreviewFrame, String> {
    let mut pixels: Vec<u8> = Vec::new();
    let mut width = width;
    let mut height = height;

    unsafe {
        revo_lib::obs::obs_view_set_source(runtime.preview_view, 0, render_source);
        crate::logging::trace::step("preview-capture", "obs_enter_graphics:enter");
        revo_lib::obs::obs_enter_graphics();

        // Never fall back to obs_get_main_texture() here: the main/output
        // texture is the composed output channel (base canvas, with transition),
        // not the preview view — using it makes the preview show a cropped/
        // zoomed frame after Record/Go Live start. Retry on a fresh texrender
        // instead; on failure keep the last good frame (handled by caller).
        let mut rendered = false;

        if !runtime.preview_texrender.is_null() {
            reset_texrender(runtime.preview_texrender);
        }
        if !runtime.preview_texrender.is_null()
            && revo_lib::obs::gs_texrender_begin(runtime.preview_texrender, width, height)
        {
            clear_current_render_target();
            revo_lib::obs::gs_ortho(0.0, width as f32, 0.0, height as f32, -100.0, 100.0);
            revo_lib::obs::obs_view_render(runtime.preview_view);
            revo_lib::obs::gs_texrender_end(runtime.preview_texrender);
            crate::logging::trace::step("preview-capture", "obs_view_render:done");
            rendered = true;
        }

        if !rendered {
            eprintln!(
                "[preview-capture] texrender_begin failed, retrying with a fresh texrender ({}x{})",
                width, height
            );
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
                clear_current_render_target();
                revo_lib::obs::gs_ortho(0.0, width as f32, 0.0, height as f32, -100.0, 100.0);
                revo_lib::obs::obs_view_render(runtime.preview_view);
                revo_lib::obs::gs_texrender_end(runtime.preview_texrender);
                crate::logging::trace::step("preview-capture", "obs_view_render:retry-done");
                rendered = true;
            }
        }

        if !rendered {
            revo_lib::obs::obs_leave_graphics();
            return Err("preview texrender failed".to_string());
        }

        let tex = revo_lib::obs::gs_texrender_get_texture(runtime.preview_texrender);
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

    Ok(CapturedPreviewFrame {
        width,
        height,
        pixels,
    })
}
