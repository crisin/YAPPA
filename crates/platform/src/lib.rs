//! OS integration behind small interfaces (CLAUDE.md rule 3). Today: global hotkeys for
//! push-to-talk (spike S3, ADR-007). The hook sees every key and mouse button system-wide —
//! also while a game in fullscreen has the focus — and passes it on, so the game still
//! gets the key.
//!
//! A hotkey is a stable name: a keyboard key as `KeyV`, `F13`, `CapsLock`, … and a mouse
//! button as `Mouse4`, `Mouse5`, `MouseMiddle`. Left and right click are never offered.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

type PttFn = Box<dyn Fn(bool) + Send>;

#[derive(Default)]
struct State {
    ptt: Option<String>,
    held: bool,
    /// Waiting for "the next key" (choosing the PTT key in the settings).
    capture: Option<mpsc::Sender<String>>,
    on_ptt: Option<PttFn>,
    /// The hook could not be installed (no permission, no display): keys only work while
    /// the window has the focus — the UI says so.
    failed: bool,
}

/// The running global hook. Cheap to clone.
#[derive(Clone)]
pub struct Hotkeys {
    state: Arc<Mutex<State>>,
}

/// The name of a key or mouse button, or `None` for what is never a hotkey.
fn name(event: &rdev::EventType) -> Option<(String, bool)> {
    use rdev::{Button, EventType};
    let button = |b: &Button| match b {
        Button::Left | Button::Right => None,
        Button::Middle => Some("MouseMiddle".to_string()),
        // XButton1/2 — the side buttons; rdev reports them as Unknown(1|2) on Windows.
        Button::Unknown(n) => Some(format!("Mouse{}", u32::from(*n) + 3)),
    };
    match event {
        EventType::KeyPress(key) => Some((format!("{key:?}"), true)),
        EventType::KeyRelease(key) => Some((format!("{key:?}"), false)),
        EventType::ButtonPress(b) => button(b).map(|n| (n, true)),
        EventType::ButtonRelease(b) => button(b).map(|n| (n, false)),
        _ => None,
    }
}

impl Hotkeys {
    /// Installs the hook on a thread of its own. `on_ptt` is called with true when the
    /// push-to-talk key goes down and false when it comes up (auto-repeat is filtered).
    pub fn start(on_ptt: impl Fn(bool) + Send + 'static) -> Self {
        let state = Arc::new(Mutex::new(State {
            on_ptt: Some(Box::new(on_ptt)),
            ..State::default()
        }));
        let hook = state.clone();
        let failed = state.clone();
        let spawned = std::thread::Builder::new()
            .name("yappa-hotkeys".into())
            .spawn(move || {
                // Runs the OS message loop until the process ends. The callback must be
                // quick: Windows drops a low-level hook that keeps it waiting.
                let result = rdev::listen(move |event| Self::handle(&hook, &event.event_type));
                if let Err(error) = result {
                    tracing::warn!(
                        ?error,
                        "no global hotkeys — push-to-talk only works while yAPPA has the focus"
                    );
                    if let Ok(mut s) = failed.lock() {
                        s.failed = true;
                    }
                }
            });
        if let Err(error) = spawned {
            tracing::warn!(%error, "could not start the hotkey thread");
            state.lock().expect("lock").failed = true;
        }
        Self { state }
    }

    /// A hotkey object that never fires — for tests and the headless tool.
    pub fn none() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                failed: true,
                ..State::default()
            })),
        }
    }

    fn handle(state: &Mutex<State>, event: &rdev::EventType) {
        let Some((key, down)) = name(event) else {
            return;
        };
        let Ok(mut s) = state.lock() else {
            return;
        };
        if down {
            if let Some(capture) = s.capture.take() {
                let _ = capture.send(key);
                return;
            }
        }
        if s.ptt.as_deref() == Some(key.as_str()) && down != s.held {
            s.held = down;
            if let Some(on_ptt) = s.on_ptt.as_ref() {
                on_ptt(down);
            }
        }
    }

    /// The key that talks; `None` = no PTT key.
    pub fn set_ptt(&self, key: Option<String>) {
        let mut s = self.state.lock().expect("lock");
        if s.held {
            // Changing the key while it is held must not leave the microphone open.
            s.held = false;
            if let Some(on_ptt) = s.on_ptt.as_ref() {
                on_ptt(false);
            }
        }
        s.ptt = key;
    }

    /// Waits for the next key or mouse button (left/right click excluded). `Escape` cancels
    /// and gives `None`, as does the timeout.
    pub fn capture_next(&self, timeout: Duration) -> Option<String> {
        let (tx, rx) = mpsc::channel();
        self.state.lock().expect("lock").capture = Some(tx);
        let key = rx.recv_timeout(timeout).ok();
        self.state.lock().expect("lock").capture = None;
        key.filter(|k| k != "Escape")
    }

    /// False when the global hook is not running (the UI explains the consequence).
    pub fn is_global(&self) -> bool {
        !self.state.lock().map(|s| s.failed).unwrap_or(true)
    }

    #[cfg(test)]
    fn feed(&self, event: rdev::EventType) {
        Self::handle(&self.state, &event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rdev::{Button, EventType, Key};
    use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

    fn recorder() -> (Hotkeys, Arc<Mutex<Vec<bool>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let hotkeys = Hotkeys {
            state: Arc::new(Mutex::new(State {
                on_ptt: Some(Box::new(move |down| log.lock().unwrap().push(down))),
                ..State::default()
            })),
        };
        (hotkeys, seen)
    }

    #[test]
    fn the_ptt_key_reports_down_and_up_once_despite_auto_repeat() {
        let (hotkeys, seen) = recorder();
        hotkeys.set_ptt(Some("KeyV".into()));
        for _ in 0..5 {
            hotkeys.feed(EventType::KeyPress(Key::KeyV));
        }
        hotkeys.feed(EventType::KeyPress(Key::KeyB));
        hotkeys.feed(EventType::KeyRelease(Key::KeyV));
        assert_eq!(*seen.lock().unwrap(), [true, false]);
    }

    #[test]
    fn a_mouse_side_button_works_and_clicks_never_do() {
        let (hotkeys, seen) = recorder();
        hotkeys.set_ptt(Some("Mouse4".into()));
        hotkeys.feed(EventType::ButtonPress(Button::Left));
        hotkeys.feed(EventType::ButtonPress(Button::Unknown(1)));
        hotkeys.feed(EventType::ButtonRelease(Button::Unknown(1)));
        assert_eq!(*seen.lock().unwrap(), [true, false]);
    }

    #[test]
    fn changing_the_key_while_held_closes_the_microphone() {
        let (hotkeys, seen) = recorder();
        hotkeys.set_ptt(Some("F11".into()));
        hotkeys.feed(EventType::KeyPress(Key::F11));
        hotkeys.set_ptt(Some("F12".into()));
        assert_eq!(*seen.lock().unwrap(), [true, false]);
    }

    #[test]
    fn capturing_takes_the_next_key_and_escape_cancels() {
        let (hotkeys, seen) = recorder();
        let calls = Arc::new(AtomicU32::new(0));
        let h = hotkeys.clone();
        let c = calls.clone();
        let waiter = std::thread::spawn(move || {
            c.fetch_add(1, Relaxed);
            h.capture_next(Duration::from_secs(2))
        });
        // Wait until the capture is armed.
        while hotkeys.state.lock().unwrap().capture.is_none() {
            std::thread::yield_now();
        }
        hotkeys.feed(EventType::ButtonPress(Button::Left)); // ignored
        hotkeys.feed(EventType::KeyPress(Key::CapsLock));
        assert_eq!(waiter.join().unwrap().as_deref(), Some("CapsLock"));
        assert!(seen.lock().unwrap().is_empty(), "capturing is not talking");

        let h = hotkeys.clone();
        let waiter = std::thread::spawn(move || h.capture_next(Duration::from_secs(2)));
        while hotkeys.state.lock().unwrap().capture.is_none() {
            std::thread::yield_now();
        }
        hotkeys.feed(EventType::KeyPress(Key::Escape));
        assert_eq!(waiter.join().unwrap(), None);
        assert_eq!(calls.load(Relaxed), 1);
    }
}
