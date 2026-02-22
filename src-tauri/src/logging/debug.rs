static DEBUG_LOG_ENTRIES: std::sync::OnceLock<std::sync::Mutex<Vec<crate::LibObsActionEvent>>> =
    std::sync::OnceLock::new();
static DEBUG_LOG_FILE_PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
static DEBUG_START_TS_SECS: std::sync::OnceLock<u64> = std::sync::OnceLock::new();

fn debug_log_entries() -> &'static std::sync::Mutex<Vec<crate::LibObsActionEvent>> {
    DEBUG_LOG_ENTRIES.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

fn sensitive_placeholder(key: &str) -> Option<&'static str> {
    let normalized = key.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    if normalized == "stream_url" || normalized == "streamurl" {
        return Some("{stream_url}");
    }
    if normalized == "stream_key" || normalized == "streamkey" || normalized == "key" {
        return Some("{stream_key}");
    }
    if normalized.contains("password")
        || normalized.contains("token")
        || normalized.contains("secret")
        || normalized.contains("authorization")
        || normalized.contains("auth")
    {
        return Some("{redacted}");
    }
    None
}

fn sanitize_log_detail(value: serde_json::Value) -> serde_json::Value {
    fn sanitize_value(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let mut next = serde_json::Map::new();
                for (key, val) in map {
                    if let Some(mask) = sensitive_placeholder(&key) {
                        next.insert(key, serde_json::Value::String(mask.to_string()));
                    } else {
                        next.insert(key, sanitize_value(val));
                    }
                }
                serde_json::Value::Object(next)
            }
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.into_iter().map(sanitize_value).collect())
            }
            other => other,
        }
    }

    sanitize_value(value)
}

pub(crate) fn init_debug_log_file() -> Result<std::path::PathBuf, String> {
    let start_ts = *DEBUG_START_TS_SECS.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    });

    let year = crate::utils::time::current_year_local();
    let dir = crate::utils::fs::runtime_logs_root_dir()?.join(year.to_string());
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create logs dir: {e}"))?;

    let path = dir.join(format!("logs_{start_ts}.log"));
    if !path.exists() {
        std::fs::write(&path, b"").map_err(|e| format!("failed to create log file: {e}"))?;
    }
    Ok(path)
}

pub(crate) fn write_debug_log_to_file(event: &crate::LibObsActionEvent) {
    let path = if let Some(existing) = DEBUG_LOG_FILE_PATH.get() {
        existing.clone()
    } else {
        match init_debug_log_file() {
            Ok(p) => {
                let _ = DEBUG_LOG_FILE_PATH.set(p.clone());
                p
            }
            Err(_) => return,
        }
    };

    let detail = event
        .detail
        .as_ref()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "null".to_string());
    let line = format!("{} | {} | {}\n", event.timestamp_ms, event.action, detail);
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
}

pub(crate) fn push_debug_log_entry(action: String, detail: Option<serde_json::Value>) {
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let sanitized_detail = detail.map(sanitize_log_detail);

    let event = crate::LibObsActionEvent {
        timestamp_ms,
        action,
        detail: sanitized_detail,
    };

    write_debug_log_to_file(&event);

    if let Ok(mut entries) = debug_log_entries().lock() {
        entries.push(event.clone());
        if entries.len() > 1000 {
            let keep_from = entries.len().saturating_sub(1000);
            entries.drain(0..keep_from);
        }
    }
}

pub(crate) fn debug_console_log(
    action: String,
    detail: Option<serde_json::Value>,
) -> Result<(), String> {
    let action = action.trim().to_string();
    if action.is_empty() {
        return Err("action is required".to_string());
    }
    push_debug_log_entry(action, detail);
    Ok(())
}

pub(crate) fn debug_console_history() -> Result<Vec<crate::LibObsActionEvent>, String> {
    let entries = debug_log_entries()
        .lock()
        .map_err(|_| "state poisoned".to_string())?;
    Ok(entries.clone())
}

pub(crate) fn debug_console_clear() -> Result<(), String> {
    let mut entries = debug_log_entries()
        .lock()
        .map_err(|_| "state poisoned".to_string())?;
    entries.clear();
    Ok(())
}
