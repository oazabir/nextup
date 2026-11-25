use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const MAX_SAMPLE: Duration = Duration::from_millis(2500);

#[derive(Default)]
pub struct AwakeClock(Arc<AtomicU64>);

impl AwakeClock {
    pub fn start(&self) {
        let total = Arc::clone(&self.0);
        thread::spawn(move || {
            let mut previous = (Instant::now(), SystemTime::now(), display_awake());
            let mut wake = WakeSamples::default();
            loop {
                thread::sleep(SAMPLE_INTERVAL);
                let current = (Instant::now(), SystemTime::now(), display_awake());
                let elapsed = current.0 - previous.0;
                let wall = current.1.duration_since(previous.1).unwrap_or(elapsed);
                let credited = wake.record(elapsed, wall, previous.2 && current.2);
                total.fetch_add(credited.as_millis() as u64, Ordering::Relaxed);
                previous = current;
            }
        });
    }

    pub fn millis(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

#[derive(Default)]
struct WakeSamples {
    pending: Option<Duration>,
}

impl WakeSamples {
    fn record(&mut self, elapsed: Duration, wall: Duration, display_awake: bool) -> Duration {
        if !display_awake || elapsed > MAX_SAMPLE || wall > MAX_SAMPLE {
            self.pending = Some(Duration::ZERO);
            return Duration::ZERO;
        }
        if let Some(pending) = &mut self.pending {
            *pending += elapsed;
            // Filter brief maintenance wakes on systems where display power is unavailable.
            if !cfg!(target_os = "macos") && *pending < Duration::from_secs(15) {
                return Duration::ZERO;
            }
            self.pending = None;
        }
        elapsed
    }
}

#[cfg(target_os = "macos")]
fn display_awake() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGMainDisplayID() -> u32;
        fn CGDisplayIsAsleep(display: u32) -> u32;
    }
    // Core Graphics allows display state queries from background threads.
    unsafe { CGDisplayIsAsleep(CGMainDisplayID()) == 0 }
}

#[cfg(not(target_os = "macos"))]
fn display_awake() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_sleep_and_brief_maintenance_wakes() {
        let mut wake = WakeSamples::default();
        let second = Duration::from_secs(1);
        let sleep = Duration::from_secs(8 * 3600);
        assert_eq!(wake.record(second, second, true), second);
        assert_eq!(wake.record(second, sleep, true), Duration::ZERO);
        for _ in 0..5 {
            assert_eq!(wake.record(second, second, false), Duration::ZERO);
        }
        assert_eq!(wake.record(sleep, sleep, true), Duration::ZERO);
        if !cfg!(target_os = "macos") {
            for _ in 0..14 {
                assert_eq!(wake.record(second, second, true), Duration::ZERO);
            }
        }
        assert_eq!(wake.record(second, second, true), second);
    }

    #[test]
    fn excludes_display_off_time_during_maintenance() {
        let mut wake = WakeSamples::default();
        let second = Duration::from_secs(1);
        for _ in 0..30 {
            assert_eq!(wake.record(second, second, false), Duration::ZERO);
        }
    }
}
