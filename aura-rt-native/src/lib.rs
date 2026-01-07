#![forbid(unsafe_code)]

use std::sync::mpsc;

pub mod allocator;

/// Minimal native runtime facade for `~>`.
///
/// Phase 3 goal: provide a stable ABI surface for the compiler backend.
/// Implementation uses Rayon as a work-stealing scheduler.
pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    rayon::spawn(move || {
        let _ = tx.send(f());
    });
    JoinHandle { rx }
}

pub struct JoinHandle<T> {
    rx: mpsc::Receiver<T>,
}

impl<T> JoinHandle<T> {
    pub fn join(self) -> T {
        self.rx.recv().expect("task panicked")
    }
}
