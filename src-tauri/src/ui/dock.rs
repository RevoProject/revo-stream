static BROWSER_DOCK_LAST_URL: std::sync::OnceLock<std::sync::Mutex<Option<String>>> =
	std::sync::OnceLock::new();

fn browser_dock_last_url() -> &'static std::sync::Mutex<Option<String>> {
	BROWSER_DOCK_LAST_URL.get_or_init(|| std::sync::Mutex::new(None))
}

fn set_browser_dock_last_url(value: String) {
	if let Ok(mut guard) = browser_dock_last_url().lock() {
		*guard = Some(value);
	}
}

fn get_browser_dock_last_url() -> Option<String> {
	browser_dock_last_url().lock().ok().and_then(|g| g.clone())
}

pub(crate) fn open_graphic_planner(app: tauri::AppHandle) -> Result<(), String> {
	use tauri::Manager;
	let label = "source-transform";
	if let Some(window) = app.get_webview_window(label) {
		let _ = window.set_focus();
		return Ok(());
	}

	tauri::WebviewWindowBuilder::new(
		&app,
		label,
		tauri::WebviewUrl::App("source-transform".into()),
	)
	.title("RevoStream - Graphic planner")
	.min_inner_size(680.0, 620.0)
	.inner_size(900.0, 700.0)
	.devtools(true)
	.build()
	.map_err(|e| e.to_string())?;

	Ok(())
}

pub(crate) fn open_browser_dock(app: tauri::AppHandle, url: Option<String>) -> Result<String, String> {
	use tauri::Manager;

	let dock_label = "browser-dock";
	let target = url.unwrap_or_else(|| "https://google.com".to_string());
	let trimmed = target.trim();

	let parsed = tauri::Url::parse(trimmed).map_err(|e| format!("invalid url: {e}"))?;
	let scheme = parsed.scheme();
	if scheme != "http" && scheme != "https" {
		return Err("Only http/https URLs are supported".to_string());
	}

	set_browser_dock_last_url(trimmed.to_string());

	let escaped_start_url = trimmed
		.replace('\\', "\\\\")
		.replace('"', "\\\"")
		.replace('\n', "\\n")
		.replace('\r', "\\r");
	let dock_script = format!(
		"(function() {{\n  var startUrl = \"{}\";\n  window.__revoDockStartUrl = startUrl;\n  if (!window.__revoDockBound) {{\n    window.addEventListener('keydown', function(event) {{\n      if (event.key === 'F5') {{\n        event.preventDefault();\n        var target = window.__revoDockStartUrl || startUrl;\n        if (target) window.location.href = target;\n      }}\n    }}, true);\n    window.__revoDockBound = true;\n  }}\n}})();",
		escaped_start_url
	);

	if let Some(window) = app.get_webview_window(dock_label) {
		let _ = window.eval(&dock_script);
		let _ = window.eval(&format!("window.location.href = \"{}\";", escaped_start_url));
		let _ = window.set_focus();
		return Ok("Browser dock focused".to_string());
	}

	let window = tauri::WebviewWindowBuilder::new(&app, dock_label, tauri::WebviewUrl::External(parsed))
		.title("RevoStream - Browser Dock")
		.inner_size(1280.0, 800.0)
		.min_inner_size(640.0, 420.0)
		.resizable(true)
		.on_navigation(|url| {
			set_browser_dock_last_url(url.to_string());
			true
		})
		.build()
		.map_err(|e| format!("failed to open browser dock: {e}"))?;

	let _ = window.eval(&dock_script);

	Ok("Browser dock opened".to_string())
}

pub(crate) fn browser_dock_state(app: tauri::AppHandle) -> Result<crate::BrowserDockState, String> {
	use tauri::Manager;

	if let Some(window) = app.get_webview_window("browser-dock") {
		let current_url = window
			.url()
			.ok()
			.map(|u| u.to_string())
			.or_else(get_browser_dock_last_url);
		return Ok(crate::BrowserDockState {
			is_open: true,
			url: current_url,
		});
	}

	Ok(crate::BrowserDockState {
		is_open: false,
		url: get_browser_dock_last_url(),
	})
}

pub(crate) fn close_browser_dock(app: tauri::AppHandle) -> Result<(), String> {
	use tauri::Manager;

	if let Some(window) = app.get_webview_window("browser-dock") {
		window.close().map_err(|e| e.to_string())?;
	}
	Ok(())
}

pub(crate) fn open_debug_console(app: tauri::AppHandle) -> Result<(), String> {
	use tauri::Manager;
	let label = "debug-console";

	if let Some(window) = app.get_webview_window(label) {
		let _ = window.show();
		let _ = window.set_focus();
		return Ok(());
	}

	tauri::WebviewWindowBuilder::new(&app, label, tauri::WebviewUrl::App("debug-console".into()))
		.title("RevoStream - Debug Console")
		.inner_size(980.0, 680.0)
		.min_inner_size(760.0, 420.0)
		.devtools(true)
		.build()
		.map_err(|e| e.to_string())?;

	Ok(())
}
