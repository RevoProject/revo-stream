use std::os::raw::{c_char, c_int};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
type VaListParam = *mut revo_lib::obs::__va_list_tag;

#[cfg(target_os = "windows")]
type VaListParam = *mut c_char;

#[cfg(target_os = "windows")]
extern "C" {
	fn vsnprintf(
		buffer: *mut c_char,
		count: usize,
		format: *const c_char,
		arglist: VaListParam,
	) -> c_int;
}

fn format_message(msg: *const c_char, args: VaListParam, buf: &mut [u8]) -> String {
	if msg.is_null() || buf.is_empty() {
		return String::new();
	}

	#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
	let written = unsafe {
		revo_lib::obs::vsnprintf(
			buf.as_mut_ptr() as *mut c_char,
			buf.len() as ::std::os::raw::c_ulong,
			msg,
			args,
		)
	};

	#[cfg(target_os = "windows")]
	let written = unsafe { vsnprintf(buf.as_mut_ptr() as *mut c_char, buf.len(), msg, args) };

	buf[buf.len() - 1] = 0;
	let formatted = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr() as *const c_char) }
		.to_string_lossy()
		.trim_end()
		.to_string();
	if formatted.is_empty() || written <= 0 {
		return unsafe { std::ffi::CStr::from_ptr(msg) }
			.to_string_lossy()
			.trim_end()
			.to_string();
	}
	formatted
}

pub(crate) unsafe extern "C" fn obs_log_handler(
	lvl: c_int,
	msg: *const c_char,
	args: VaListParam,
	_p: *mut std::os::raw::c_void,
) {
	if msg.is_null() {
		return;
	}

	let mut buf = vec![0u8; 16384];
	let message = format_message(msg, args, &mut buf);
	if message.is_empty() {
		return;
	}

	let level = match lvl {
		x if x == revo_lib::obs::LOG_ERROR as i32 => "error",
		x if x == revo_lib::obs::LOG_WARNING as i32 => "warning",
		x if x == revo_lib::obs::LOG_INFO as i32 => "info",
		_ => "debug",
	};

	if message.contains("Could not update timestamps for skipped samples") {
		return;
	}

	eprintln!("[libobs:{level}] {message}");

	crate::logging::debug::push_debug_log_entry(
		format!("libobs:{level}"),
		Some(serde_json::json!({
			"level": level,
			"message": message,
		})),
	);
}
