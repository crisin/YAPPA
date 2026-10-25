//! OS integration behind small interfaces (CLAUDE.md rule 3). Today: global hotkeys for
//! push-to-talk, mute and deafen (spike S3, ADR-007, A3) and audio device hot-plug
//! notifications ([`devices`], A2). The hook sees every key and mouse button system-wide —
//! also while a game in fullscreen has the focus — and passes it on, so the game still
//! gets the key.
//!
//! A hotkey is a stable name: a keyboard key as `KeyV`, `F13`, `CapsLock`, … and a mouse
//! button as `Mouse4`, `Mouse5`, `MouseMiddle`. Left and right click are never offered.

pub mod devices;

pub use devices::DeviceWatcher;

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What a global key does. The hook reports down and up for each; what that means (hold
/// to talk, press to flip) is the shell's business.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    PushToTalk,
    Mute,
    Deafen,
}

impl HotkeyAction {
    pub const ALL: [HotkeyAction; 3] = [
        HotkeyAction::PushToTalk,
        HotkeyAction::Mute,
        HotkeyAction::Deafen,
    ];

    fn index(self) -> usize {
        match self {
            HotkeyAction::PushToTalk => 0,
            HotkeyAction::Mute => 1,
            HotkeyAction::Deafen => 2,
        }
    }
}

type KeyFn = Box<dyn Fn(HotkeyAction, bool) + Send>;

#[derive(Default)]
struct State {
    /// The key of each action, by [`HotkeyAction::index`]; `None` = not set.
    keys: [Option<String>; 3],
    held: [bool; 3],
    /// Waiting for "the next key" (choosing a key in the settings).
    capture: Option<mpsc::Sender<String>>,
    on_key: Option<KeyFn>,
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
    /// Installs the hook on a thread of its own. `on_key` is called with the action and
    /// true when its key goes down, false when it comes up (auto-repeat is filtered).
    pub fn start(on_key: impl Fn(HotkeyAction, bool) + Send + 'static) -> Self {
        let state = Arc::new(Mutex::new(State {
            on_key: Some(Box::new(on_key)),
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
                        "no global hotkeys — the keys only work while yAPPA has the focus"
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
        for action in HotkeyAction::ALL {
            let i = action.index();
            if s.keys[i].as_deref() == Some(key.as_str()) && down != s.held[i] {
                s.held[i] = down;
                if let Some(on_key) = s.on_key.as_ref() {
                    on_key(action, down);
                }
            }
        }
    }

    /// The key of one action; `None` = none. The same key on two actions would fire both —
    /// the settings layer refuses that before it gets here.
    pub fn set_key(&self, action: HotkeyAction, key: Option<String>) {
        let mut s = self.state.lock().expect("lock");
        let i = action.index();
        if s.held[i] {
            // Changing the key while it is held must not leave the microphone open.
            s.held[i] = false;
            if let Some(on_key) = s.on_key.as_ref() {
                on_key(action, false);
            }
        }
        s.keys[i] = key;
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
    use HotkeyAction::{Deafen, Mute, PushToTalk};

    type Seen = Arc<Mutex<Vec<(HotkeyAction, bool)>>>;

    fn recorder() -> (Hotkeys, Seen) {
        let seen: Seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let hotkeys = Hotkeys {
            state: Arc::new(Mutex::new(State {
                on_key: Some(Box::new(move |action, down| {
                    log.lock().unwrap().push((action, down));
                })),
                ..State::default()
            })),
        };
        (hotkeys, seen)
    }

    #[test]
    fn the_ptt_key_reports_down_and_up_once_despite_auto_repeat() {
        let (hotkeys, seen) = recorder();
        hotkeys.set_key(PushToTalk, Some("KeyV".into()));
        for _ in 0..5 {
            hotkeys.feed(EventType::KeyPress(Key::KeyV));
        }
        hotkeys.feed(EventType::KeyPress(Key::KeyB));
        hotkeys.feed(EventType::KeyRelease(Key::KeyV));
        assert_eq!(
            *seen.lock().unwrap(),
            [(PushToTalk, true), (PushToTalk, false)]
        );
    }

    #[test]
    fn a_mouse_side_button_works_and_clicks_never_do() {
        let (hotkeys, seen) = recorder();
        hotkeys.set_key(PushToTalk, Some("Mouse4".into()));
        hotkeys.feed(EventType::ButtonPress(Button::Left));
        hotkeys.feed(EventType::ButtonPress(Button::Unknown(1)));
        hotkeys.feed(EventType::ButtonRelease(Button::Unknown(1)));
        assert_eq!(
            *seen.lock().unwrap(),
            [(PushToTalk, true), (PushToTalk, false)]
        );
    }

    #[test]
    fn changing_the_key_while_held_closes_the_microphone() {
        let (hotkeys, seen) = recorder();
        hotkeys.set_key(PushToTalk, Some("F11".into()));
        hotkeys.feed(EventType::KeyPress(Key::F11));
        hotkeys.set_key(PushToTalk, Some("F12".into()));
        assert_eq!(
            *seen.lock().unwrap(),
            [(PushToTalk, true), (PushToTalk, false)]
        );
    }

    #[test]
    fn mute_and_deafen_keys_report_their_own_action_and_do_not_disturb_ptt() {
        let (hotkeys, seen) = recorder();
        hotkeys.set_key(PushToTalk, Some("Mouse4".into()));
        hotkeys.set_key(Mute, Some("F7".into()));
        hotkeys.set_key(Deafen, Some("F8".into()));
        hotkeys.feed(EventType::ButtonPress(Button::Unknown(1)));
        hotkeys.feed(EventType::KeyPress(Key::F7));
        hotkeys.feed(EventType::KeyPress(Key::F7)); // auto-repeat while held
        hotkeys.feed(EventType::KeyRelease(Key::F7));
        hotkeys.feed(EventType::KeyPress(Key::F8));
        hotkeys.feed(EventType::KeyRelease(Key::F8));
        hotkeys.feed(EventType::ButtonRelease(Button::Unknown(1)));
        assert_eq!(
            *seen.lock().unwrap(),
            [
                (PushToTalk, true),
                (Mute, true),
                (Mute, false),
                (Deafen, true),
                (Deafen, false),
                (PushToTalk, false),
            ]
        );
        hotkeys.set_key(Mute, None);
        hotkeys.feed(EventType::KeyPress(Key::F7));
        assert_eq!(seen.lock().unwrap().len(), 6, "a removed key is silent");
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
