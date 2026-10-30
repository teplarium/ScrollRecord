//! Long jobs: a job id now, progress later, and a way to stop (ADR 0006 §4).
//!
//! Purpose: work that takes seconds (reading the site for the site map; later
//!          renders) must not block an IPC call. A command starts the work on
//!          its own thread and returns a job id at once; the work reports over
//!          a Tauri `Channel`; `cancel_job(id)` raises the job's cancel flag,
//!          which the library checks between steps (SR-0004).
//! Rules:   plumbing only — what a job does lives in a library crate. Every
//!          job is removed from the list when it ends; app exit cancels all.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// Running jobs and their cancel flags.
#[derive(Default)]
pub struct Jobs {
    next: AtomicU64,
    running: Mutex<HashMap<u64, Arc<AtomicBool>>>,
}

impl Jobs {
    /// Registers a new job: its id and the flag it must check.
    pub fn start(&self) -> (u64, Arc<AtomicBool>) {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let flag = Arc::new(AtomicBool::new(false));
        self.running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id, Arc::clone(&flag));
        (id, flag)
    }

    /// Asks job `id` to stop; unknown or finished ids are ignored.
    pub fn cancel(&self, id: u64) {
        if let Some(flag) = self
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&id)
        {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Removes a job that ended.
    pub fn finish(&self, id: u64) {
        self.running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id);
    }

    /// App exit: ask every job to stop (their browsers close with them).
    pub fn cancel_all(&self) {
        for flag in self
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
        {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_reaches_only_its_running_job() {
        let jobs = Jobs::default();
        let (a, flag_a) = jobs.start();
        let (b, flag_b) = jobs.start();
        assert_ne!(a, b);
        jobs.cancel(a);
        assert!(flag_a.load(Ordering::Relaxed));
        assert!(!flag_b.load(Ordering::Relaxed));

        jobs.finish(b);
        jobs.cancel(b); // finished: ignored
        assert!(!flag_b.load(Ordering::Relaxed));
        jobs.cancel(999); // unknown: ignored

        let (_, flag_c) = jobs.start();
        jobs.cancel_all();
        assert!(flag_c.load(Ordering::Relaxed));
    }
}
