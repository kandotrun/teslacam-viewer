//! Times `scan_library` on a folder: walk, classification and parallel `event.json` reads.
//!
//! ```text
//! cargo run --release -p teslacam-core --example scan_bench -- <TeslaCam folder> [runs]
//! ```

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use teslacam_core::scan::scan_library;

fn main() {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().expect("usage: scan_bench <folder> [runs]"));
    let runs: usize = args.next().map_or(5, |runs| runs.parse().expect("runs must be a number"));
    let cancel = AtomicBool::new(false);
    let mut millis = Vec::with_capacity(runs);
    for run in 1..=runs {
        let started = Instant::now();
        let scanned = scan_library(&root, &mut |_| {}, &cancel).expect("scan failed");
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        millis.push(elapsed);
        println!(
            "run {run}: {elapsed:.1} ms, {} files, {} events",
            scanned.library.files.len(),
            scanned.library.events.len()
        );
    }
    millis.sort_by(f64::total_cmp);
    println!("median {:.1} ms (min {:.1}, max {:.1})", millis[runs / 2], millis[0], millis[runs - 1]);
}
