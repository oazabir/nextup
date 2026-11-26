#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::Instant;
use std::{
    sync::{Arc, Mutex},
    thread,
};

use tauri::{Manager, WebviewWindow};

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
    let window_switch: Notify = Arc::new({
        let window = window.clone();
        move || {
            let _ = window.eval("window.dispatchEvent(new Event('desktop-window-switch'))");
        }
    });
    #[cfg(target_os = "macos")]
    let last_pulse = Mutex::new(None::<Instant>);
    #[cfg(target_os = "macos")]
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
        thread::spawn(move || browser::watch(notify, preference));
    }
    thread::spawn(move || watch_windows(window_switch));
}

#[derive(Default)]
struct RecentWindows {
    current: Option<usize>,
    previous: Option<usize>,
}

impl RecentWindows {
    fn visit(&mut self, next: usize) -> bool {
        if self.current == Some(next) {
            return false;
        }
        let is_new = self.current.is_some() && self.previous != Some(next);
        self.previous = self.current.replace(next);
        is_new
    }
}

pub fn create_overlay(app: &tauri::App) -> tauri::Result<()> {
    let overlay = tauri::WebviewWindowBuilder::new(
        app,
        "task-reminder",
        tauri::WebviewUrl::App("task-reminder.html".into()),
    )
    .visible(false)
    .focused(false)
    .focusable(false)
    .decorations(false)
    .shadow(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .build()?;
    if let Err(error) = overlay.set_ignore_cursor_events(true) {
        let _ = overlay.destroy();
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
pub fn show_task_reminder(window: WebviewWindow, title: String) -> Result<(), String> {
    let Some(overlay) = window.get_webview_window("task-reminder") else {
        return Ok(());
    };
    let monitor = window
        .cursor_position()
        .ok()
        .and_then(|point| window.monitor_from_point(point.x, point.y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        let width = area.size.width.min((1100.0 * scale) as u32);
        let height = area.size.height.min((300.0 * scale) as u32);
        overlay
            .set_size(tauri::PhysicalSize::new(width, height))
            .map_err(|e| e.to_string())?;
        overlay
            .set_position(tauri::PhysicalPosition::new(
                area.position.x + ((area.size.width - width) / 2) as i32,
                area.position.y + ((area.size.height - height) / 2) as i32,
            ))
            .map_err(|e| e.to_string())?;
    }
    let title = serde_json::to_string(&title).map_err(|e| e.to_string())?;
    overlay
        .eval(format!("window.showReminder({title})"))
        .map_err(|e| e.to_string())?;
    overlay.show().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn hide_task_reminder(window: WebviewWindow) -> Result<(), String> {
    if window.label() == "task-reminder" {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn watch_windows(notify: Notify) {
    let mut recent = RecentWindows::default();
    loop {
        if let Some(current) = foreground_window() {
            if recent.visit(current) {
                notify();
            }
        }
        thread::sleep(Duration::from_millis(350));
    }
}

#[cfg(target_os = "macos")]
fn foreground_window() -> Option<usize> {
    use objc2::rc::autoreleasepool;
    use objc2_app_kit::NSWorkspace;
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
    let active_pid = autoreleasepool(|_| {
        Some(
            NSWorkspace::sharedWorkspace()
                .frontmostApplication()?
                .processIdentifier(),
        )
    })?;
    if active_pid as u32 == std::process::id() {
        return None;
    }
    unsafe {
        // On-screen windows, front to back, excluding desktop elements.
        let windows = CGWindowListCopyWindowInfo(1 | 16, 0);
        if windows.is_null() {
            return None;
        }
        let front = (0..CFArrayGetCount(windows)).find_map(|index| {
            let window = CFArrayGetValueAtIndex(windows, index);
            if number(window, kCGWindowLayer)? != 0
                || number(window, kCGWindowOwnerPID)? != i64::from(active_pid)
            {
                return None;
            }
            number(window, kCGWindowNumber).map(|id| id as usize)
        });
        CFRelease(windows);
        front
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

    let mut recent = RecentWindows::default();
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
        if usize::from_str_radix(&id[2..], 16).is_ok_and(|id| recent.visit(id)) {
            notify();
        }
    }
    let _ = watcher.wait();
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn watch_windows(_: Notify) {}

#[cfg(test)]
mod tests {
    use super::RecentWindows;

    #[test]
    fn reminds_only_when_leaving_the_current_pair() {
        let mut recent = RecentWindows::default();
        let reminders = [1, 1, 2, 1, 2, 2, 3, 2, 3, 1, 3].map(|window| recent.visit(window));
        assert_eq!(
            reminders,
            [false, false, true, false, false, false, true, false, false, true, false]
        );
    }
}
