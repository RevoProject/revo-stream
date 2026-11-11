pub fn current_year_local() -> i32 {
    unsafe {
        let mut now: libc::time_t = 0;
        libc::time(&mut now as *mut libc::time_t);
        let mut tm: libc::tm = std::mem::zeroed();

        #[cfg(unix)]
        let failed =
            libc::localtime_r(&now as *const libc::time_t, &mut tm as *mut libc::tm).is_null();

        #[cfg(windows)]
        let failed = libc::localtime_s(&mut tm as *mut libc::tm, &now as *const libc::time_t) != 0;

        if failed {
            return 1970;
        }

        tm.tm_year + 1900
    }
}

/// Local-time stamp used by the default recording file name (plan §23):
/// `YYYYMMDD_HHMM` in 24-hour time, e.g. `20251025_1230`.
pub fn local_record_stamp() -> String {
    let mut now: libc::time_t = 0;
    unsafe {
        libc::time(&mut now as *mut libc::time_t);
        let mut tm: libc::tm = std::mem::zeroed();

        #[cfg(unix)]
        let failed =
            libc::localtime_r(&now as *const libc::time_t, &mut tm as *mut libc::tm).is_null();

        #[cfg(windows)]
        let failed = libc::localtime_s(&mut tm as *mut libc::tm, &now as *const libc::time_t) != 0;

        if failed {
            return "19700101_0000".to_string();
        }

        format!(
            "{:04}{:02}{:02}_{:02}{:02}",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            tm.tm_hour,
            tm.tm_min
        )
    }
}
