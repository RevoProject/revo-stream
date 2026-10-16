pub(crate) type SceneState = crate::SceneState;

pub(crate) fn current_scene(runtime: &crate::ObsRuntime) -> Result<&SceneState, String> {
	let current = runtime
		.current_scene
		.as_ref()
		.ok_or_else(|| "no active scene".to_string())?;
	runtime
		.scenes
		.get(current)
		.ok_or_else(|| "active scene not found".to_string())
}

pub(crate) fn current_scene_mut(runtime: &mut crate::ObsRuntime) -> Result<&mut SceneState, String> {
	let name = runtime
		.current_scene
		.as_ref()
		.ok_or_else(|| "no active scene".to_string())?
		.to_string();
	runtime
		.scenes
		.get_mut(&name)
		.ok_or_else(|| "active scene not found".to_string())
}

pub(crate) fn set_current_scene_internal(
	runtime: &mut crate::ObsRuntime,
	name: &str,
) -> Result<(), String> {
	let next = runtime
		.scenes
		.get(name)
		.ok_or_else(|| "scene not found".to_string())?;
	if next.scene_source.is_null() {
		return Err("scene source unavailable".to_string());
	}

	if runtime.current_scene.as_deref() == Some(name) {
		return Ok(());
	}

	unsafe {
		if !runtime.transition_source.is_null() {
			if runtime.program_transition_kind != "cut" && runtime.program_transition_duration_ms > 0 {
				// Animated program transition: play from current scene to the
				// newly selected one; the transition source keeps showing frames.
				crate::logging::trace::step("scene-switch", "obs_transition_start:enter");
				revo_lib::obs::obs_transition_start(
					runtime.transition_source,
					revo_lib::obs::obs_transition_mode_OBS_TRANSITION_MODE_AUTO,
					runtime.program_transition_duration_ms,
					next.scene_source,
				);
				crate::logging::trace::step("scene-switch", "obs_transition_start:done");
			} else {
				// The transition source manages showing/dec_showing internally.
				crate::logging::trace::step("scene-switch", "obs_transition_set:enter");
				revo_lib::obs::obs_transition_set(runtime.transition_source, next.scene_source);
				crate::logging::trace::step("scene-switch", "obs_transition_set:done");
			}
		} else {
			revo_lib::obs::obs_set_output_source(0, next.scene_source);
			revo_lib::obs::obs_source_inc_showing(next.scene_source);
			if let Some(current_name) = runtime.current_scene.clone() {
				if let Some(current) = runtime.scenes.get(&current_name) {
					if !current.scene_source.is_null() {
						revo_lib::obs::obs_source_dec_showing(current.scene_source);
					}
				}
			}
		}
		if !runtime.preview_view.is_null() {
			crate::logging::trace::step("scene-switch", "obs_view_set_source");
			revo_lib::obs::obs_view_set_source(runtime.preview_view, 0, next.scene_source);
		}
	}
	runtime.current_scene = Some(name.to_string());
	runtime.scene_generation = runtime.scene_generation.wrapping_add(1);
	crate::logging::trace::step("scene-switch", "scene-committed");
	Ok(())
}
