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

pub(crate) fn list_alsa_devices() -> Result<Vec<crate::AudioDevice>, String> {
	let output = std::process::Command::new("arecord")
		.args(["-l"])
		.output()
		.map_err(|e| format!("failed to run arecord: {e}"))?;

	if !output.status.success() {
		return Ok(vec![]);
	}

	let text = String::from_utf8_lossy(&output.stdout);
	let mut devices = Vec::new();
	for line in text.lines() {
		// arecord -l outputs lines like:
		// card 0: DeviceName [Device Description], device 0: ...
		if let Some(card) = line.strip_prefix("card ") {
			let parts: Vec<&str> = card.splitn(2, ':').collect();
			if parts.len() == 2 {
				let card_info = parts[1].trim();
				let hw_id = format!("hw:{}", parts[0].split_whitespace().next().unwrap_or("0"));
				let label = card_info.trim().to_string();
				devices.push(crate::AudioDevice {
					id: hw_id,
					name: label,
				});
			}
		}
	}

	Ok(devices)
}
