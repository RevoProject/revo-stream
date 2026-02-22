pub(crate) fn export_profile_obs() -> Result<String, String> {
	let settings = crate::settings::core::settings_get()?;
	let payload = serde_json::json!({
		"format": "obs_profile_v1",
		"profile_name": "RevoStream",
		"stream": {
			"server": settings.stream_url,
			"key": settings.stream_key
		},
		"output": {
			"record_path": settings.record_path,
			"preview_quality": settings.preview_quality,
			"encoder_preference": settings.encoder_preference,
			"scene_resolution": settings.scene_resolution
		},
		"network": {
			"whep_url": settings.whep_url,
			"whip_url": settings.whip_url,
			"auto_retry_preview": settings.auto_retry_preview
		},
		"ui": {
			"autorescale_inputs": settings.autorescale_inputs,
			"profile": settings.ui_profile
		},
		"revo": {
			"active_profile": settings.active_profile
		}
	});

	serde_json::to_string_pretty(&payload)
		.map_err(|e| format!("failed to serialize OBS profile: {e}"))
}

pub(crate) fn export_profile_obs_to_file(path: String) -> Result<String, String> {
	let raw = export_profile_obs()?;
	std::fs::write(&path, raw).map_err(|e| format!("failed to write OBS profile: {e}"))?;
	Ok(format!("OBS profile exported to {}", path))
}

pub(crate) fn import_profile_obs(json: String) -> Result<String, String> {
	let parsed: serde_json::Value =
		serde_json::from_str(&json).map_err(|e| format!("failed to parse OBS profile JSON: {e}"))?;

	let obj = parsed
		.as_object()
		.ok_or_else(|| "OBS profile must be a JSON object".to_string())?;

	let mut settings = crate::settings::core::settings_get()?;

	if let Some(stream) = obj.get("stream").and_then(|v| v.as_object()) {
		if let Some(server) = stream.get("server").and_then(|v| v.as_str()) {
			settings.stream_url = server.to_string();
		}
		if let Some(key) = stream.get("key").and_then(|v| v.as_str()) {
			settings.stream_key = key.to_string();
		}
	}

	if let Some(output) = obj.get("output").and_then(|v| v.as_object()) {
		if let Some(record_path) = output.get("record_path").and_then(|v| v.as_str()) {
			settings.record_path = record_path.to_string();
		}
		if let Some(preview_quality) = output.get("preview_quality").and_then(|v| v.as_str()) {
			settings.preview_quality = Some(preview_quality.to_string());
		}
		if let Some(encoder_preference) = output
			.get("encoder_preference")
			.and_then(|v| v.as_str())
		{
			settings.encoder_preference = Some(encoder_preference.to_string());
		}
		if let Some(scene_resolution) = output.get("scene_resolution").and_then(|v| v.as_str()) {
			settings.scene_resolution = Some(scene_resolution.to_string());
		}
	}

	if let Some(network) = obj.get("network").and_then(|v| v.as_object()) {
		if let Some(whep_url) = network.get("whep_url").and_then(|v| v.as_str()) {
			settings.whep_url = Some(whep_url.to_string());
		}
		if let Some(whip_url) = network.get("whip_url").and_then(|v| v.as_str()) {
			settings.whip_url = Some(whip_url.to_string());
		}
		if let Some(auto_retry_preview) = network
			.get("auto_retry_preview")
			.and_then(|v| v.as_bool())
		{
			settings.auto_retry_preview = Some(auto_retry_preview);
		}
	}

	if let Some(ui) = obj.get("ui").and_then(|v| v.as_object()) {
		if let Some(autorescale_inputs) = ui.get("autorescale_inputs").and_then(|v| v.as_bool()) {
			settings.autorescale_inputs = Some(autorescale_inputs);
		}
		if let Some(profile) = ui.get("profile") {
			if profile.is_object() {
				settings.ui_profile = Some(profile.clone());
			}
		}
	}

	if let Some(revo) = obj.get("revo").and_then(|v| v.as_object()) {
		if let Some(active_profile) = revo.get("active_profile").and_then(|v| v.as_str()) {
			settings.active_profile = Some(active_profile.to_string());
		}
	}

	crate::settings::core::settings_save(settings)
}
