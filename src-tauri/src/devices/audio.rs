pub(crate) fn list_pulse_devices(kind: &str) -> Result<Vec<crate::AudioDevice>, String> {
	// OBS pulse sources expect Pulse SOURCE names (device_id); the full
	// `pactl list sources` output also carries a human Description we use
	// as the friendly label (short names look like
	// `alsa_input.usb-Logitech_c340s...`).
	let output = std::process::Command::new("pactl")
		.args(["list", "sources"])
		.output()
		.map_err(|e| format!("failed to run pactl: {e}"))?;

	if !output.status.success() {
		return Ok(vec![]);
	}

	let text = String::from_utf8_lossy(&output.stdout);
	let mut devices = Vec::new();
	let mut current_name: Option<String> = None;
	let mut current_description: Option<String> = None;

	let flush = |current_name: &mut Option<String>,
	             current_description: &mut Option<String>,
	             devices: &mut Vec<crate::AudioDevice>| {
		if let Some(name) = current_name.take() {
			let friendly = current_description
				.take()
				.map(|d| d.trim().to_string())
				.filter(|d| !d.is_empty());
			devices.push(crate::AudioDevice {
				id: name.clone(),
				name: friendly.unwrap_or(name),
			});
		}
	};

	for line in text.lines() {
		let trimmed = line.trim();
		if trimmed.starts_with("Source #") {
			flush(&mut current_name, &mut current_description, &mut devices);
			continue;
		}
		if let Some(value) = trimmed.strip_prefix("Name: ") {
			current_name = Some(value.trim().to_string());
			continue;
		}
		if let Some(value) = trimmed.strip_prefix("Description: ") {
			current_description = Some(value.trim().to_string());
		}
	}
	flush(&mut current_name, &mut current_description, &mut devices);

	devices.retain(|device| {
		let is_monitor = device.id.ends_with(".monitor");
		if kind == "output" {
			is_monitor
		} else {
			!is_monitor
		}
	});

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
