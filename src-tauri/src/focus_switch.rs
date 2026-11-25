use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "macos")]
use tauri::Manager;
use tauri::WebviewWindow;

#[cfg(target_os = "macos")]
mod browser;

#[derive(Clone, Copy, PartialEq, Eq)]
enum WorkBrowser {
    Chrome,
    Safari,
}

#[derive(Clone, Default)]
pub struct BrowserPreference(Arc<Mutex<Option<WorkBrowser>>>);

#[tauri::command]
pub fn set_work_browser(browser: &str, preference: tauri::State<'_, BrowserPreference>) {
    let selected = match browser {
        "chrome" => Some(WorkBrowser::Chrome),
        "safari" => Some(WorkBrowser::Safari),
        _ => None,
    };
    if let Ok(mut current) = preference.0.lock() {
        *current = selected;
    }
}

type Notify = Arc<dyn Fn() + Send + Sync>;

pub fn start(window: WebviewWindow) {
    #[cfg(target_os = "macos")]
    let preference = window.state::<BrowserPreference>().inner().clone();
    let last_pulse = Mutex::new(None::<Instant>);
    let notify: Notify = Arc::new(move || {
        let Ok(mut last) = last_pulse.lock() else {
            return;
        };
        let now = Instant::now();
        if last.is_some_and(|time| now.duration_since(time) < Duration::from_millis(750)) {
            return;
        }
        *last = Some(now);
        let _ = window.eval("window.dispatchEvent(new Event('desktop-focus-switch'))");
    });

    #[cfg(target_os = "macos")]
    {
        let browser_notify = Arc::clone(&notify);
        thread::spawn(move || browser::watch(browser_notify, preference));
    }
    thread::spawn(move || watch_windows(notify));
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn watch_windows(notify: Notify) {
    let mut previous = None;
    loop {
        if let Some(current) = foreground_window() {
            if previous
                .replace(current)
                .is_some_and(|last| last != current)
            {
                notify();
            }
        }
        thread::sleep(Duration::from_millis(350));
    }
}

#[cfg(target_os = "macos")]
fn foreground_window() -> Option<usize> {
    use std::ffi::c_void;
    type Ref = *const c_void;
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGWindowListCopyWindowInfo(options: u32, relative_to: u32) -> Ref;
        static kCGWindowLayer: Ref;
        static kCGWindowNumber: Ref;
        static kCGWindowOwnerPID: Ref;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFArrayGetCount(array: Ref) -> isize;
        fn CFArrayGetValueAtIndex(array: Ref, index: isize) -> Ref;
        fn CFDictionaryGetValue(dictionary: Ref, key: Ref) -> Ref;
        fn CFNumberGetValue(number: Ref, number_type: isize, value: *mut i64) -> u8;
        fn CFRelease(value: Ref);
    }
    fn number(dictionary: Ref, key: Ref) -> Option<i64> {
        let mut value = 0;
        // kCFNumberSInt64Type
        unsafe {
            let number = CFDictionaryGetValue(dictionary, key);
            (!number.is_null() && CFNumberGetValue(number, 4, &mut value) != 0).then_some(value)
        }
    }
    unsafe {
        // On-screen windows, front to back, excluding desktop elements.
        let windows = CGWindowListCopyWindowInfo(1 | 16, 0);
        if windows.is_null() {
            return None;
        }
        let front = (0..CFArrayGetCount(windows)).find_map(|index| {
            let window = CFArrayGetValueAtIndex(windows, index);
            if number(window, kCGWindowLayer)? != 0 {
                return None;
            }
            Some((
                number(window, kCGWindowOwnerPID)?,
                number(window, kCGWindowNumber)?,
            ))
        });
        CFRelease(windows);
        front.and_then(|(pid, id)| (pid != i64::from(std::process::id())).then_some(id as usize))
    }
}

#[cfg(target_os = "windows")]
fn foreground_window() -> Option<usize> {
    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> isize;
        fn GetWindowThreadProcessId(window: isize, pid: *mut u32) -> u32;
    }

    unsafe {
        let window = GetForegroundWindow();
        let mut pid = 0;
        (window != 0
            && GetWindowThreadProcessId(window, &mut pid) != 0
            && pid != std::process::id())
        .then_some(window as usize)
    }
}

#[cfg(target_os = "linux")]
fn watch_windows(notify: Notify) {
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
    };

    if std::env::var_os("DISPLAY").is_none() {
        return;
    }
    let Ok(mut watcher) = Command::new("xprop")
        .args(["-root", "-spy", "_NET_ACTIVE_WINDOW"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };

    let mut previous = None;
    for line in BufReader::new(watcher.stdout.take().unwrap())
        .lines()
        .map_while(Result::ok)
    {
        let Some(id) = line
            .split_whitespace()
            .last()
            .filter(|id| id.starts_with("0x") && *id != "0x0")
        else {
            continue;
        };
        let Ok(owner) = Command::new("xprop")
            .args(["-id", id, "_NET_WM_PID"])
            .output()
        else {
            continue;
        };
        let pid = String::from_utf8_lossy(&owner.stdout)
            .split_whitespace()
            .last()
            .and_then(|pid| pid.parse::<u32>().ok());
        if pid == Some(std::process::id()) {
            continue;
        }
        if previous
            .replace(id.to_owned())
            .is_some_and(|last| last != id)
        {
            notify();
        }
    }
    let _ = watcher.wait();
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn watch_windows(_: Notify) {}
