//! Typed, bounded backend service boundary.

use std::{
    error::Error,
    fmt,
    sync::{
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveDirection {
    Up,
    Down,
    Top,
    Bottom,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendCommand {
    Undo,
    Redo,
    SetStudioMode(bool),
    SetRealtimeRefresh(bool),
    StartObs,
    StopObs,
    StartRecording,
    StopRecording,
    StartStreaming,
    StopStreaming,
    ForcePreviewResolution,
    SelectScene(String),
    RenameScene {
        from: String,
        to: String,
    },
    ReorderScene {
        name: String,
        to_index: usize,
    },
    SelectSource(String),
    ToggleSourceVisibility(String),
    ToggleSourceLock(String),
    MoveSource {
        id: String,
        direction: MoveDirection,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BackendSnapshot {
    pub obs_running: bool,
    pub recording: bool,
    pub streaming: bool,
    pub studio_mode: bool,
    pub realtime_refresh: bool,
    pub preview_resolution_requests: u64,
    pub selected_scene: Option<String>,
    pub selected_source: Option<String>,
    pub scenes: Vec<String>,
    pub sources: Vec<String>,
    pub visible_sources: Vec<String>,
    pub locked_sources: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendError {
    InvalidCapacity,
    QueueFull,
    Disconnected,
    WorkerPanicked,
    Runtime(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCapacity => formatter.write_str("backend queue capacity must be nonzero"),
            Self::QueueFull => formatter.write_str("backend request queue is full"),
            Self::Disconnected => formatter.write_str("backend service is disconnected"),
            Self::WorkerPanicked => formatter.write_str("backend service worker panicked"),
            Self::Runtime(message) => write!(formatter, "backend runtime error: {message}"),
        }
    }
}

impl Error for BackendError {}

/// Runtime implementation owned and called exclusively by the backend worker thread.
pub trait BackendRuntime: Send + 'static {
    fn execute(&mut self, command: BackendCommand) -> Result<(), BackendError>;
    fn snapshot(&self) -> Result<BackendSnapshot, BackendError>;

    fn shutdown(&mut self) -> Result<(), BackendError> {
        Ok(())
    }
}

enum Request {
    Dispatch(BackendCommand),
    Execute(BackendCommand, SyncSender<Result<(), BackendError>>),
    Snapshot(SyncSender<Result<BackendSnapshot, BackendError>>),
    Shutdown(SyncSender<Result<(), BackendError>>),
}

struct ServiceInner {
    sender: Mutex<Option<SyncSender<Request>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
    shutdown: Mutex<()>,
}

impl ServiceInner {
    fn sender(&self) -> Result<SyncSender<Request>, BackendError> {
        self.sender
            .lock()
            .expect("backend sender mutex poisoned")
            .clone()
            .ok_or(BackendError::Disconnected)
    }

    fn join_worker(&self) -> Result<(), BackendError> {
        let worker = self
            .worker
            .lock()
            .expect("backend worker mutex poisoned")
            .take();
        if let Some(worker) = worker {
            worker.join().map_err(|_| BackendError::WorkerPanicked)
        } else {
            Ok(())
        }
    }
}

impl Drop for ServiceInner {
    fn drop(&mut self) {
        self.sender
            .get_mut()
            .expect("backend sender mutex poisoned")
            .take();
        if let Some(worker) = self
            .worker
            .get_mut()
            .expect("backend worker mutex poisoned")
            .take()
        {
            let _ = worker.join();
        }
    }
}

/// Cloneable handle to a bounded backend worker.
#[derive(Clone)]
pub struct BackendService {
    inner: Arc<ServiceInner>,
}

impl BackendService {
    /// Starts a dedicated worker and constructs its runtime on that worker.
    pub fn new<R, F>(capacity: usize, create_runtime: F) -> Result<Self, BackendError>
    where
        R: BackendRuntime,
        F: FnOnce() -> R + Send + 'static,
    {
        if capacity == 0 {
            return Err(BackendError::InvalidCapacity);
        }

        let (sender, receiver) = mpsc::sync_channel(capacity);
        let worker = thread::Builder::new()
            .name("revo-backend-service".into())
            .spawn(move || run_worker(create_runtime(), receiver))
            .map_err(|error| BackendError::Runtime(format!("failed to start worker: {error}")))?;

        Ok(Self {
            inner: Arc::new(ServiceInner {
                sender: Mutex::new(Some(sender)),
                worker: Mutex::new(Some(worker)),
                shutdown: Mutex::new(()),
            }),
        })
    }

    /// Nonblocking command submission intended for foreground UI code.
    pub fn try_dispatch(&self, command: BackendCommand) -> Result<(), BackendError> {
        match self.inner.sender()?.try_send(Request::Dispatch(command)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(BackendError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(BackendError::Disconnected),
        }
    }

    /// Blocking command execution for tests and background threads only.
    pub fn execute(&self, command: BackendCommand) -> Result<(), BackendError> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        self.inner
            .sender()?
            .send(Request::Execute(command, reply_sender))
            .map_err(|_| BackendError::Disconnected)?;
        reply_receiver
            .recv()
            .map_err(|_| BackendError::Disconnected)?
    }

    /// Blocking snapshot request for tests and background threads only.
    pub fn snapshot(&self) -> Result<BackendSnapshot, BackendError> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        self.inner
            .sender()?
            .send(Request::Snapshot(reply_sender))
            .map_err(|_| BackendError::Disconnected)?;
        reply_receiver
            .recv()
            .map_err(|_| BackendError::Disconnected)?
    }

    /// Stops the worker cleanly. Repeated calls on any handle are idempotent.
    pub fn shutdown(&self) -> Result<(), BackendError> {
        let _shutdown = self
            .inner
            .shutdown
            .lock()
            .expect("backend shutdown mutex poisoned");
        if self
            .inner
            .worker
            .lock()
            .expect("backend worker mutex poisoned")
            .is_none()
        {
            return Ok(());
        }

        let sender = self.inner.sender()?;
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        let request_result = sender
            .send(Request::Shutdown(reply_sender))
            .map_err(|_| BackendError::Disconnected)
            .and_then(|()| {
                reply_receiver
                    .recv()
                    .map_err(|_| BackendError::Disconnected)?
            });
        self.inner
            .sender
            .lock()
            .expect("backend sender mutex poisoned")
            .take();
        let join_result = self.inner.join_worker();
        request_result.and(join_result)
    }
}

fn run_worker<R: BackendRuntime>(mut runtime: R, receiver: Receiver<Request>) {
    while let Ok(request) = receiver.recv() {
        match request {
            Request::Dispatch(command) => {
                let _ = runtime.execute(command);
            }
            Request::Execute(command, reply) => {
                let _ = reply.send(runtime.execute(command));
            }
            Request::Snapshot(reply) => {
                let _ = reply.send(runtime.snapshot());
            }
            Request::Shutdown(reply) => {
                let _ = reply.send(runtime.shutdown());
                return;
            }
        }
    }
    let _ = runtime.shutdown();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    };

    struct RecordingRuntime {
        snapshot: BackendSnapshot,
        commands: Vec<BackendCommand>,
        thread_ids: mpsc::Sender<(&'static str, thread::ThreadId)>,
        shutdowns: Arc<AtomicUsize>,
        fail_next: bool,
    }

    impl BackendRuntime for RecordingRuntime {
        fn execute(&mut self, command: BackendCommand) -> Result<(), BackendError> {
            let _ = self.thread_ids.send(("execute", thread::current().id()));
            if self.fail_next {
                self.fail_next = false;
                return Err(BackendError::Runtime("expected failure".into()));
            }
            match &command {
                BackendCommand::StartObs => self.snapshot.obs_running = true,
                BackendCommand::SelectScene(scene) => {
                    self.snapshot.selected_scene = Some(scene.clone())
                }
                BackendCommand::SelectSource(source) => {
                    self.snapshot.selected_source = Some(source.clone())
                }
                _ => {}
            }
            self.commands.push(command);
            self.snapshot.scenes = self
                .commands
                .iter()
                .map(|command| format!("{command:?}"))
                .collect();
            Ok(())
        }

        fn snapshot(&self) -> Result<BackendSnapshot, BackendError> {
            let _ = self.thread_ids.send(("snapshot", thread::current().id()));
            Ok(self.snapshot.clone())
        }

        fn shutdown(&mut self) -> Result<(), BackendError> {
            let _ = self.thread_ids.send(("shutdown", thread::current().id()));
            self.shutdowns.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    impl Drop for RecordingRuntime {
        fn drop(&mut self) {
            let _ = self.thread_ids.send(("drop", thread::current().id()));
        }
    }

    fn service(
        fail_next: bool,
    ) -> (
        BackendService,
        mpsc::Receiver<(&'static str, thread::ThreadId)>,
        Arc<AtomicUsize>,
    ) {
        let (thread_ids, receiver) = mpsc::channel();
        let shutdowns = Arc::new(AtomicUsize::new(0));
        let runtime_shutdowns = shutdowns.clone();
        let service = BackendService::new(4, move || {
            let _ = thread_ids.send(("construct", thread::current().id()));
            RecordingRuntime {
                snapshot: BackendSnapshot::default(),
                commands: Vec::new(),
                thread_ids,
                shutdowns: runtime_shutdowns,
                fail_next,
            }
        })
        .unwrap();
        (service, receiver, shutdowns)
    }

    #[test]
    fn runtime_lifecycle_stays_on_worker_thread() {
        let caller = thread::current().id();
        let (service, events, _) = service(false);
        service.execute(BackendCommand::StartObs).unwrap();
        service.snapshot().unwrap();
        service.shutdown().unwrap();

        let captured: Vec<_> = events.iter().collect();
        let worker = captured[0].1;
        assert_ne!(worker, caller);
        assert_eq!(
            captured.iter().map(|(event, _)| *event).collect::<Vec<_>>(),
            ["construct", "execute", "snapshot", "shutdown", "drop"]
        );
        assert!(captured.iter().all(|(_, id)| *id == worker));
    }

    #[test]
    fn commands_are_ordered_and_snapshot_reflects_results() {
        let (service, _, _) = service(false);
        service.execute(BackendCommand::StartObs).unwrap();
        service
            .execute(BackendCommand::SelectScene("Main".into()))
            .unwrap();
        service
            .execute(BackendCommand::SelectSource("camera".into()))
            .unwrap();

        let snapshot = service.snapshot().unwrap();
        assert!(snapshot.obs_running);
        assert_eq!(snapshot.selected_scene.as_deref(), Some("Main"));
        assert_eq!(snapshot.selected_source.as_deref(), Some("camera"));
        assert_eq!(
            snapshot.scenes,
            [
                "StartObs",
                "SelectScene(\"Main\")",
                "SelectSource(\"camera\")"
            ]
        );
    }

    #[test]
    fn runtime_errors_propagate_to_blocking_callers() {
        let (service, _, _) = service(true);
        assert_eq!(
            service.execute(BackendCommand::StartObs),
            Err(BackendError::Runtime("expected failure".into()))
        );
    }

    #[test]
    fn queue_capacity_is_enforced_without_waiting() {
        struct BlockingRuntime {
            entered: mpsc::SyncSender<()>,
            release: mpsc::Receiver<()>,
            blocked_once: bool,
        }

        impl BackendRuntime for BlockingRuntime {
            fn execute(&mut self, _: BackendCommand) -> Result<(), BackendError> {
                if !self.blocked_once {
                    self.blocked_once = true;
                    self.entered.send(()).unwrap();
                    self.release.recv().unwrap();
                }
                Ok(())
            }

            fn snapshot(&self) -> Result<BackendSnapshot, BackendError> {
                Ok(BackendSnapshot::default())
            }
        }

        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let service = BackendService::new(1, move || BlockingRuntime {
            entered: entered_tx,
            release: release_rx,
            blocked_once: false,
        })
        .unwrap();
        service.try_dispatch(BackendCommand::Undo).unwrap();
        entered_rx.recv().unwrap();
        service.try_dispatch(BackendCommand::Redo).unwrap();
        assert_eq!(
            service.try_dispatch(BackendCommand::StartObs),
            Err(BackendError::QueueFull)
        );
        release_tx.send(()).unwrap();
        service.shutdown().unwrap();
    }

    #[test]
    fn shutdown_is_idempotent_and_hook_runs_once() {
        let (service, _, shutdowns) = service(false);
        let second_handle = service.clone();
        service.shutdown().unwrap();
        second_handle.shutdown().unwrap();
        assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
        assert_eq!(
            service.try_dispatch(BackendCommand::Undo),
            Err(BackendError::Disconnected)
        );
    }

    #[test]
    fn dropping_all_handles_stops_worker_and_calls_shutdown() {
        let (service, events, shutdowns) = service(false);
        let second_handle = service.clone();
        drop(service);
        assert_eq!(shutdowns.load(Ordering::SeqCst), 0);
        drop(second_handle);
        assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
        assert_eq!(events.iter().last().unwrap().0, "drop");
    }

    #[test]
    fn zero_capacity_is_rejected() {
        struct Runtime;
        impl BackendRuntime for Runtime {
            fn execute(&mut self, _: BackendCommand) -> Result<(), BackendError> {
                Ok(())
            }
            fn snapshot(&self) -> Result<BackendSnapshot, BackendError> {
                Ok(BackendSnapshot::default())
            }
        }

        assert!(matches!(
            BackendService::new(0, || Runtime),
            Err(BackendError::InvalidCapacity)
        ));
    }
}
