use tauri::Manager;

/// Tauri-side orchestration of the Browser Interaction flow: the backend owns
/// the CDP session (`sources::browser_interact`), this adapter opens/focuses
/// the webview window that renders it.
pub(crate) fn open_source_interaction(
	app: tauri::AppHandle,
	id: String,
	open_window: bool,
	(url, custom_css, custom_js, user_agent, width, height): (String, String, String, String, i64, i64),
) -> Result<String, String> {
	let url = url.trim().to_string();
	if !url.starts_with("http://") && !url.starts_with("https://") {
		return Err("browser source needs an http(s) URL before interaction is available".to_string());
	}

	let viewport_width = if width > 0 { width as u32 } else { 1280 };
	let viewport_height = if height > 0 { height as u32 } else { 720 };
	crate::sources::browser_interact::ensure_session(
		id.trim(),
		&url,
		&user_agent,
		&custom_css,
		&custom_js,
		(viewport_width, viewport_height),
	)?;
	if !open_window {
		return Ok(format!("interaction session ready: {url}"));
	}

	let window_label = format!("interact-{}", id.trim().replace([' ', '#', '/', ':'], "_"));
	let route = format!("interact?id={}", percent_encode_query_value(id.trim()));
	if let Some(existing) = app.get_webview_window(&window_label) {
		let _ = existing.set_focus();
		let _ = existing.eval(&format!(
			"window.location.hash = '' ; window.location.href = window.location.origin + '/{}';",
			route
		));
	} else {
		tauri::WebviewWindowBuilder::new(&app, &window_label, tauri::WebviewUrl::App(route.into()))
			.title(format!("Interact - {id}"))
			.inner_size(
				(viewport_width as f64).min(1280.0).max(480.0),
				((viewport_height as f64) + 90.0).min(800.0).max(360.0),
			)
			.build()
			.map_err(|e| format!("open interact window: {e}"))?;
	}
	Ok(format!("interaction session ready: {url}"))
}

fn percent_encode_query_value(value: &str) -> String {
	let mut out = String::new();
	for byte in value.bytes() {
		match byte {
			b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
				out.push(byte as char);
			}
			_ => out.push_str(&format!("%{byte:02X}")),
		}
	}
	out
}
