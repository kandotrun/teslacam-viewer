//! Q8 helper: print the CGWindowID(s) owned by a PID (for `screencapture -l`).
use objc2_core_foundation::CFString;
use objc2_core_graphics::{
    CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID, kCGWindowName,
    kCGWindowLayer, kCGWindowNumber, kCGWindowOwnerPID,
};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};

fn key(k: &CFString) -> &NSString {
    unsafe { &*(k as *const CFString as *const NSString) }
}

fn main() {
    let pid: i64 = std::env::args().nth(1).expect("pid").parse().unwrap();
    let list = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, kCGNullWindowID)
        .expect("window list");
    let list: &NSArray<NSDictionary<NSString, objc2::runtime::AnyObject>> =
        unsafe { &*(&*list as *const _ as *const NSArray<_>) };
    for dict in list.iter() {
        let get_num = |k: &CFString| {
            dict.objectForKey(key(k))
                .and_then(|o| o.downcast::<NSNumber>().ok())
                .map(|n| n.as_i64())
        };
        if get_num(unsafe { kCGWindowOwnerPID }) == Some(pid) && get_num(unsafe { kCGWindowLayer }) == Some(0) {
            let name = dict
                .objectForKey(key(unsafe { kCGWindowName }))
                .and_then(|o| o.downcast::<NSString>().ok())
                .map(|s| s.to_string())
                .unwrap_or_default();
            println!("{} {}", get_num(unsafe { kCGWindowNumber }).unwrap_or(-1), name);
        }
    }
}
