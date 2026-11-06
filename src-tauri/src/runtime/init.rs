// Thin Tauri wrappers over the framework-neutral libobs start/shutdown flow
// in revo-backend (which owns `start_with_platform` / `shutdown_with_platform`).

pub fn start(
	app: tauri::AppHandle,
	state: tauri::State<'_, crate::ObsState>,
	root_dir: Option<String>,
) -> Result<String, String> {
	let platform = super::lifecycle::TauriObsPlatform::new(app);
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	super::start_with_platform(&mut runtime, &platform, root_dir)
}

pub fn shutdown(app: tauri::AppHandle, state: tauri::State<'_, crate::ObsState>) -> Result<String, String> {
	let platform = super::lifecycle::TauriObsPlatform::new(app);
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	super::shutdown_with_platform(&mut runtime, &platform)
}
