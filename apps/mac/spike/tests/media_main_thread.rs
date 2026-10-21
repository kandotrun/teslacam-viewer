//! Q9: AVFoundation needs the main thread (objc2 marks AVPlayer/AVPlayerItem
//! MainThreadOnly), but libtest runs tests on worker threads. This target uses
//! `harness = false` so `main` runs on the process main thread.
use std::time::Duration;

use teslacam_mac_spike::{
    av::{self, SyncGroup, VideoTile, pump_until},
    fixtures,
};

fn main() {
    // Prove the default harness would not work: worker threads have no marker.
    let on_worker = std::thread::spawn(|| objc2::MainThreadMarker::new().is_some()).join().unwrap();
    assert!(!on_worker, "worker threads must not get a MainThreadMarker");

    let mtm = objc2::MainThreadMarker::new().expect("harness=false runs on the main thread");
    let composition = av::event_composition(&fixtures::camera_segments(3)).expect("composition");
    let mut group = SyncGroup::new(vec![VideoTile::new(&composition, mtm)]);
    group.use_shared_host_clock();
    assert!(group.wait_ready(Duration::from_secs(10)));
    group.seek_exact(61.5, Duration::from_secs(5)).expect("seek");
    let tile = &mut group.tiles[0];
    assert!(pump_until(Duration::from_secs(2), || tile.pull_current()));
    let stamp = av::decode_stamp(tile.current().unwrap()).expect("decode");
    assert_eq!(stamp.camera, 3);
    assert_eq!(stamp.frame, (61.5 * 36.0) as u32);
    println!("media_main_thread: ok (frame {} camera {})", stamp.frame, stamp.camera);
}
