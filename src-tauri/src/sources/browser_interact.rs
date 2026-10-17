//! Persistent headless Chromium session driven over the Chrome DevTools
//! Protocol (CDP). One shared instance powers both the OBS source frames and
//! the interactive window, so interactions made in the Interact window are
//! reflected in the rendered source output — like OBS Studio's Interact.

use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tungstenite::Message as WsMessage;
use tungstenite::stream::MaybeTlsStream;
use tauri::Url as ParsedUrl;

const SESSION_IDLE_SHUTDOWN: Duration = Duration::from_secs(300);

#[derive(Default, serde::Serialize)]
struct InteractStats {
	screencast_frames: u64,
	screencast_bytes_total: u64,
	frame_calls: u64,
	frame_fast_hits: u64,
	frame_forced_captures: u64,
	frame_us_max: u64,
	rpc_calls: u64,
	rpc_us_max: u64,
	rpc_timeouts: u64,
	ensure_calls: u64,
	ensure_us_max: u64,
	ensure_launches: u64,
	reaped_sessions: u64,
	dispatch_mouse_calls: u64,
	dispatch_key_calls: u64,
}

fn interact_stats<T>(f: impl FnOnce(&mut InteractStats) -> T) -> T {
	static STATS: OnceLock<Mutex<InteractStats>> = OnceLock::new();
	let mut guard = STATS
		.get_or_init(|| Mutex::new(InteractStats::default()))
		.lock()
		.unwrap_or_else(|poisoned| poisoned.into_inner());
	f(&mut guard)
}

pub(crate) fn debug_stats() -> serde_json::Value {
	let counters = interact_stats(|stats| serde_json::to_value(stats).unwrap_or_default());
	let sessions_json = match sessions().try_lock() {
		Ok(map) => {
			let now = Instant::now();
			serde_json::json!(map.iter().map(|(source_id, session)| {
				let frame_age_ms = session.latest_frame.lock().ok()
					.and_then(|slot| slot.as_ref().map(|(at, png)| serde_json::json!({
						"age_ms": at.elapsed().as_millis() as u64, "bytes": png.len()
					}))).unwrap_or(serde_json::Value::Null);
				let idle_ms = session.last_used.lock().ok()
					.map(|last| now.duration_since(*last).as_millis() as u64).unwrap_or(0);
				serde_json::json!({ "source_id": source_id, "url": session.page_url,
					"viewport": session.viewport, "idle_ms": idle_ms, "latest_frame": frame_age_ms })
			}).collect::<Vec<_>>())
		}
		Err(_) => serde_json::json!({ "sessions_lock": "busy" }),
	};
	serde_json::json!({
		"counters": counters,
		"sessions": sessions_json,
		"mutex_probe": crate::logging::mutex_probe::snapshot(),
		"session_idle_shutdown_s": SESSION_IDLE_SHUTDOWN.as_secs(),
	})
}

struct InteractProcess {
	proc: Child,
}

impl Drop for InteractProcess {
	fn drop(&mut self) {
		#[cfg(unix)]
		unsafe {
			libc::kill(-(self.proc.id() as i32), libc::SIGKILL);
		}
		let _ = self.proc.kill();
		let _ = self.proc.wait();
	}
}

struct InteractSession {
	_process: InteractProcess,
	port: u16,
	page_ws: Mutex<Option<tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>>>,
	next_id: Mutex<u64>,
	latest_frame: Mutex<Option<(Instant, Vec<u8>)>>,
	page_url: String,
	user_agent: String,
	css: String,
	js: String,
	viewport: (u32, u32),
	last_used: Mutex<Instant>,
}

impl InteractSession {
	fn rpc(&self, request: &serde_json::Value) -> Result<Value, String> {
		let started = Instant::now();
		let result = self.rpc_inner(request);
		let elapsed_us = started.elapsed().as_micros() as u64;
		let timed_out = matches!(&result, Err(message) if message.contains("timeout"));
		interact_stats(|stats| {
			stats.rpc_calls += 1;
			stats.rpc_us_max = stats.rpc_us_max.max(elapsed_us);
			if timed_out { stats.rpc_timeouts += 1; }
		});
		result
	}

	fn rpc_inner(&self, request: &serde_json::Value) -> Result<Value, String> {
		let mut guard = self
			.page_ws
			.lock()
			.map_err(|_| "interact socket poisoned".to_string())?;
		if guard.is_none() {
			let ws = connect_page(self.port)
				.ok_or_else(|| "interact page socket unavailable".to_string())?;
			*guard = Some(ws);
		}
		let socket = guard.as_mut().ok_or("interact socket unavailable")?;
		let payload = serde_json::to_string(request).map_err(|e| format!("cdp encode: {e}"))?;
		socket
			.send(WsMessage::Text(payload.into()))
			.map_err(|e| format!("cdp send: {e}"))?;
		let deadline = Instant::now() + Duration::from_secs(15);
		loop {
			let remaining = deadline
				.checked_duration_since(Instant::now())
				.ok_or_else(|| "cdp response timeout".to_string())?;
			match socket.get_ref() {
				tungstenite::stream::MaybeTlsStream::Plain(tcp) => tcp
					.set_read_timeout(Some(remaining))
					.map_err(|e| format!("cdp timeout: {e}"))?,
				_ => return Err("interact socket unsupported".to_string()),
			}
			let message = match socket.read() {
				Ok(msg) => msg,
				Err(tungstenite::Error::Io(err))
					if err.kind() == std::io::ErrorKind::WouldBlock
						|| err.kind() == std::io::ErrorKind::TimedOut =>
				{
					return Err("cdp response timeout".to_string());
				}
				Err(e) => return Err(format!("cdp read: {e}")),
			};
			let WsMessage::Text(data) = message else {
				continue;
			};
			let value: Value = serde_json::from_str(&data).map_err(|e| format!("cdp decode: {e}"))?;
			if value.get("method").and_then(|v| v.as_str()) == Some("Page.screencastFrame") {
				let session_id = value.pointer("/params/sessionId").and_then(|v| v.as_i64()).unwrap_or(0);
				if let Some(data_b64) = value.pointer("/params/data").and_then(|v| v.as_str()) {
					if let Ok(png) = base64_decode(data_b64) {
						interact_stats(|stats| {
							stats.screencast_frames += 1;
							stats.screencast_bytes_total += png.len() as u64;
						});
						if let Ok(mut slot) = self.latest_frame.lock() {
							*slot = Some((Instant::now(), png));
						}
					}
				}
				let _ = socket.send(WsMessage::Text(
					serde_json::json!({ "id": self.next_rpc_id(), "method": "Page.screencastFrameAck", "params": { "sessionId": session_id } })
						.to_string()
						.into(),
				));
				continue;
			}
			if value.get("id") == request.get("id") {
				if let Some(error) = value.get("error") {
					return Err(format!("cdp error: {error}"));
				}
				return Ok(value.get("result").cloned().unwrap_or(Value::Null));
			}
			// Events from the page are drained until the matching response arrives.
		}
	}

	fn send_only(&self, request: &serde_json::Value) -> Result<(), String> {
		self.rpc(request).map(|_| ())
	}

	fn inject_code(&self) -> Result<(), String> {
		let mut script = String::from("(function(){function apply(){");
		if !self.css.trim().is_empty() {
			let css = serde_json::to_string(&self.css).map_err(|e| format!("css encode: {e}"))?;
			script.push_str(&format!(
				"var s=document.querySelector('style[data-revo-custom]');if(!s){{s=document.createElement('style');s.setAttribute('data-revo-custom','1');(document.head||document.documentElement).appendChild(s);}}s.textContent={css};"
			));
		}
		if !self.js.trim().is_empty() {
			let js = serde_json::to_string(&self.js).map_err(|e| format!("js encode: {e}"))?;
			script.push_str(&format!(
				"try{{new Function({js})()}}catch(e){{console.error('[RevoStream custom JS]',e)}}"
			));
		}
		script.push_str(
			"}window.__revoReapply=apply;if(document.readyState==='loading'){document.addEventListener('DOMContentLoaded',apply);window.addEventListener('load',apply);}else{apply();}})();",
		);
		self.send_only(&serde_json::json!({
			"id": self.next_rpc_id(),
			"method": "Runtime.evaluate",
			"params": { "expression": script }
		}))?;
		self.send_only(&serde_json::json!({
			"id": self.next_rpc_id(),
			"method": "Page.addScriptToEvaluateOnNewDocument",
			"params": { "source": script }
		}))?;
		self.send_only(&serde_json::json!({
			"id": self.next_rpc_id(),
			"method": "Page.startScreencast",
			"params": { "format": "png", "everyNthFrame": 1 }
		}))?;
		Ok(())
	}

	fn next_rpc_id(&self) -> u64 {
		let mut guard = self
			.next_id
			.lock()
			.unwrap_or_else(|e| e.into_inner());
		*guard += 1;
		*guard
	}

	fn navigate(&self, url: &str) -> Result<(), String> {
		self.send_only(&serde_json::json!({
			"id": self.next_rpc_id(),
			"method": "Page.navigate",
			"params": { "url": url }
		}))?;
		std::thread::sleep(Duration::from_millis(900));
		Ok(())
	}

	fn set_viewport(&self, width: u32, height: u32) -> Result<(), String> {
		self.send_only(&serde_json::json!({
			"id": self.next_rpc_id(),
			"method": "Emulation.setDeviceMetricsOverride",
			"params": {
				"width": width.clamp(64, 4096),
				"height": height.clamp(64, 4096),
				"deviceScaleFactor": 1,
				"mobile": false
			}
		}))?;
		Ok(())
	}

	fn screenshot_png(&self) -> Result<Vec<u8>, String> {
		// Screencast drives the compositor: frames arrive on content damage.
		// A fresh captured frame (<= 400 ms old) is preferred; otherwise
		// trigger Page.captureScreenshot and wait briefly for the screencast
		// to deliver the updated frame.
		if let Ok(slot) = self.latest_frame.lock() {
			if let Some((captured_at, png)) = slot.as_ref() {
				if captured_at.elapsed() < Duration::from_millis(400) {
					interact_stats(|stats| stats.frame_fast_hits += 1);
					return Ok(png.clone());
				}
			}
		}

		interact_stats(|stats| stats.frame_forced_captures += 1);
		let request_time = Instant::now();
		let response = self.rpc(&serde_json::json!({
			"id": self.next_rpc_id(),
			"method": "Page.captureScreenshot",
			"params": { "format": "png", "fromSurface": true }
		}))?;
		let data = response
			.get("data")
			.and_then(|v| v.as_str())
			.ok_or("cdp screenshot missing data")?;
		let immediate = base64_decode(data)?;

		let deadline = request_time + Duration::from_millis(700);
		while Instant::now() < deadline {
			if let Ok(slot) = self.latest_frame.lock() {
				if let Some((captured_at, png)) = slot.as_ref() {
					if *captured_at > request_time {
						return Ok(png.clone());
					}
				}
			}
			std::thread::sleep(Duration::from_millis(40));
		}
		Ok(immediate)
	}

	fn dispatch_mouse(&self, params: &serde_json::Value) -> Result<(), String> {
		self.send_only(&serde_json::json!({
			"id": self.next_rpc_id(),
			"method": "Input.dispatchMouseEvent",
			"params": params
		}))
	}

	fn dispatch_key(&self, params: &serde_json::Value) -> Result<(), String> {
		self.send_only(&serde_json::json!({
			"id": self.next_rpc_id(),
			"method": "Input.dispatchKeyEvent",
			"params": params
		}))
	}
}

fn base64_decode(data: &str) -> Result<Vec<u8>, String> {
	use base64::Engine as _;
	base64::engine::general_purpose::STANDARD
		.decode(data)
		.map_err(|e| format!("cdp screenshot decode: {e}"))
}

fn connect_page(port: u16) -> Option<tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>> {
	let list = http_get_json(port, "/json/list")?;
	let targets = list.as_array()?;
	for target in targets.iter().rev() {
		if target.get("type").and_then(|v| v.as_str()) != Some("page") {
			continue;
		}
		let ws = target.get("webSocketDebuggerUrl").and_then(|v| v.as_str())?;
		let parsed: ParsedUrl = ws.parse().ok()?;
		let (socket, _response) = tungstenite::connect(parsed.as_str()).ok()?;
		return Some(socket);
	}
	None
}

fn http_get_json(port: u16, path: &str) -> Option<Value> {
	use std::io::Write;
	let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
	stream
		.set_read_timeout(Some(Duration::from_secs(5)))
		.ok()?;
	let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
	stream.write_all(request.as_bytes()).ok()?;
	let mut raw = Vec::new();
	let mut reader = BufReader::new(stream);
	loop {
		let mut chunk = Vec::new();
		match reader.read_until(b'\n', &mut chunk) {
			Ok(0) | Err(_) => break,
			Ok(_) => raw.extend_from_slice(&chunk),
		}
	}
	let text = String::from_utf8_lossy(&raw);
	let body = text.split("\r\n\r\n").nth(1).or_else(|| text.split("\n\n").nth(1))?;
	// Chunked transfer: strip chunk-size lines.
	if text.to_ascii_lowercase().contains("transfer-encoding: chunked") {
		let mut decoded = String::new();
		let mut rest = body;
		loop {
			let Some(line_end) = rest.find("\r\n") else { break };
			let size = usize::from_str_radix(rest[..line_end].trim(), 16).unwrap_or(0);
			if size == 0 {
				break;
			}
			let start = line_end + 2;
			let end = (start + size).min(rest.len());
			decoded.push_str(&rest[start..end]);
			rest = &rest[(end + 2).min(rest.len())..];
		}
		return serde_json::from_str(&decoded).ok();
	}
	serde_json::from_str(body.trim()).ok()
}

fn sessions() -> &'static Mutex<HashMap<String, Arc<InteractSession>>> {
	static SESSIONS: OnceLock<Mutex<HashMap<String, Arc<InteractSession>>>> = OnceLock::new();
	SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn chromium_binary() -> Option<String> {
	super::browser::chromium_binary_pub()
}

pub(crate) fn ensure_session(
	source_id: &str,
	url: &str,
	user_agent: &str,
	css: &str,
	js: &str,
	viewport: (u32, u32),
) -> Result<(), String> {
	let started = Instant::now();
	let result = ensure_session_inner(source_id, url, user_agent, css, js, viewport);
	let elapsed_us = started.elapsed().as_micros() as u64;
	interact_stats(|stats| {
		stats.ensure_calls += 1;
		stats.ensure_us_max = stats.ensure_us_max.max(elapsed_us);
	});
	result
}

fn ensure_session_inner(
	source_id: &str,
	url: &str,
	user_agent: &str,
	css: &str,
	js: &str,
	viewport: (u32, u32),
) -> Result<(), String> {
	let url = url.trim().to_string();
	if !url.starts_with("http://") && !url.starts_with("https://") {
		return Err("browser source needs an http(s) URL".to_string());
	}
	reap_idle();
	let normalized_user_agent = user_agent.trim();

	{
		let map = crate::logging::mutex_probe::probe_lock(sessions(), "interact.ensure_lookup")
			.map_err(|_| "interact sessions poisoned".to_string())?;
		if let Some(existing) = map.get(source_id) {
			if existing.page_url != url
				|| existing.user_agent != normalized_user_agent
				|| existing.css != css
				|| existing.js != js
				|| existing.viewport != viewport
			{
				drop(map);
				close_session(source_id);
			} else {
				return Ok(());
			}
		}
	}

	let binary = chromium_binary()
		.ok_or("No Chromium/Brave/Chrome executable available for interaction")?;
	let profile_hash: u64 = source_id
		.bytes()
		.map(|b| b as u64)
		.fold(0xcbf29ce484222325u64, |hash, byte| {
			(hash ^ byte as u64).wrapping_mul(0x100000001b3)
		});
	let profile_dir = std::env::temp_dir().join(format!("revo-interact-{profile_hash:016x}"));
	std::fs::create_dir_all(&profile_dir).map_err(|e| format!("create interact profile: {e}"))?;

	let mut cmd = Command::new(&binary);
	cmd.env_remove("LD_LIBRARY_PATH")
		.args([
			"--headless",
			"--disable-gpu",
			"--mute-audio",
			"--no-first-run",
			"--no-default-browser-check",
			"--remote-debugging-port=0",
			"--remote-allow-origins=*",
			"--hide-scrollbars",
			"--disable-frame-rate-limit",
			"--disable-gpu-vsync",
			"--window-size=1280,720",
		])
		.arg(format!("--user-data-dir={}", profile_dir.display()));
	if !user_agent.trim().is_empty() {
		cmd.arg(format!("--user-agent={}", user_agent.trim()));
	}
	cmd.arg(&url)
		.stdin(Stdio::null())
		.stdout(Stdio::null())
		.stderr(Stdio::piped());
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
	let mut child = cmd
		.spawn()
		.map_err(|e| format!("launch interact browser: {e}"))?;
	interact_stats(|stats| stats.ensure_launches += 1);

	let stderr = child.stderr.take();
	let (ready_tx, ready_rx) = std::sync::mpsc::channel::<u16>();
	let stop_flag = Arc::new(AtomicBool::new(false));
	let stop_flag_clone = stop_flag.clone();
	let stderr_thread = std::thread::spawn(move || {
		if let Some(stderr) = stderr {
			let reader = BufReader::new(stderr);
			for line in reader.lines() {
				if stop_flag_clone.load(Ordering::Acquire) {
					break;
				}
				let Ok(line) = line else { break };
				if let Some(port) = parse_devtools_port(&line) {
					let _ = ready_tx.send(port);
					break;
				}
			}
		}
	});

	let port = match ready_rx.recv_timeout(Duration::from_secs(15)) {
		Ok(port) => port,
		Err(_) => {
			let _ = child.kill();
			return Err("interact browser did not expose a DevTools port".to_string());
		}
	};
	stop_flag.store(true, Ordering::Release);
	let _ = stderr_thread.join();

	std::thread::sleep(Duration::from_millis(400));

	let session = InteractSession {
		_process: InteractProcess { proc: child },
		port,
		page_ws: Mutex::new(None),
		next_id: Mutex::new(0),
		latest_frame: Mutex::new(None),
		page_url: url.clone(),
		user_agent: normalized_user_agent.to_string(),
		css: css.to_string(),
		js: js.to_string(),
		viewport: (viewport.0.clamp(64, 4096), viewport.1.clamp(64, 4096)),
		last_used: Mutex::new(Instant::now()),
	};

	session.set_viewport(session.viewport.0, session.viewport.1)?;
	session.inject_code()?;

	crate::logging::mutex_probe::probe_lock(sessions(), "interact.ensure_insert")
		.map_err(|_| "interact sessions poisoned".to_string())?
		.insert(source_id.to_string(), Arc::new(session));

	Ok(())
}

fn parse_devtools_port(line: &str) -> Option<u16> {
	let marker = "DevTools listening on ws://127.0.0.1:";
	let idx = line.find(marker)?;
	let rest = &line[idx + marker.len()..];
	let port_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
	port_str.parse().ok()
}

fn touch(source_id: &str) {
	if let Ok(map) = crate::logging::mutex_probe::probe_lock(sessions(), "interact.touch") {
		if let Some(session) = map.get(source_id) {
			if let Ok(mut last) = session.last_used.lock() {
				*last = Instant::now();
			}
		}
	}
}

pub(crate) fn frame_png(source_id: &str) -> Result<Vec<u8>, String> {
	touch(source_id);
	let started = Instant::now();
	let map = crate::logging::mutex_probe::probe_lock(sessions(), "interact.frame_png")
		.map_err(|_| "interact sessions poisoned".to_string())?;
	let session = map
		.get(source_id)
		.ok_or_else(|| "no interact session".to_string())?;
	let result = session.screenshot_png();
	let elapsed_us = started.elapsed().as_micros() as u64;
	interact_stats(|stats| {
		stats.frame_calls += 1;
		stats.frame_us_max = stats.frame_us_max.max(elapsed_us);
	});
	result
}

pub(crate) fn has_session(source_id: &str) -> bool {
	crate::logging::mutex_probe::probe_lock(sessions(), "interact.has_session")
		.map(|map| map.contains_key(source_id))
		.unwrap_or(false)
}

pub(crate) fn session_url(source_id: &str) -> Option<String> {
	let map = crate::logging::mutex_probe::probe_lock(sessions(), "interact.session_url")
		.ok()?;
	let session = map.get(source_id)?;
	Some(session.page_url.clone())
}

pub(crate) fn dispatch_mouse(
	source_id: &str,
	kind: &str,
	x: f64,
	y: f64,
	button: &str,
	buttons: i64,
	click_count: i64,
	delta_x: f64,
	delta_y: f64,
) -> Result<(), String> {
	touch(source_id);
	let map = crate::logging::mutex_probe::probe_lock(sessions(), "interact.dispatch_mouse")
		.map_err(|_| "interact sessions poisoned".to_string())?;
	interact_stats(|stats| stats.dispatch_mouse_calls += 1);
	let session = map
		.get(source_id)
		.ok_or_else(|| "no interact session".to_string())?;
	let mut params = serde_json::json!({
		"type": kind,
		"x": x,
		"y": y
	});
	if kind != "mouseWheel" {
		params["button"] = serde_json::Value::String(button.to_string());
		params["buttons"] = serde_json::Value::Number(buttons.into());
		if kind == "mousePressed" || kind == "mouseReleased" {
			params["clickCount"] = serde_json::Value::Number(click_count.into());
		}
	} else {
		params["deltaX"] = serde_json::Value::Number(
			serde_json::Number::from_f64(delta_x).unwrap_or_else(|| 0.into()),
		);
		params["deltaY"] = serde_json::Value::Number(
			serde_json::Number::from_f64(delta_y).unwrap_or_else(|| 0.into()),
		);
	}
	session.dispatch_mouse(&params)
}

pub(crate) fn dispatch_key(
	source_id: &str,
	kind: &str,
	key: &str,
	code: &str,
	key_code: i64,
	text: &str,
) -> Result<(), String> {
	touch(source_id);
	let map = crate::logging::mutex_probe::probe_lock(sessions(), "interact.dispatch_key")
		.map_err(|_| "interact sessions poisoned".to_string())?;
	interact_stats(|stats| stats.dispatch_key_calls += 1);
	let session = map
		.get(source_id)
		.ok_or_else(|| "no interact session".to_string())?;
	let mut params = serde_json::json!({
		"type": kind,
		"key": key,
		"code": code,
		"windowsVirtualKeyCode": key_code,
		"nativeVirtualKeyCode": key_code
	});
	if !text.is_empty() && (kind == "keyDown" || kind == "char") {
		params["text"] = serde_json::Value::String(text.to_string());
	}
	session.dispatch_key(&params)
}


pub(crate) fn reapply_code(source_id: &str) -> Result<(), String> {
	touch(source_id);
	let map = crate::logging::mutex_probe::probe_lock(sessions(), "interact.reapply_code")
		.map_err(|_| "interact sessions poisoned".to_string())?;
	let session = map
		.get(source_id)
		.ok_or_else(|| "no interact session".to_string())?;
	session.send_only(&serde_json::json!({
		"id": session.next_rpc_id(),
		"method": "Runtime.evaluate",
		"params": { "expression": "window.__revoReapply && window.__revoReapply()" }
	}))
}

pub(crate) fn navigate_session(source_id: &str, url: &str) -> Result<(), String> {
	touch(source_id);
	let map = crate::logging::mutex_probe::probe_lock(sessions(), "interact.navigate_session")
		.map_err(|_| "interact sessions poisoned".to_string())?;
	let session = map
		.get(source_id)
		.ok_or_else(|| "no interact session".to_string())?;
	session.navigate(url)
}

pub(crate) fn close_session(source_id: &str) {
	if let Ok(mut map) = crate::logging::mutex_probe::probe_lock(sessions(), "interact.close_session") {
		if let Some(session) = map.remove(source_id) {
			drop(map);
			let _ = session
				.rpc(&serde_json::json!({
					"id": 0,
					"method": "Browser.close"
				}))
				.is_err();
		}
	}
}

pub(crate) fn close_all() {
	if let Ok(mut map) = crate::logging::mutex_probe::probe_lock(sessions(), "interact.close_all") {
		map.clear();
	}
}

pub(crate) fn reap_idle() {
	let now = Instant::now();
	if let Ok(mut map) = crate::logging::mutex_probe::probe_lock(sessions(), "interact.reap_idle") {
		let before = map.len();
		map.retain(|_, session| {
			let idle = session
				.last_used
				.lock()
				.map(|last| now.duration_since(*last) < SESSION_IDLE_SHUTDOWN)
				.unwrap_or(false);
			idle
		});
		let reaped = before.saturating_sub(map.len()) as u64;
		if reaped > 0 {
			interact_stats(|stats| stats.reaped_sessions += reaped);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	#[ignore = "requires working Chromium installation; launches a real browser session"]
	fn interact_session_screenshot_reflects_click() {
		let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
		listener.set_nonblocking(true).unwrap();
		let url = format!("http://{}/", listener.local_addr().unwrap());
		let done = Arc::new(AtomicBool::new(false));
		let server_done = done.clone();
		let server = std::thread::spawn(move || {
			use std::io::{Read, Write};
			while !server_done.load(Ordering::Acquire) {
				if let Ok((mut stream, _)) = listener.accept() {
					let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
					let mut request = [0; 4096];
					let _ = stream.read(&mut request);
					let body = "<!doctype html><html><body style='margin:0'>						<button id='b' onclick=\"document.body.style.background='rgb(200,30,30)'\" 						style='position:absolute;left:80px;top:60px;width:120px;height:40px'>click</button>						</body></html>";
					let response = format!(
						"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
						body.len(),
						body
					);
					let _ = stream.write_all(response.as_bytes());
				} else {
					std::thread::sleep(Duration::from_millis(10));
				}
			}
		});

		ensure_session("test-interact", &url, "", "", "", (640, 360)).unwrap();
		let frame = frame_png("test-interact").unwrap();
		let image = image::load_from_memory(&frame).unwrap().to_rgba8();
		// Sample the page background away from the button.
		assert_ne!(image.get_pixel(300, 200).0, [200, 30, 30, 255]);

		dispatch_mouse(
			"test-interact",
			"mouseMoved",
			90.0,
			65.0,
			"left",
			0,
			0,
			0.0,
			0.0,
		)
		.unwrap();
		dispatch_mouse(
			"test-interact",
			"mousePressed",
			140.0,
			80.0,
			"left",
			1,
			1,
			0.0,
			0.0,
		)
		.unwrap();
		dispatch_mouse(
			"test-interact",
			"mouseReleased",
			140.0,
			80.0,
			"left",
			0,
			1,
			0.0,
			0.0,
		)
		.unwrap();

		let mut changed = false;
		for _ in 0..20 {
			let _ = dispatch_mouse(
				"test-interact",
				"mouseMoved",
				140.0,
				80.0,
				"left",
				0,
				0,
				0.0,
				0.0,
			);
			let after = frame_png("test-interact").unwrap();
			let image = image::load_from_memory(&after).unwrap().to_rgba8();
			if image.get_pixel(300, 200).0 == [200, 30, 30, 255] {
				changed = true;
				break;
			}
			std::thread::sleep(Duration::from_millis(250));
		}
		assert!(changed, "click did not propagate into rendered page");
		done.store(true, Ordering::Release);
		server.join().unwrap();
		close_session("test-interact");
	}
}
