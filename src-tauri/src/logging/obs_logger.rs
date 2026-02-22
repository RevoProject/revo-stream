pub(crate) unsafe extern "C" fn obs_log_handler(
	lvl: ::std::os::raw::c_int,
	msg: *const std::os::raw::c_char,
	args: *mut revo_lib::obs::__va_list_tag,
	_p: *mut std::os::raw::c_void,
) {
	if msg.is_null() {
		return;
	}

	let mut buf = vec![0u8; 16384];
	let written = revo_lib::obs::vsnprintf(
		buf.as_mut_ptr() as *mut std::os::raw::c_char,
		buf.len() as ::std::os::raw::c_ulong,
		msg,
		args,
	);

	let message = if written >= 0 {
		std::ffi::CStr::from_ptr(buf.as_ptr() as *const std::os::raw::c_char)
			.to_string_lossy()
			.trim_end()
			.to_string()
	} else {
		std::ffi::CStr::from_ptr(msg)
			.to_string_lossy()
			.trim_end()
			.to_string()
	};

	let level = match lvl as u32 {
		x if x == revo_lib::obs::LOG_ERROR => "error",
		x if x == revo_lib::obs::LOG_WARNING => "warning",
		x if x == revo_lib::obs::LOG_INFO => "info",
		_ => "debug",
	};

	if message.contains("Could not update timestamps for skipped samples") {
		return;
	}

	crate::logging::debug::push_debug_log_entry(
		format!("libobs:{level}"),
		Some(serde_json::json!({
			"level": level,
			"message": message,
		})),
	);
}
