use std::collections::HashSet;

// OBS linux-capture (xcompcap) stores the "capture_window"/"window" setting as a
// decimal X11 window id string. Values like "Title:Class:Exe" parse to 0 and the
// plugin logs "XComposite capture disabled." rendering nothing.
fn window_id_value(raw: &str) -> Option<String> {
	let hex = raw.trim().trim_start_matches("0x");
	if hex.is_empty() {
		return None;
	}
	u64::from_str_radix(hex, 16).ok().map(|id| id.to_string())
}

pub(crate) fn list_window_picker_items() -> Result<Vec<crate::SourceTypeItem>, String> {
	let mut out: Vec<crate::SourceTypeItem> = Vec::new();
	let mut seen: HashSet<String> = HashSet::new();

	if let Ok(output) = std::process::Command::new("wmctrl").arg("-lxp").output() {
		if output.status.success() {
			let text = String::from_utf8_lossy(&output.stdout);
			for line in text.lines() {
				let cols: Vec<&str> = line.split_whitespace().collect();
				if cols.len() < 6 {
					continue;
				}

				let pid = cols[2].trim();
				let class = cols[3].trim();
				let title = cols[5..].join(" ").trim().to_string();
				if class.is_empty() && title.is_empty() {
					continue;
				}

				let exe = pid
					.parse::<u32>()
					.ok()
					.and_then(|p| {
						std::fs::read_link(format!("/proc/{p}/exe"))
							.ok()
							.and_then(|path| path.file_name().map(|n| n.to_string_lossy().to_string()))
					})
					.unwrap_or_else(|| class.to_string());

				let legacy_value = if title.is_empty() {
					format!("{}:{}", class, exe)
				} else {
					format!("{}:{}:{}", title, class, exe)
				};

				let value = window_id_value(cols[0]).unwrap_or(legacy_value);

				if value.trim().is_empty() || seen.contains(&value) {
					continue;
				}
				seen.insert(value.clone());

				let label = if title.is_empty() {
					class.to_string()
				} else {
					format!("{} ({})", title, class)
				};

				out.push(crate::SourceTypeItem { id: value, label });
			}
		}
	}

	if out.is_empty() {
		if let Ok(output) = std::process::Command::new("xwininfo").args(["-root", "-tree"]).output() {
			if output.status.success() {
				let text = String::from_utf8_lossy(&output.stdout);
				for line in text.lines() {
					let trimmed = line.trim();
					if !trimmed.starts_with("0x") || !trimmed.contains('"') {
						continue;
					}

					let parts: Vec<&str> = trimmed.split('"').collect();
					if parts.len() < 2 {
						continue;
					}

					let title = parts[1].trim();
					if title.is_empty() || title == "(has no name)" {
						continue;
					}

					let class = if parts.len() >= 6 {
						let c = parts[5].trim();
						if c.is_empty() {
							"UnknownClass"
						} else {
							c
						}
					} else {
						"UnknownClass"
					};

					let xid_token = trimmed.split_whitespace().next().unwrap_or("");
					let value = window_id_value(xid_token).unwrap_or_else(|| format!("{}:{}", title, class));
					if seen.contains(&value) {
						continue;
					}
					seen.insert(value.clone());

					out.push(crate::SourceTypeItem {
						id: value.clone(),
						label: format!("{} ({})", title, class),
					});
				}
			}
		}
	}

	Ok(out)
}

#[cfg(test)]
mod tests {
	use super::window_id_value;

	#[test]
	fn parses_window_xids_as_decimal() {
		assert_eq!(window_id_value("0x03c00007").as_deref(), Some("62914567"));
		assert_eq!(window_id_value("0x1").as_deref(), Some("1"));
		assert_eq!(window_id_value("junk"), None);
		assert_eq!(window_id_value(""), None);
	}
}
