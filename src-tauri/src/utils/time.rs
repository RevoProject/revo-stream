pub(crate) fn current_year_local() -> i32 {
    unsafe {
        let mut now: libc::time_t = 0;
        libc::time(&mut now as *mut libc::time_t);
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now as *const libc::time_t, &mut tm as *mut libc::tm).is_null() {
            return 1970;
        }
        tm.tm_year + 1900
    }
}
