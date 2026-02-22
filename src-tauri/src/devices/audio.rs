pub(crate) fn list_pulse_devices(kind: &str) -> Result<Vec<crate::AudioDevice>, String> {
	// OBS pulse input/output sources expect Pulse SOURCE names (device_id),
	// where desktop audio is typically exposed as "*.monitor" sources.
	let list_cmd = vec!["list", "short", "sources"];

	let output = std::process::Command::new("pactl")
		.args(list_cmd)
		.output()
		.map_err(|e| format!("failed to run pactl: {e}"))?;

	if !output.status.success() {
		return Ok(vec![]);
	}

	let text = String::from_utf8_lossy(&output.stdout);
	let mut devices = Vec::new();
	for line in text.lines() {
		let cols: Vec<&str> = line.split('\t').collect();
		if cols.len() < 2 {
			continue;
		}
		let name = cols[1].trim().to_string();
		if name.is_empty() {
			continue;
		}

		let is_monitor = name.ends_with(".monitor");
		if kind == "output" {
			// Desktop/output capture should list monitor sources only.
			if !is_monitor {
				continue;
			}
		} else {
			// Mic/input capture should avoid monitor loopback sources.
			if is_monitor {
				continue;
			}
		}

		devices.push(crate::AudioDevice {
			id: name.clone(),
			name,
		});
	}

	Ok(devices)
}
