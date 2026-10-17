//! Read-only mutex wait/hold diagnostics.

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

const WAIT_WARN: Duration = Duration::from_millis(50);
const HOLD_WARN: Duration = Duration::from_millis(250);
const WARN_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Default)]
struct OpStats {
	acquires: u64,
	last_wait_us: u64,
	max_wait_us: u64,
	last_hold_us: u64,
	max_hold_us: u64,
}

#[derive(Clone)]
struct Holder {
	thread: String,
	since: Instant,
}

#[derive(Default)]
struct ProbeState {
	ops: HashMap<&'static str, OpStats>,
	current: HashMap<&'static str, Holder>,
	last_warn: HashMap<&'static str, Instant>,
}

fn state() -> &'static Mutex<ProbeState> {
	static STATE: OnceLock<Mutex<ProbeState>> = OnceLock::new();
	STATE.get_or_init(|| Mutex::new(ProbeState::default()))
}

fn with_state<T>(f: impl FnOnce(&mut ProbeState) -> T) -> T {
	let mut guard = state().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
	f(&mut guard)
}

fn thread_label() -> String {
	let current = std::thread::current();
	format!("{} ({:?})", current.name().unwrap_or("<unnamed>"), current.id())
}

fn can_warn(state: &mut ProbeState, op: &'static str, now: Instant) -> bool {
	if state.last_warn.get(op).is_some_and(|last| now.duration_since(*last) < WARN_INTERVAL) {
		return false;
	}
	state.last_warn.insert(op, now);
	true
}

fn acquired(op: &'static str, wait: Duration) {
	let now = Instant::now();
	let mut warn = false;
	with_state(|state| {
		let stats = state.ops.entry(op).or_default();
		stats.acquires += 1;
		stats.last_wait_us = wait.as_micros() as u64;
		stats.max_wait_us = stats.max_wait_us.max(stats.last_wait_us);
		state.current.insert(op, Holder { thread: thread_label(), since: now });
		if wait >= WAIT_WARN {
			warn = can_warn(state, op, now);
		}
	});
	if warn {
		eprintln!("obs: lock-wait: {op} waited {}us", wait.as_micros());
	}
}

fn released(op: &'static str, wait: Duration, hold: Duration) {
	let now = Instant::now();
	let mut warn = false;
	with_state(|state| {
		let stats = state.ops.entry(op).or_default();
		stats.last_hold_us = hold.as_micros() as u64;
		stats.max_hold_us = stats.max_hold_us.max(stats.last_hold_us);
		state.current.remove(op);
		if hold >= HOLD_WARN {
			warn = can_warn(state, op, now);
		}
	});
	if warn {
		let detail = serde_json::json!({
			"op": op,
			"wait_us": wait.as_micros() as u64,
			"held_us": hold.as_micros() as u64,
			"thread": thread_label(),
		});
		eprintln!("obs: lock-hold: {op} held {}us", hold.as_micros());
		crate::logging::debug::push_debug_log_entry("lock-hold".to_string(), Some(detail));
	}
}

pub(crate) struct ProbeGuard<'a, T> {
	guard: Option<MutexGuard<'a, T>>,
	op: &'static str,
	wait: Duration,
	started: Instant,
}

impl<T> Deref for ProbeGuard<'_, T> {
	type Target = T;
	fn deref(&self) -> &T {
		self.guard.as_ref().expect("probe guard released").deref()
	}
}

impl<T> DerefMut for ProbeGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut T {
		self.guard.as_mut().expect("probe guard released").deref_mut()
	}
}

impl<T> Drop for ProbeGuard<'_, T> {
	fn drop(&mut self) {
		let hold = self.started.elapsed();
		drop(self.guard.take());
		released(self.op, self.wait, hold);
	}
}

pub(crate) fn probe_lock<'a, T>(
	mutex: &'a Mutex<T>,
	op: &'static str,
) -> Result<ProbeGuard<'a, T>, std::sync::PoisonError<MutexGuard<'a, T>>> {
	let started = Instant::now();
	let guard = mutex.lock()?;
	let wait = started.elapsed();
	acquired(op, wait);
	Ok(ProbeGuard { guard: Some(guard), op, wait, started: Instant::now() })
}

pub(crate) fn snapshot() -> serde_json::Value {
	let now = Instant::now();
	with_state(|state| {
		let ops: serde_json::Map<String, serde_json::Value> = state.ops.iter().map(|(op, stats)| {
			((*op).to_string(), serde_json::json!({
				"acquires": stats.acquires,
				"last_wait_us": stats.last_wait_us,
				"max_wait_us": stats.max_wait_us,
				"last_hold_us": stats.last_hold_us,
				"max_hold_us": stats.max_hold_us,
			}))
		}).collect();
		let currently_held: Vec<_> = state.current.iter().map(|(op, holder)| serde_json::json!({
			"op": op,
			"thread": holder.thread,
			"held_ms": now.duration_since(holder.since).as_millis() as u64,
		})).collect();
		serde_json::json!({ "ops": ops, "currently_held": currently_held })
	})
}

#[cfg(test)]
mod tests {
	#[test]
	fn records_wait_and_hold_metrics() {
		let mutex = std::sync::Mutex::new(1u32);
		{
			let mut guard = super::probe_lock(&mutex, "probe.test").expect("lock");
			*guard += 1;
		}
		let snapshot = super::snapshot();
		assert_eq!(snapshot["ops"]["probe.test"]["acquires"], 1);
	}
}
