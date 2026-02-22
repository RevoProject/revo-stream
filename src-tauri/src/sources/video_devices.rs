pub(crate) fn list_video_device_picker_items() -> Result<Vec<crate::SourceTypeItem>, String> {
	let mut out: Vec<crate::SourceTypeItem> = Vec::new();
	let entries = match std::fs::read_dir("/dev") {
		Ok(v) => v,
		Err(_) => return Ok(out),
	};

	let mut paths: Vec<String> = Vec::new();
	for entry in entries.flatten() {
		let name = entry.file_name().to_string_lossy().to_string();
		if name.starts_with("video") {
			paths.push(format!("/dev/{name}"));
		}
	}

	paths.sort();
	for path in paths {
		out.push(crate::SourceTypeItem {
			id: path.clone(),
			label: path,
		});
	}

	Ok(out)
}
