//! Times the composite export of a fixture event with both pipelines, alternating them so that both
//! see the same machine load.
//!
//! ```text
//! just mac-fixtures
//! cargo run --release -p teslacam-media --example export_bench -- [runs] [full|gaps] [output dir]
//! ```
//!
//! Prints one line per export and the median of each pipeline. The exported files stay in the
//! output directory (default: the system temporary directory).

use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use objc2::MainThreadMarker;
use teslacam_media::{
    CameraMap, CompositeExporter, EventInput, ExportBackend, ExportState, SegmentInput,
    run_loop::pump_until,
};

fn saved_clips() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.fixtures/TeslaCam/SavedClips")
}

fn event(dir: &str, segments: &[(&str, f64)]) -> EventInput {
    let dir = saved_clips().join(dir);
    EventInput {
        segments: segments
            .iter()
            .map(|(timestamp, start)| SegmentInput {
                start_seconds: *start,
                files: CameraMap::from_fn(|camera| {
                    let path = dir.join(format!("{timestamp}-{}.mp4", camera.id()));
                    path.exists().then_some(path)
                }),
            })
            .collect(),
    }
}

fn export(backend: ExportBackend, event: &EventInput, output: &Path, mtm: MainThreadMarker) -> f64 {
    let started = Instant::now();
    let exporter = CompositeExporter::new().with_backend(backend);
    let mut job = exporter.start(event, output, |_| {}, mtm);
    assert!(
        pump_until(Duration::from_secs(600), || job.poll().is_done()),
        "export did not finish"
    );
    let elapsed = started.elapsed().as_secs_f64();
    match job.state() {
        ExportState::Finished { duration, .. } => println!(
            "{backend:?}: {duration:.0} s exported in {elapsed:.2} s ({:.1}x real time)",
            duration / elapsed
        ),
        state => panic!("{backend:?}: {state:?}"),
    }
    elapsed
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

fn main() {
    let mtm = MainThreadMarker::new().expect("runs on the main thread");
    let mut args = std::env::args().skip(1);
    let runs: usize = args.next().map_or(3, |n| n.parse().expect("runs"));
    let event = match args.next().as_deref().unwrap_or("full") {
        "full" => event(
            "2026-10-20_12-01-30",
            &[("2026-10-20_12-00-00", 0.0), ("2026-10-20_12-01-00", 60.0)],
        ),
        "gaps" => event(
            "2026-10-20_13-00-50",
            &[
                ("2026-10-20_13-00-00", 0.0),
                ("2026-10-20_13-00-20", 20.0),
                ("2026-10-20_13-00-40", 40.0),
            ],
        ),
        other => panic!("unknown event `{other}` (full or gaps)"),
    };
    let dir = args.next().map_or_else(std::env::temp_dir, PathBuf::from);
    let backends = [ExportBackend::ExportSession, ExportBackend::ReaderWriter];
    let mut times: Vec<Vec<f64>> = vec![Vec::new(); backends.len()];
    for run in 0..runs {
        for (index, backend) in backends.iter().enumerate() {
            let output = dir.join(format!("export-bench-{backend:?}-{run}.mp4"));
            times[index].push(export(*backend, &event, &output, mtm));
        }
    }
    let medians: Vec<f64> = times.into_iter().map(median).collect();
    println!(
        "median of {runs}: ExportSession {:.2} s, ReaderWriter {:.2} s ({:.2}x faster)",
        medians[0],
        medians[1],
        medians[0] / medians[1]
    );
}
