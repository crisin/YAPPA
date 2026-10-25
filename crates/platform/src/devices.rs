//! Audio device hot-plug (A2, ADR-009). The OS tells us when an audio endpoint comes, goes,
//! changes state or becomes the default; the engine then re-reads its device list and takes
//! the chosen device back. One module per OS behind this handle (CLAUDE.md rule 3):
//!
//! - Windows: `IMMNotificationClient` on the MMDevice enumerator ([`windows`]).
//! - macOS: not implemented yet — [`DeviceWatcher::start`] gives `None` and the engine
//!   falls back to polling its device list once a second while a device is missing.
//!
//! The OS fires several notifications for one plugged-in headset (state, properties,
//! default role per flow); a watcher thread coalesces a burst into one call of the
//! callback, after [`QUIET`] without further notifications.

use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

/// A burst of notifications counts as one change once it has been quiet this long.
pub const QUIET: Duration = Duration::from_millis(300);

/// What the OS backend sends to the coalescing thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Signal {
    /// Only an OS backend sends this; where there is none, only the tests do.
    #[cfg_attr(not(windows), allow(dead_code))]
    Changed,
    Stop,
}

/// The running watcher. Dropping it stops the thread and unregisters from the OS.
pub struct DeviceWatcher {
    tx: mpsc::Sender<Signal>,
    thread: Option<JoinHandle<()>>,
}

impl DeviceWatcher {
    /// Starts watching; `on_change` runs on the watcher's thread, once per burst of
    /// notifications. `None` when this OS has no implementation or refused — the caller
    /// degrades to polling, the call goes on either way.
    pub fn start(on_change: impl Fn() + Send + 'static) -> Option<Self> {
        if !os::SUPPORTED {
            tracing::info!(
                "no device notifications on this OS — a returning device is found by polling"
            );
            return None;
        }
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let backend_tx = tx.clone();
        let thread = std::thread::Builder::new()
            .name("yappa-devices".into())
            .spawn(move || {
                // The backend lives on this thread for as long as it coalesces; going out
                // of scope at the end unregisters the callback (COM wants that on the same
                // thread).
                let backend = os::Backend::start(backend_tx);
                let _ = ready_tx.send(backend.is_ok());
                let Ok(_backend) = backend else {
                    return;
                };
                tracing::info!("watching audio devices");
                coalesce(&rx, QUIET, on_change);
            });
        let thread = match thread {
            Ok(thread) => thread,
            Err(error) => {
                tracing::warn!(%error, "could not start the device watcher thread");
                return None;
            }
        };
        match ready_rx.recv() {
            Ok(true) => Some(Self {
                tx,
                thread: Some(thread),
            }),
            _ => {
                let _ = thread.join();
                None
            }
        }
    }
}

impl Drop for DeviceWatcher {
    fn drop(&mut self) {
        let _ = self.tx.send(Signal::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Turns a stream of [`Signal::Changed`] into one `on_change` per burst: after the first
/// signal it waits until `quiet` passes without another, then calls. Returns on
/// [`Signal::Stop`] or when every sender is gone.
pub(crate) fn coalesce(rx: &mpsc::Receiver<Signal>, quiet: Duration, mut on_change: impl FnMut()) {
    loop {
        match rx.recv() {
            Ok(Signal::Changed) => {}
            Ok(Signal::Stop) | Err(_) => return,
        }
        loop {
            match rx.recv_timeout(quiet) {
                Ok(Signal::Changed) => continue,
                Ok(Signal::Stop) => return,
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        on_change();
    }
}

#[cfg(windows)]
mod os {
    //! `IMMNotificationClient`: the MMDevice enumerator calls us on a COM thread for every
    //! endpoint change. COM registration has no safe wrapper, so this module is the one
    //! place in the repo with `unsafe` — four calls, each with its reason.
    #![allow(unsafe_code)]

    use super::Signal;
    use std::sync::mpsc;
    use windows::core::{implement, Result, PCWSTR};
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::Media::Audio::{
        EDataFlow, ERole, IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl,
        MMDeviceEnumerator, DEVICE_STATE,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    pub(super) const SUPPORTED: bool = true;

    #[implement(IMMNotificationClient)]
    struct Client {
        tx: mpsc::Sender<Signal>,
    }

    impl Client {
        fn changed(&self) -> Result<()> {
            let _ = self.tx.send(Signal::Changed);
            Ok(())
        }
    }

    // Every callback runs on a COM worker thread: send and leave, nothing else.
    impl IMMNotificationClient_Impl for Client_Impl {
        fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> Result<()> {
            self.changed()
        }
        fn OnDeviceAdded(&self, _: &PCWSTR) -> Result<()> {
            self.changed()
        }
        fn OnDeviceRemoved(&self, _: &PCWSTR) -> Result<()> {
            self.changed()
        }
        fn OnDefaultDeviceChanged(&self, _: EDataFlow, _: ERole, _: &PCWSTR) -> Result<()> {
            self.changed()
        }
        /// Properties change all the time (volume, names): not a hot-plug event.
        fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> Result<()> {
            Ok(())
        }
    }

    /// The registration; dropped on the thread that made it.
    pub(super) struct Backend {
        /// Enumerator and client, released before COM is uninitialised.
        registered: Option<(IMMDeviceEnumerator, IMMNotificationClient)>,
    }

    impl Backend {
        pub(super) fn start(tx: mpsc::Sender<Signal>) -> std::result::Result<Self, String> {
            // SAFETY: plain COM initialisation of this thread (multithreaded apartment, so
            // the callbacks need no message pump); balanced in `Drop` on the same thread.
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if hr.is_err() {
                return Err(format!("CoInitializeEx: {hr}"));
            }
            // S_OK or S_FALSE (already initialised on this thread): both count and both
            // are balanced by the CoUninitialize in `Drop`.
            let mut backend = Self { registered: None };
            // SAFETY: the documented way to get the enumerator; the GUID is its CLSID.
            let enumerator: IMMDeviceEnumerator =
                unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
                    .map_err(|e| format!("MMDeviceEnumerator: {e}"))?;
            let client: IMMNotificationClient = Client { tx }.into();
            // SAFETY: a valid interface pointer; COM holds its own reference until the
            // matching unregister in `Drop`.
            unsafe { enumerator.RegisterEndpointNotificationCallback(&client) }
                .map_err(|e| format!("RegisterEndpointNotificationCallback: {e}"))?;
            backend.registered = Some((enumerator, client));
            Ok(backend)
        }
    }

    impl Drop for Backend {
        fn drop(&mut self) {
            if let Some((enumerator, client)) = self.registered.take() {
                // SAFETY: the same client that was registered; afterwards COM calls it no
                // more, and both objects are released right here.
                let _ = unsafe { enumerator.UnregisterEndpointNotificationCallback(&client) };
            }
            // SAFETY: balances this thread's successful CoInitializeEx (S_OK or S_FALSE),
            // after our COM objects went.
            unsafe { CoUninitialize() };
        }
    }
}

#[cfg(not(windows))]
mod os {
    //! No backend on this OS (macOS: CoreAudio property listeners are an open follow-up):
    //! the engine polls while a device is missing.
    use super::Signal;
    use std::sync::mpsc;

    pub(super) const SUPPORTED: bool = false;

    pub(super) struct Backend;

    impl Backend {
        pub(super) fn start(_: mpsc::Sender<Signal>) -> Result<Self, String> {
            Err("no device notifications on this OS".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
    use std::sync::Arc;

    #[test]
    fn a_burst_of_signals_is_one_change_and_stop_ends_it() {
        let (tx, rx) = mpsc::channel();
        let calls = Arc::new(AtomicU32::new(0));
        let seen = calls.clone();
        let quiet = Duration::from_millis(40);
        let thread = std::thread::spawn(move || {
            coalesce(&rx, quiet, move || {
                seen.fetch_add(1, Relaxed);
            });
        });
        // One headset: Windows reports state, default (console/multimedia/communications),
        // and the device again — within a few milliseconds.
        for _ in 0..6 {
            tx.send(Signal::Changed).unwrap();
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(quiet * 3);
        assert_eq!(calls.load(Relaxed), 1, "one burst, one change");
        // A second, separate event.
        tx.send(Signal::Changed).unwrap();
        std::thread::sleep(quiet * 3);
        assert_eq!(calls.load(Relaxed), 2);
        tx.send(Signal::Stop).unwrap();
        thread.join().unwrap();
    }

    #[test]
    fn every_sender_gone_ends_the_loop_without_a_call() {
        let (tx, rx) = mpsc::channel::<Signal>();
        drop(tx);
        let mut called = false;
        coalesce(&rx, Duration::from_millis(10), || called = true);
        assert!(!called);
    }

    /// On Windows the real registration must work on a plain thread; elsewhere the watcher
    /// says so by giving `None`. Either way the process goes on.
    #[test]
    fn starting_and_dropping_the_watcher_is_harmless() {
        let watcher = DeviceWatcher::start(|| {});
        assert_eq!(watcher.is_some(), cfg!(windows));
        drop(watcher);
    }
}
