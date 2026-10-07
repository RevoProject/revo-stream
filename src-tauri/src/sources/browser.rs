//! Video-only Chromium snapshots, not an embedded/persistent browser engine.
use revo_lib::obs;
use std::{
    ffi::{c_char, c_void, CStr},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, OnceLock, Weak,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const CAPTURE_TIMEOUT: Duration = Duration::from_secs(15);
const REFRESH_INTERVAL: Duration = Duration::from_secs(2);

pub(crate) fn chromium_binary() -> Option<PathBuf> {
    let names = [
        "chromium",
        "chromium-browser",
        "brave-browser",
        "brave",
        "google-chrome-stable",
        "google-chrome",
    ];
    let mut dirs: Vec<PathBuf> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    dirs.extend([PathBuf::from("/usr/bin"), PathBuf::from("/snap/bin")]);
    let mut paths = Vec::new();
    if let Some(path) = std::env::var_os("REVO_BROWSER_BINARY") {
        paths.push(PathBuf::from(path));
    }
    for dir in dirs {
        for name in names {
            paths.push(dir.join(name));
            #[cfg(windows)]
            paths.push(dir.join(format!("{name}.exe")));
        }
    }
    #[cfg(target_os = "macos")]
    for root in [
        Some(PathBuf::from("/Applications")),
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Applications")),
    ]
    .into_iter()
    .flatten()
    {
        for app in [
            "Google Chrome.app/Contents/MacOS/Google Chrome",
            "Chromium.app/Contents/MacOS/Chromium",
            "Brave Browser.app/Contents/MacOS/Brave Browser",
            "Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ] {
            paths.push(root.join(app));
        }
    }
    #[cfg(windows)]
    for root in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
        if let Some(root) = std::env::var_os(root) {
            for app in [
                "Google/Chrome/Application/chrome.exe",
                "Chromium/Application/chrome.exe",
                "BraveSoftware/Brave-Browser/Application/brave.exe",
                "Microsoft/Edge/Application/msedge.exe",
            ] {
                paths.push(PathBuf::from(&root).join(app));
            }
        }
    }
    for path in paths {
        if let Ok(meta) = path.metadata() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if meta.is_file() && meta.permissions().mode() & 0o111 != 0 {
                    return Some(path);
                }
            }
            #[cfg(not(unix))]
            if meta.is_file() {
                return Some(path);
            }
        }
    }
    None
}

pub(crate) fn validate_url(url: &str) -> Result<String, String> {
    let parsed = tauri::Url::parse(url.trim()).map_err(|e| format!("invalid browser URL: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Browser snapshots support only http/https URLs".into());
    }
    Ok(parsed.to_string())
}

struct CaptureDirectory(PathBuf);
impl Drop for CaptureDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct CaptureProcess(Child);
impl Drop for CaptureProcess {
    fn drop(&mut self) {
        // Chromium launches child processes. Kill the isolated process group too.
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.0.id() as i32), libc::SIGKILL);
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub(crate) fn capture_png(
    url: &str,
    width: u32,
    height: u32,
    stop: &AtomicBool,
) -> Result<Vec<u8>, String> {
    if stop.load(Ordering::Acquire) {
        return Err("Browser capture cancelled".into());
    }
    let url = validate_url(url)?;
    let binary = chromium_binary()
        .ok_or("No Chromium/Brave/Chrome executable available for browser snapshots")?;
    let dir = CaptureDirectory(
        std::env::temp_dir().join(format!("revo-browser-{}", uuid::Uuid::new_v4())),
    );
    std::fs::create_dir(&dir.0).map_err(|e| format!("create browser capture directory: {e}"))?;
    let path = dir.0.join("frame.png");
    let mut cmd = Command::new(binary);
    // App/Nix library overrides belong to libobs, not a separately packaged browser.
    cmd.env_remove("LD_LIBRARY_PATH");
    cmd.args([
        "--headless",
        "--disable-gpu",
        "--hide-scrollbars",
        "--mute-audio",
        "--no-first-run",
        "--no-default-browser-check",
        "--run-all-compositor-stages-before-draw",
        "--virtual-time-budget=2500",
    ])
    .arg(format!(
        "--user-data-dir={}",
        dir.0.join("profile").display()
    ))
    .arg(format!(
        "--window-size={},{}",
        width.clamp(64, 4096),
        height.clamp(64, 4096)
    ))
    .arg(format!("--screenshot={}", path.display()))
    .arg(url)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                if libc::setpgid(0, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    let mut process = CaptureProcess(
        cmd.spawn()
            .map_err(|e| format!("start Chromium capture: {e}"))?,
    );
    let start = Instant::now();
    loop {
        if stop.load(Ordering::Acquire) {
            return Err("Browser capture cancelled".into());
        }
        if start.elapsed() >= CAPTURE_TIMEOUT {
            return Err("Chromium capture timed out after 15 seconds".into());
        }
        match process.0.try_wait().map_err(|e| format!("wait for Chromium capture: {e}"))? {
            Some(status) if status.success() => break,
            Some(status) => return Err(format!("Chromium capture failed ({status}); check browser installation and sandbox support")),
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    std::fs::read(&path).map_err(|e| format!("read Chromium snapshot: {e}"))
}

#[derive(Clone, PartialEq)]
struct Config {
    url: String,
    width: u32,
    height: u32,
}
struct State {
    config: Config,
    revision: u64,
    visible: bool,
    status: String,
}
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    stop: AtomicBool,
}
struct Browser {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
    source_address: usize,
}

fn sources() -> &'static Mutex<std::collections::HashMap<usize, Weak<Shared>>> {
    static SOURCES: OnceLock<Mutex<std::collections::HashMap<usize, Weak<Shared>>>> =
        OnceLock::new();
    SOURCES.get_or_init(Mutex::default)
}

pub(crate) fn status(source: *mut obs::obs_source_t) -> Option<String> {
    let shared = sources()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&(source as usize))?
        .upgrade()?;
    let status = shared
        .state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .status
        .clone();
    Some(status)
}

unsafe fn config(settings: *mut obs::obs_data_t) -> Config {
    let url = obs::obs_data_get_string(settings, c"url".as_ptr());
    Config {
        url: if url.is_null() {
            String::new()
        } else {
            CStr::from_ptr(url).to_string_lossy().into_owned()
        },
        width: obs::obs_data_get_int(settings, c"width".as_ptr()).clamp(64, 4096) as u32,
        height: obs::obs_data_get_int(settings, c"height".as_ptr()).clamp(64, 4096) as u32,
    }
}

fn worker(shared: Arc<Shared>, source_address: usize) {
    let source = source_address as *mut obs::obs_source_t;
    while !shared.stop.load(Ordering::Acquire) {
        let (config, revision) = {
            let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            while !state.visible && !shared.stop.load(Ordering::Acquire) {
                state = shared.wake.wait(state).unwrap_or_else(|e| e.into_inner());
            }
            if shared.stop.load(Ordering::Acquire) {
                break;
            }
            (state.config.clone(), state.revision)
        };
        let result = capture_png(&config.url, config.width, config.height, &shared.stop)
            .and_then(|png| {
                image::load_from_memory_with_format(&png, image::ImageFormat::Png)
                    .map(|image| image.to_rgba8())
                    .map_err(|e| format!("decode browser snapshot: {e}"))
            })
            .and_then(|image| {
                if image.dimensions() == (config.width, config.height) {
                    Ok(image)
                } else {
                    Err(format!(
                        "Chromium returned {}x{} instead of {}x{}",
                        image.width(),
                        image.height(),
                        config.width,
                        config.height
                    ))
                }
            });
        let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if shared.stop.load(Ordering::Acquire) {
            break;
        }
        if state.revision != revision || !state.visible {
            continue;
        }
        match result {
            Ok(mut pixels) => unsafe {
                let mut frame: obs::obs_source_frame = std::mem::zeroed();
                frame.data[0] = pixels.as_mut_ptr();
                frame.linesize[0] = config.width * 4;
                frame.width = config.width;
                frame.height = config.height;
                frame.timestamp = obs::obs_get_video_frame_time();
                frame.format = obs::video_format_VIDEO_FORMAT_RGBA;
                frame.full_range = true;
                // libobs copies the pixels during this call; source lives until destroy joins us.
                obs::obs_source_output_video(source, &frame);
                state.status = "Snapshot ready (video-only; page reloads on every refresh)".into();
            },
            Err(error) => {
                unsafe {
                    obs::obs_source_output_video(source, std::ptr::null());
                }
                if state.status != error {
                    crate::push_debug_log_entry(
                        "browser_snapshot:error".into(),
                        Some(serde_json::json!({"url": config.url, "error": error})),
                    );
                }
                state.status = error;
            }
        }
        let _ = shared.wake.wait_timeout(state, REFRESH_INTERVAL);
    }
}

unsafe extern "C" fn create(
    settings: *mut obs::obs_data_t,
    source: *mut obs::obs_source_t,
) -> *mut c_void {
    if settings.is_null() || source.is_null() || chromium_binary().is_none() {
        return std::ptr::null_mut();
    }
    let config = config(settings);
    if validate_url(&config.url).is_err() {
        return std::ptr::null_mut();
    }
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            config,
            revision: 0,
            visible: false,
            status: "Waiting for first Chromium snapshot".into(),
        }),
        wake: Condvar::new(),
        stop: AtomicBool::new(false),
    });
    obs::obs_source_set_async_unbuffered(source, true);
    let worker_shared = shared.clone();
    let address = source as usize;
    let Ok(worker) = std::thread::Builder::new()
        .name("browser-snapshot".into())
        .spawn(move || worker(worker_shared, address))
    else {
        return std::ptr::null_mut();
    };
    sources()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(address, Arc::downgrade(&shared));
    Box::into_raw(Box::new(Browser {
        shared,
        worker: Some(worker),
        source_address: address,
    })) as *mut c_void
}

unsafe extern "C" fn destroy(data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let mut browser = Box::from_raw(data as *mut Browser);
    sources()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&browser.source_address);
    {
        // Hold the predicate lock so shutdown cannot miss a worker's initial wait.
        let _state = browser
            .shared
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        browser.shared.stop.store(true, Ordering::Release);
        browser.shared.wake.notify_all();
    }
    if let Some(worker) = browser.worker.take() {
        let _ = worker.join();
    }
}

unsafe extern "C" fn update(data: *mut c_void, settings: *mut obs::obs_data_t) {
    if data.is_null() || settings.is_null() {
        return;
    }
    let browser = &*(data as *mut Browser);
    let mut state = browser
        .shared
        .state
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let config = config(settings);
    if state.config != config {
        state.config = config;
        state.revision += 1;
        state.status = "Waiting for updated Chromium snapshot".into();
    }
    browser.shared.wake.notify_all();
}

unsafe fn visibility(data: *mut c_void, visible: bool) {
    if data.is_null() {
        return;
    }
    let browser = &*(data as *mut Browser);
    browser
        .shared
        .state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .visible = visible;
    browser.shared.wake.notify_all();
}
unsafe extern "C" fn show(data: *mut c_void) {
    visibility(data, true);
}
unsafe extern "C" fn hide(data: *mut c_void) {
    visibility(data, false);
}
unsafe extern "C" fn name(_: *mut c_void) -> *const c_char {
    c"Browser (Chromium snapshots, video only)".as_ptr()
}
unsafe extern "C" fn defaults(settings: *mut obs::obs_data_t) {
    obs::obs_data_set_default_string(settings, c"url".as_ptr(), c"https://example.com".as_ptr());
    obs::obs_data_set_default_int(settings, c"width".as_ptr(), 1280);
    obs::obs_data_set_default_int(settings, c"height".as_ptr(), 720);
}
unsafe extern "C" fn properties(data: *mut c_void) -> *mut obs::obs_properties_t {
    let props = obs::obs_properties_create();
    obs::obs_properties_add_text(
        props,
        c"url".as_ptr(),
        c"URL (HTTP/HTTPS)".as_ptr(),
        obs::obs_text_type_OBS_TEXT_DEFAULT,
    );
    obs::obs_properties_add_int(props, c"width".as_ptr(), c"Width".as_ptr(), 64, 4096, 1);
    obs::obs_properties_add_int(props, c"height".as_ptr(), c"Height".as_ptr(), 64, 4096, 1);
    let mut status = "Video-only snapshots; reloads every capture, no audio, interaction or persistent page state. Refresh interval: capture time + 2 seconds.".to_string();
    if !data.is_null() {
        let browser = &*(data as *mut Browser);
        status.push_str(" Status: ");
        status.push_str(
            &browser
                .shared
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .status,
        );
    }
    if let Ok(status) = std::ffi::CString::new(status) {
        obs::obs_properties_add_text(
            props,
            c"snapshot_status".as_ptr(),
            status.as_ptr(),
            obs::obs_text_type_OBS_TEXT_INFO,
        );
    }
    props
}

/// Call after OBS startup, on each initialization. Unavailable backends are not registered.
pub(crate) fn register() -> Result<(), String> {
    if chromium_binary().is_none() {
        return Err("Browser snapshots unavailable: install Chromium, Brave or Chrome".into());
    }
    unsafe {
        let mut index = 0;
        let mut id = std::ptr::null();
        while obs::obs_enum_input_types(index, &mut id) {
            if !id.is_null() && CStr::from_ptr(id).to_bytes() == b"browser_source" {
                return Ok(());
            }
            index += 1;
        }
        let mut info: obs::obs_source_info = std::mem::zeroed();
        info.id = c"browser_source".as_ptr();
        info.type_ = obs::obs_source_type_OBS_SOURCE_TYPE_INPUT;
        info.output_flags = obs::OBS_SOURCE_ASYNC_VIDEO;
        info.get_name = Some(name);
        info.create = Some(create);
        info.destroy = Some(destroy);
        info.update = Some(update);
        info.get_defaults = Some(defaults);
        info.get_properties = Some(properties);
        info.show = Some(show);
        info.hide = Some(hide);
        obs::obs_register_source_s(&info, std::mem::size_of::<obs::obs_source_info>());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_only_accept_web_urls() {
        assert_eq!(
            validate_url(" https://example.com ").unwrap(),
            "https://example.com/"
        );
        for url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,test",
            "not a URL",
        ] {
            assert!(validate_url(url).is_err());
        }
    }

    #[test]
    fn cancelled_capture_does_not_start_browser() {
        assert_eq!(
            capture_png("https://example.com", 1280, 720, &AtomicBool::new(true)).unwrap_err(),
            "Browser capture cancelled"
        );
    }

    fn serve_page() -> (String, Arc<AtomicBool>, JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let done = Arc::new(AtomicBool::new(false));
        let server_done = done.clone();
        let server = std::thread::spawn(move || {
            while !server_done.load(Ordering::Acquire) {
                if let Ok((mut stream, _)) = listener.accept() {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    let mut request = [0; 4096];
                    let _ = stream.read(&mut request);
                    let body = "<!doctype html><html style='background:rgb(12,34,56)'><body>Real browser page</body></html>";
                    let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
                } else {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        });
        (url, done, server)
    }

    #[test]
    #[ignore = "requires working Chromium installation and sandbox"]
    fn chromium_renders_actual_page_pixels() {
        let (url, done, server) = serve_page();
        let result = capture_png(&url, 320, 240, &AtomicBool::new(false));
        done.store(true, Ordering::Release);
        server.join().unwrap();
        let image = image::load_from_memory(&result.unwrap())
            .unwrap()
            .to_rgba8();
        assert_eq!(image.dimensions(), (320, 240));
        assert_eq!(image.get_pixel(300, 200).0, [12, 34, 56, 255]);
    }

    #[test]
    #[ignore = "requires libobs and Chromium; initializes the process-global OBS runtime"]
    fn registered_obs_source_submits_snapshot_and_cleans_up() {
        let (url, done, server) = serve_page();
        unsafe {
            assert!(obs::obs_startup(
                c"en-US".as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut()
            ));
            register().unwrap();
            let settings = obs::obs_data_create();
            let url = std::ffi::CString::new(url).unwrap();
            obs::obs_data_set_string(settings, c"url".as_ptr(), url.as_ptr());
            obs::obs_data_set_int(settings, c"width".as_ptr(), 320);
            obs::obs_data_set_int(settings, c"height".as_ptr(), 240);
            let source = obs::obs_source_create(
                c"browser_source".as_ptr(),
                c"Browser smoke test".as_ptr(),
                settings,
                std::ptr::null_mut(),
            );
            obs::obs_data_release(settings);
            assert!(!source.is_null());
            assert_eq!(
                obs::obs_source_get_output_flags(source) & obs::OBS_SOURCE_ASYNC_VIDEO,
                obs::OBS_SOURCE_ASYNC_VIDEO
            );
            // No graphics/video thread in this smoke test to dispatch OBS show callbacks.
            // Signal the same worker predicate as show(), then exercise real libobs output.
            let shared = sources()
                .lock()
                .unwrap()
                .get(&(source as usize))
                .unwrap()
                .upgrade()
                .unwrap();
            shared.state.lock().unwrap().visible = true;
            shared.wake.notify_all();
            let start = Instant::now();
            let mut submitted = false;
            while start.elapsed() < Duration::from_secs(20) {
                if status(source).is_some_and(|status| status.starts_with("Snapshot ready")) {
                    submitted = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let last_status = status(source);
            obs::obs_source_release(source);
            obs::obs_shutdown();
            done.store(true, Ordering::Release);
            server.join().unwrap();
            assert!(submitted, "no OBS snapshot submission: {last_status:?}");
            assert!(
                status(source).is_none(),
                "destroy must remove backend state"
            );
        }
    }
}
