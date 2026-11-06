// Tauri-specific libobs platform implementation and runtime-state queries.
// The framework-neutral `ObsPlatform` trait lives in revo-backend.

use revo_lib::obs;

pub struct TauriObsPlatform {
	app: tauri::AppHandle,
}

impl TauriObsPlatform {
	pub fn new(app: tauri::AppHandle) -> Self {
		Self { app }
	}
}

impl super::ObsPlatform for TauriObsPlatform {
	fn startup(&self, locale: &std::ffi::CStr, config: &std::ffi::CStr) -> Result<(), String> {
		#[cfg(target_os = "macos")]
		{
			// libobs hotkey platform init (HIToolbox/TSM) asserts the main
			// dispatch queue, so block this worker until startup completes.
			let locale = locale.to_owned();
			let config = config.to_owned();
			let (tx, rx) = std::sync::mpsc::channel::<bool>();
			self.app
				.run_on_main_thread(move || {
					// SAFETY: The owned C strings outlive this call, and libobs startup
					// is dispatched to the required macOS main thread.
					let ok = unsafe {
						obs::obs_startup(
							locale.as_ptr(),
							config.as_ptr(),
							std::ptr::null_mut(),
						)
					};
					let _ = tx.send(ok);
				})
				.map_err(|e| format!("failed to dispatch obs_startup to main thread: {e}"))?;
			eprintln!("[init] obs_startup dispatched to main thread");
			if !rx
				.recv()
				.map_err(|e| format!("main thread obs_startup dropped: {e}"))?
			{
				return Err("obs_startup failed".to_string());
			}
			Ok(())
		}
		#[cfg(not(target_os = "macos"))]
		{
			let _ = &self.app;
			// SAFETY: The caller serializes the libobs lifecycle, and both C
			// strings remain valid for the duration of obs_startup.
			if unsafe { obs::obs_startup(locale.as_ptr(), config.as_ptr(), std::ptr::null_mut()) } {
				Ok(())
			} else {
				Err("obs_startup failed".to_string())
			}
		}
	}

	fn shutdown(&self) -> Result<(), String> {
		#[cfg(target_os = "macos")]
		{
			// Match obs_startup: teardown touches HIToolbox on macOS.
			let (tx, rx) = std::sync::mpsc::channel::<()>();
			self.app
				.run_on_main_thread(move || {
					// SAFETY: The caller established that the libobs core is active,
					// and teardown runs on the required macOS main thread.
					unsafe {
						obs::obs_shutdown();
					}
					let _ = tx.send(());
				})
				.map_err(|e| format!("failed to dispatch obs_shutdown to main thread: {e}"))?;
			rx.recv()
				.map_err(|e| format!("main thread obs_shutdown dropped: {e}"))?;
			Ok(())
		}
		#[cfg(not(target_os = "macos"))]
		{
			let _ = &self.app;
			// SAFETY: The caller serializes lifecycle access and only invokes
			// shutdown for a successfully started libobs core.
			unsafe {
				obs::obs_shutdown();
			}
			Ok(())
		}
	}
}

pub fn is_initialized(state: tauri::State<'_, crate::ObsState>) -> Result<bool, String> {
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	Ok(runtime.initialized)
}
