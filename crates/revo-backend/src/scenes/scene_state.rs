pub type SceneState = crate::SceneState;

pub fn current_scene(runtime: &crate::ObsRuntime) -> Result<&SceneState, String> {
    let current = runtime
        .current_scene
        .as_ref()
        .ok_or_else(|| "no active scene".to_string())?;
    runtime
        .scenes
        .get(current)
        .ok_or_else(|| "active scene not found".to_string())
}

pub fn current_scene_mut(runtime: &mut crate::ObsRuntime) -> Result<&mut SceneState, String> {
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

/// One-off transition override for a scene switch (plan §19.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneSwitch<'a> {
    /// The configured program transition and duration.
    Default,
    /// Force this kind and duration for this switch only (Cut / Fade).
    Forced { kind: &'a str, duration_ms: u32 },
    /// Keep the configured kind, retime this switch only.
    Retimed { duration_ms: u32 },
}

pub fn set_current_scene_internal(
    runtime: &mut crate::ObsRuntime,
    name: &str,
) -> Result<(), String> {
    set_current_scene_with(runtime, name, SceneSwitch::Default)
}

/// Switches the program scene, optionally overriding the transition kind and
/// duration for this one switch only (studio-mode Cut/Fade controls, plan
/// §19.2). The runtime's configured transition is left untouched, so the next
/// ordinary switch still uses it.
pub fn set_current_scene_with(
    runtime: &mut crate::ObsRuntime,
    name: &str,
    switch: SceneSwitch<'_>,
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

    // The override wins for this switch only and never rewrites
    // `program_transition_*`, so the next ordinary switch is unaffected.
    let (kind, duration) = match switch {
        SceneSwitch::Default => (
            runtime.program_transition_kind.clone(),
            runtime.program_transition_duration_ms,
        ),
        SceneSwitch::Forced { kind, duration_ms } => (kind.to_string(), duration_ms),
        SceneSwitch::Retimed { duration_ms } => {
            (runtime.program_transition_kind.clone(), duration_ms)
        }
    };
    let kind = kind.as_str();

    unsafe {
        if !runtime.transition_source.is_null() {
            if kind != "cut" && duration > 0 {
                // Animated program transition: play from current scene to the
                // newly selected one; the transition source keeps showing frames.
                crate::logging::trace::step("scene-switch", "obs_transition_start:enter");
                revo_lib::obs::obs_transition_start(
                    runtime.transition_source,
                    revo_lib::obs::obs_transition_mode_OBS_TRANSITION_MODE_AUTO,
                    duration,
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
