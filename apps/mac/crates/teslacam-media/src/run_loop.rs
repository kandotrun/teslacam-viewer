//! Blocking helpers that pump the current thread's run loop.
//!
//! For tests and command-line tools only. Inside the app the main run loop already runs; pumping it
//! from a GPUI callback re-enters the app.

use std::time::{Duration, Instant};

use objc2_core_foundation::{CFRunLoop, kCFRunLoopDefaultMode};

fn run_once() {
    // SAFETY: running the current thread's run loop in the default mode for 1 ms is always allowed.
    unsafe { CFRunLoop::run_in_mode(kCFRunLoopDefaultMode, 0.001, true) };
}

/// Pumps the run loop until `done` returns true; false if `timeout` elapsed first.
pub fn pump_until(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while !done() {
        if start.elapsed() > timeout {
            return false;
        }
        run_once();
    }
    true
}

/// Pumps the run loop for `duration`.
pub fn pump_for(duration: Duration) {
    let end = Instant::now() + duration;
    while Instant::now() < end {
        run_once();
    }
}
