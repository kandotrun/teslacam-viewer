//! AVFoundation integration tests. They need the process main thread (objc2 marks `AVPlayer`
//! main-thread-only), so this target uses `harness = false` and a tiny runner.
//!
//! `cargo test -p teslacam-media --test media_main_thread [-- <name filter>...]`

mod counter;
mod export;
mod fixtures;
mod playback;

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    time::Instant,
};

use objc2::MainThreadMarker;

type Test = (&'static str, fn(MainThreadMarker));

fn main() {
    let on_worker = std::thread::spawn(|| MainThreadMarker::new().is_some())
        .join()
        .unwrap();
    assert!(!on_worker, "worker threads must not get a MainThreadMarker");
    let mtm = MainThreadMarker::new().expect("harness = false runs on the main thread");

    let filters: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .collect();
    let tests: Vec<Test> = playback::TESTS
        .iter()
        .chain(export::TESTS)
        .copied()
        .collect();
    let selected: Vec<Test> = tests
        .into_iter()
        .filter(|(name, _)| filters.is_empty() || filters.iter().any(|f| name.contains(f.as_str())))
        .collect();
    fixtures::ensure();

    let mut failed = Vec::new();
    for (name, test) in &selected {
        let started = Instant::now();
        println!("test {name} ...");
        match catch_unwind(AssertUnwindSafe(|| test(mtm))) {
            Ok(()) => println!(
                "test {name} ... ok ({:.1} s)",
                started.elapsed().as_secs_f64()
            ),
            Err(_) => {
                println!("test {name} ... FAILED");
                failed.push(*name);
            }
        }
    }
    println!(
        "\ntest result: {}. {} passed; {} failed",
        if failed.is_empty() { "ok" } else { "FAILED" },
        selected.len() - failed.len(),
        failed.len()
    );
    if !failed.is_empty() {
        println!("failures: {failed:?}");
        std::process::exit(1);
    }
}
