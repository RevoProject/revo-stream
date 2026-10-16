//! Rate-limited step tracer for libobs hang/freeze diagnosis.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const LOG_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Copy)]
struct ScopeState {
	step: &'static str,
	at: Instant,
	last_log_at: Instant,
}

static SCOPES: OnceLock<Mutex<HashMap<String, ScopeState>>> = OnceLock::new();

fn with_scopes<T>(f: impl FnOnce(&mut HashMap<String, ScopeState>) -> T) -> T {
	let mut guard = SCOPES
		.get_or_init(|| Mutex::new(HashMap::new()))
		.lock()
		.unwrap_or_else(|poisoned| poisoned.into_inner());
	f(&mut guard)
}

pub(crate) fn step(scope: &'static str, step: &'static str) {
	let now = Instant::now();
	let should_log = with_scopes(|map| {
		let state = map.entry(scope.to_string()).or_insert(ScopeState {
			step,
			at: now,
			last_log_at: now.checked_sub(LOG_INTERVAL).unwrap_or(now),
		});
		let emit = now.duration_since(state.last_log_at) >= LOG_INTERVAL;
		if emit {
			state.last_log_at = now;
		}
		state.step = step;
		state.at = now;
		emit
	});
	if should_log {
		crate::logging::debug::push_debug_log_entry(
			"libobs-trace".to_string(),
			Some(serde_json::json!({ "scope": scope, "step": step })),
		);
	}
}

pub(crate) fn last() -> Option<(String, u64)> {
	with_scopes(|map| {
		map.values()
			.max_by_key(|state| state.at)
			.map(|state| (state.step.to_string(), state.at.elapsed().as_millis() as u64))
	})
}

pub(crate) fn snapshot() -> Vec<serde_json::Value> {
	with_scopes(|map| {
		let mut out: Vec<_> = map
			.iter()
			.map(|(scope, state)| {
				serde_json::json!({
					"scope": scope,
					"step": state.step,
					"age_ms": state.at.elapsed().as_millis() as u64,
				})
			})
			.collect();
		out.sort_by_key(|entry| entry["age_ms"].as_u64().unwrap_or(u64::MAX));
		out
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn records_latest_trace_step() {
		step("trace-test", "entered");
		step("trace-test", "done");
		let (name, age) = last().expect("trace step");
		assert_eq!(name, "done");
		assert!(age < 60_000);
		assert!(snapshot().iter().any(|entry| entry["scope"] == "trace-test"));
	}
}
