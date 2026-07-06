pub(crate) fn list_system_fonts() -> Result<Vec<crate::SourceTypeItem>, String> {
	let mut out: Vec<crate::SourceTypeItem> = Vec::new();

	// fc-list returns lines like:
	// /nix/store/.../font.ttf: FontName:style=Regular
	if let Ok(output) = std::process::Command::new("fc-list")
		.arg("--format=%{family}\n")
		.output()
	{
		if output.status.success() {
			let text = String::from_utf8_lossy(&output.stdout);
			let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
			for line in text.lines() {
				let family = line.trim();
				if family.is_empty() || seen.contains(family) {
					continue;
				}
				seen.insert(family.to_string());
				// Pobierz pierwszy styl by skonstruowac label
				let id = family.to_string();
				let label = family.to_string();
				out.push(crate::SourceTypeItem { id, label });
			}
		}
	}

	out.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
	Ok(out)
}
