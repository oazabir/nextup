use std::{process::Command, thread, time::Duration};

use objc2::rc::autoreleasepool;
use objc2_app_kit::NSWorkspace;

use super::Notify;

const CHROME: &str = "com.google.Chrome";
const ACTIVE_TAB: &str = r#"
tell application "__CHROME_PATH__"
    with timeout of 2 seconds
        if not running then return ""
        if (count of windows) is 0 then return ""
        set chromeWindow to front window
        return ((id of chromeWindow) as text) & ":" & ((id of active tab of chromeWindow) as text)
    end timeout
end tell
"#;

fn foreground_chrome() -> Option<(i32, String)> {
    autoreleasepool(|_| {
        let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        let bundle = app.bundleIdentifier()?;
        if bundle.to_string() != CHROME {
            return None;
        }
        Some((
            app.processIdentifier(),
            app.bundleURL()?.path()?.to_string(),
        ))
    })
}

#[derive(PartialEq, Eq)]
struct Tab {
    pid: i32,
    window: String,
    id: String,
}

impl Tab {
    fn parse(pid: i32, output: &[u8]) -> Option<Self> {
        let (window, id) = std::str::from_utf8(output).ok()?.trim().split_once(':')?;
        if window.is_empty() || id.is_empty() {
            return None;
        }
        Some(Self {
            pid,
            window: window.to_owned(),
            id: id.to_owned(),
        })
    }

    fn switched_from(&self, previous: &Self) -> bool {
        self.pid == previous.pid && self.window == previous.window && self.id != previous.id
    }
}

pub(super) fn watch(notify: Notify) {
    let mut previous = None;
    loop {
        let mut current = None;
        let mut wait = Duration::from_millis(650);
        if let Some((pid, path)) = foreground_chrome() {
            let path = path.replace('\\', "\\\\").replace('"', "\\\"");
            let script = ACTIVE_TAB.replace("__CHROME_PATH__", &path);
            match Command::new("/usr/bin/osascript")
                .args(["-e", &script])
                .output()
            {
                Ok(result) if result.status.success() => {
                    if foreground_chrome().is_some_and(|(current_pid, _)| current_pid == pid) {
                        current = Tab::parse(pid, &result.stdout);
                    }
                }
                // Retry after a pause so a change in Automation settings takes effect.
                _ => wait = Duration::from_secs(15),
            }
        }
        if current
            .as_ref()
            .zip(previous.as_ref())
            .is_some_and(|(tab, prior)| tab.switched_from(prior))
        {
            notify();
        }
        previous = current;
        thread::sleep(wait);
    }
}

#[cfg(test)]
mod tests {
    use super::Tab;

    #[test]
    fn detects_only_a_different_tab_in_the_same_chrome_window() {
        let previous = Tab::parse(1, b"20:42\n").unwrap();
        assert!(Tab::parse(1, b"20:43").unwrap().switched_from(&previous));
        assert!(!Tab::parse(1, b"20:42").unwrap().switched_from(&previous));
        assert!(!Tab::parse(1, b"21:43").unwrap().switched_from(&previous));
        assert!(!Tab::parse(2, b"20:43").unwrap().switched_from(&previous));
        assert!(Tab::parse(1, b"").is_none());
    }
}
