//! App audio capture (ADR-011): the sound of one application, as the OS hands it out.
//! One module per OS behind this small surface (CLAUDE.md rule 3):
//!
//! - Windows: WASAPI **process loopback** — `ActivateAudioInterfaceAsync` on the virtual
//!   device `VAD\Process_Loopback` with `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`,
//!   include-tree of the chosen process. The application keeps playing to its device; we
//!   get a copy of what it (and its child processes) render, mixed by the audio engine,
//!   in the format we ask for. Needs Windows 10 build 20348 or Windows 11; older builds
//!   report "not available". The list of applications comes from the audio sessions on
//!   every active render endpoint (`IAudioSessionManager2`).
//! - macOS: not implemented — [`SUPPORTED`] is false, [`list_apps`] and
//!   [`ProcessLoopback::start`] say so. ScreenCaptureKit (macOS 13+) is the way in when
//!   somebody needs it.
//!
//! The capture runs on a thread of its own, `yappa-app-audio`, woken by the audio engine's
//! event every 10 ms; every packet goes to the `on_packet` callback on that thread as
//! interleaved `f32`. Nothing here knows the engine or the transport.

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::Arc;
use std::thread::JoinHandle;

/// Whether this OS and version can capture one application's sound.
pub fn supported() -> bool {
    os::supported()
}

/// One running application with an audio session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSession {
    pub pid: u32,
    /// The executable's name without extension (`chrome`), or `pid <n>` when the process
    /// cannot be opened (another user's, elevated).
    pub name: String,
    /// The session plays right now.
    pub active: bool,
}

/// The applications with a sound session on any active output, playing ones first, then
/// by name; one line per process (an app with sessions on two devices is one app). The
/// own process and the system sounds are left out.
pub fn list_apps() -> Result<Vec<AppSession>, String> {
    let mut apps = os::sessions()?;
    apps.retain(|a| a.pid != 0 && a.pid != std::process::id());
    apps.sort_by(|a, b| {
        b.active
            .cmp(&a.active)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.pid.cmp(&b.pid))
    });
    apps.dedup_by_key(|a| a.pid);
    Ok(apps)
}

/// What the capture thread calls: `(interleaved samples in −1..1, rate, channels)`.
pub type PacketFn = Box<dyn FnMut(&[f32], u32, u16) + Send>;
/// Called once, from the capture thread, when the capture ends without [`Drop`].
pub type EndFn = Box<dyn FnOnce(String) + Send>;

/// A running process-loopback capture. Dropping it stops the thread and waits for it.
pub struct ProcessLoopback {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    waker: os::Waker,
}

impl ProcessLoopback {
    /// Starts capturing the process tree of `pid`. Returns once the stream runs — or with
    /// the OS's reason why not (no such process, not supported on this Windows, the audio
    /// engine refused the format).
    pub fn start(
        pid: u32,
        on_packet: impl FnMut(&[f32], u32, u16) + Send + 'static,
        on_end: impl FnOnce(String) + Send + 'static,
    ) -> Result<Self, String> {
        if !os::supported() {
            return Err(os::UNSUPPORTED.into());
        }
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let stopped = stop.clone();
        let (on_packet, on_end): (PacketFn, EndFn) = (Box::new(on_packet), Box::new(on_end));
        let thread = std::thread::Builder::new()
            .name("yappa-app-audio".into())
            .spawn(move || {
                os::run(pid, &stopped, ready_tx, on_packet, on_end);
            })
            .map_err(|e| format!("could not start the capture thread: {e}"))?;
        match ready_rx.recv() {
            Ok(Ok(waker)) => Ok(Self {
                stop,
                thread: Some(thread),
                waker,
            }),
            Ok(Err(reason)) => {
                let _ = thread.join();
                Err(reason)
            }
            Err(_) => {
                let _ = thread.join();
                Err("the capture thread ended before it started".into())
            }
        }
    }
}

impl Drop for ProcessLoopback {
    fn drop(&mut self) {
        self.stop.store(true, Relaxed);
        self.waker.wake();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(windows)]
mod os {
    //! WASAPI process loopback and the session list. COM has no safe wrapper, so this
    //! module allows `unsafe` — each call with its reason, like `devices.rs`.
    #![allow(unsafe_code)]

    use super::{AppSession, EndFn, PacketFn};
    use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
    use std::sync::{mpsc, Mutex};
    use std::time::{Duration, Instant};
    use windows::core::{implement, Interface, Ref, Result, HRESULT};
    use windows::Win32::Foundation::{
        CloseHandle, DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE, RPC_E_CHANGED_MODE,
        WAIT_OBJECT_0,
    };
    use windows::Win32::Media::Audio::{
        eRender, ActivateAudioInterfaceAsync, AudioSessionStateActive,
        IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
        IActivateAudioInterfaceCompletionHandler_Impl, IAudioCaptureClient, IAudioClient,
        IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, MMDeviceEnumerator,
        AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
        AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
        AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0,
        AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS,
        DEVICE_STATE_ACTIVE, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
        VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WAVEFORMATEX,
    };
    use windows::Win32::System::Com::StructuredStorage::{
        PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, BLOB, CLSCTX_ALL, COINIT_MULTITHREADED,
    };
    use windows::Win32::System::Threading::{
        CreateEventW, GetCurrentProcess, OpenProcess, QueryFullProcessImageNameW, SetEvent,
        WaitForSingleObject, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_SYNCHRONIZE,
    };
    use windows::Win32::System::Variant::VT_BLOB;

    pub(super) const UNSUPPORTED: &str =
        "app audio needs Windows 11 (or Windows 10 build 20348): this Windows is older";
    /// Process loopback arrived with Windows 10 build 20348 (Windows 11, Server 2022).
    const MIN_BUILD: u32 = 20348;
    /// `WAVEFORMATEX::wFormatTag` for 32-bit float and for integer PCM.
    const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
    const WAVE_FORMAT_PCM: u16 = 1;
    /// The stream's buffer, in 100 ns units: 200 ms — plenty against a late wake-up; the
    /// engine's event fires per 10 ms period regardless.
    const BUFFER_HNS: i64 = 2_000_000;
    /// How long the activation may take before it counts as failed.
    const ACTIVATE_TIMEOUT: Duration = Duration::from_secs(5);

    pub(super) fn supported() -> bool {
        windows_version::OsVersion::current().build >= MIN_BUILD
    }

    /// Handed back to the main thread so `Drop` can wake the capture loop at once. Holds
    /// its own duplicate of the stream's event: the capture thread closes its handle when
    /// it ends — also by itself, long before `ProcessLoopback` is dropped — and a closed
    /// handle value may already name another object (realtime review 2026-10-27). `None`
    /// when the duplication failed: the loop then notices the stop flag within 100 ms.
    pub(super) struct Waker(Option<HANDLE>);

    // The handle is only ever passed to `SetEvent` and `CloseHandle`, both thread-safe.
    unsafe impl Send for Waker {}
    unsafe impl Sync for Waker {}

    impl Waker {
        fn duplicate(event: HANDLE) -> Self {
            let mut copy = HANDLE::default();
            // SAFETY: a valid event handle of this process; the copy is ours to close.
            let result = unsafe {
                DuplicateHandle(
                    GetCurrentProcess(),
                    event,
                    GetCurrentProcess(),
                    &mut copy,
                    0,
                    false,
                    DUPLICATE_SAME_ACCESS,
                )
            };
            Self(result.ok().map(|()| copy))
        }

        pub(super) fn wake(&self) {
            if let Some(event) = self.0 {
                // SAFETY: our own duplicate, open until `Drop`.
                let _ = unsafe { SetEvent(event) };
            }
        }
    }

    impl Drop for Waker {
        fn drop(&mut self) {
            if let Some(event) = self.0.take() {
                // SAFETY: the duplicate we made, closed once.
                let _ = unsafe { CloseHandle(event) };
            }
        }
    }

    /// A thread with COM initialised for its lifetime. `owned` is false when the thread
    /// already lives in a single-threaded apartment (the app's UI and command threads,
    /// set up by the webview): COM is usable there as it is, and is not ours to tear down.
    struct Com {
        owned: bool,
    }

    impl Com {
        fn init() -> std::result::Result<Self, String> {
            // SAFETY: plain COM initialisation of this thread; balanced in `Drop`.
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if hr == RPC_E_CHANGED_MODE {
                return Ok(Self { owned: false });
            }
            if hr.is_err() {
                return Err(format!("CoInitializeEx: {hr}"));
            }
            Ok(Self { owned: true })
        }
    }

    impl Drop for Com {
        fn drop(&mut self) {
            if self.owned {
                // SAFETY: balances this thread's successful CoInitializeEx.
                unsafe { CoUninitialize() };
            }
        }
    }

    // ── the session list ────────────────────────────────────────────────────────────────

    pub(super) fn sessions() -> std::result::Result<Vec<AppSession>, String> {
        let _com = Com::init()?;
        // SAFETY: the documented way to the enumerator; the GUID is its CLSID. Every
        // COM call below is on interfaces we hold a reference to, with plain arguments.
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(|e| format!("MMDeviceEnumerator: {e}"))?;
            let devices = enumerator
                .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
                .map_err(|e| format!("EnumAudioEndpoints: {e}"))?;
            let mut apps = Vec::new();
            for i in 0..devices.GetCount().map_err(|e| e.to_string())? {
                let Ok(device) = devices.Item(i) else {
                    continue;
                };
                let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else {
                    continue;
                };
                let Ok(sessions) = manager.GetSessionEnumerator() else {
                    continue;
                };
                for j in 0..sessions.GetCount().unwrap_or(0) {
                    let Ok(control) = sessions.GetSession(j) else {
                        continue;
                    };
                    let Ok(control2) = control.cast::<IAudioSessionControl2>() else {
                        continue;
                    };
                    let Ok(pid) = control2.GetProcessId() else {
                        continue;
                    };
                    let active = control.GetState().ok() == Some(AudioSessionStateActive);
                    apps.push(AppSession {
                        pid,
                        name: process_name(pid).unwrap_or_else(|| format!("pid {pid}")),
                        active,
                    });
                }
            }
            Ok(apps)
        }
    }

    /// The executable's name without extension, or `None` when the process cannot be
    /// opened (another user's, elevated, gone).
    fn process_name(pid: u32) -> Option<String> {
        // SAFETY: a query-only handle, closed below; the buffer length is passed with it.
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buffer = [0u16; 1024];
            let mut len = buffer.len() as u32;
            let result = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(buffer.as_mut_ptr()),
                &mut len,
            );
            let _ = CloseHandle(handle);
            result.ok()?;
            let path = String::from_utf16_lossy(&buffer[..len as usize]);
            let file = path.rsplit(['\\', '/']).next()?;
            let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
            (!stem.is_empty()).then(|| stem.to_string())
        }
    }

    // ── the capture ─────────────────────────────────────────────────────────────────────

    /// Receives the activated `IAudioClient` on a COM worker thread and hands it over.
    /// windows-rs objects are agile by default, which the API requires.
    #[implement(IActivateAudioInterfaceCompletionHandler)]
    struct Completion {
        tx: Mutex<Option<mpsc::Sender<std::result::Result<IAudioClient, String>>>>,
    }

    impl IActivateAudioInterfaceCompletionHandler_Impl for Completion_Impl {
        fn ActivateCompleted(
            &self,
            operation: Ref<IActivateAudioInterfaceAsyncOperation>,
        ) -> Result<()> {
            let result = (|| {
                let operation = operation.ok().map_err(|e| e.to_string())?;
                let mut hr = HRESULT(0);
                let mut unknown = None;
                // SAFETY: out-pointers to locals, as the method wants them.
                unsafe { operation.GetActivateResult(&mut hr, &mut unknown) }
                    .map_err(|e| format!("GetActivateResult: {e}"))?;
                hr.ok().map_err(|e| format!("activation: {e}"))?;
                unknown
                    .ok_or_else(|| "activation gave no interface".to_string())?
                    .cast::<IAudioClient>()
                    .map_err(|e| format!("not an IAudioClient: {e}"))
            })();
            if let Some(tx) = self.tx.lock().ok().and_then(|mut t| t.take()) {
                let _ = tx.send(result);
            }
            Ok(())
        }
    }

    /// The audio client for `pid`'s process tree, activated and waited for.
    fn activate(pid: u32) -> std::result::Result<IAudioClient, String> {
        let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
            ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
            Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
                ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                    TargetProcessId: pid,
                    ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
                },
            },
        };
        // The parameters travel as a VT_BLOB PROPVARIANT pointing at the struct above —
        // the documented shape (ApplicationLoopback sample). `ManuallyDrop`, because the
        // windows crate drops a PROPVARIANT with `PropVariantClear`, which for a blob calls
        // `CoTaskMemFree` on the pointer — ours is a stack address, and freeing it corrupted
        // the heap on every activation (found 2026-10-27 after a long bisection).
        let blob = std::mem::ManuallyDrop::new(PROPVARIANT {
            Anonymous: PROPVARIANT_0 {
                Anonymous: std::mem::ManuallyDrop::new(PROPVARIANT_0_0 {
                    vt: VT_BLOB,
                    wReserved1: 0,
                    wReserved2: 0,
                    wReserved3: 0,
                    Anonymous: PROPVARIANT_0_0_0 {
                        blob: BLOB {
                            cbSize: std::mem::size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
                            pBlobData: (&mut params as *mut AUDIOCLIENT_ACTIVATION_PARAMS).cast(),
                        },
                    },
                }),
            },
        });
        let (tx, rx) = mpsc::channel();
        let handler: IActivateAudioInterfaceCompletionHandler = Completion {
            tx: Mutex::new(Some(tx)),
        }
        .into();
        // SAFETY: `blob` and `params` outlive the call (the system copies the parameters
        // during it); the handler is a valid COM object the system holds its own
        // references to until the operation is done. The operation object is kept until
        // the completion arrived.
        let operation = unsafe {
            ActivateAudioInterfaceAsync(
                VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
                &IAudioClient::IID,
                Some(&*blob),
                &handler,
            )
        }
        .map_err(|e| format!("ActivateAudioInterfaceAsync: {e}"))?;
        let client = rx
            .recv_timeout(ACTIVATE_TIMEOUT)
            .map_err(|_| "the audio engine did not answer the activation".to_string())?;
        drop(operation);
        client
    }

    /// 48 kHz stereo float — what the engine wants, so nothing is converted — or, should
    /// the audio engine refuse it, 44.1 kHz 16-bit stereo (the format Microsoft's own
    /// sample asks for), converted by the engine.
    fn formats() -> [WAVEFORMATEX; 2] {
        let format = |tag: u16, rate: u32, bits: u16| WAVEFORMATEX {
            wFormatTag: tag,
            nChannels: 2,
            nSamplesPerSec: rate,
            nAvgBytesPerSec: rate * u32::from(2 * bits / 8),
            nBlockAlign: 2 * bits / 8,
            wBitsPerSample: bits,
            cbSize: 0,
        };
        [
            format(WAVE_FORMAT_IEEE_FLOAT, 48_000, 32),
            format(WAVE_FORMAT_PCM, 44_100, 16),
        ]
    }

    struct Stream {
        client: IAudioClient,
        capture: IAudioCaptureClient,
        event: HANDLE,
        format: WAVEFORMATEX,
    }

    impl Stream {
        fn open(pid: u32) -> std::result::Result<Self, String> {
            let client = activate(pid)?;
            let mut opened = None;
            let mut last = String::new();
            for format in formats() {
                // SAFETY: a valid client and a complete WAVEFORMATEX that outlives the call.
                let result = unsafe {
                    client.Initialize(
                        AUDCLNT_SHAREMODE_SHARED,
                        AUDCLNT_STREAMFLAGS_LOOPBACK
                            | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
                            | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
                        BUFFER_HNS,
                        0,
                        &format,
                        None,
                    )
                };
                match result {
                    Ok(()) => {
                        opened = Some(format);
                        break;
                    }
                    Err(e) => {
                        let rate = format.nSamplesPerSec;
                        last = format!("Initialize ({rate} Hz): {e}");
                    }
                }
            }
            let format = opened.ok_or(last)?;
            // SAFETY: an auto-reset event we own; handed to the client, closed in `Drop`
            // after the client stopped.
            let event = unsafe { CreateEventW(None, false, false, None) }
                .map_err(|e| format!("CreateEvent: {e}"))?;
            // SAFETY: valid client and event.
            unsafe { client.SetEventHandle(event) }.map_err(|e| format!("SetEventHandle: {e}"))?;
            // SAFETY: the client is initialised; the service is the capture interface.
            let capture: IAudioCaptureClient =
                unsafe { client.GetService() }.map_err(|e| format!("IAudioCaptureClient: {e}"))?;
            // SAFETY: initialised client.
            unsafe { client.Start() }.map_err(|e| format!("Start: {e}"))?;
            Ok(Self {
                client,
                capture,
                event,
                format,
            })
        }

        /// Reads every packet the engine has queued into `samples` (as f32, interleaved)
        /// and hands each to `on_packet`.
        fn drain(&self, samples: &mut Vec<f32>, on_packet: &mut PacketFn) -> Result<()> {
            let channels = self.format.nChannels;
            let rate = self.format.nSamplesPerSec;
            loop {
                // SAFETY: the capture client belongs to a started stream; the out-pointers
                // are locals; the buffer is read only between GetBuffer and ReleaseBuffer,
                // for `frames` frames of the stream's own format.
                unsafe {
                    let queued = self.capture.GetNextPacketSize()?;
                    if queued == 0 {
                        return Ok(());
                    }
                    let mut data: *mut u8 = std::ptr::null_mut();
                    let mut frames = 0u32;
                    let mut flags = 0u32;
                    self.capture
                        .GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
                    let n = frames as usize * usize::from(channels);
                    samples.clear();
                    if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                        samples.resize(n, 0.0);
                    } else if self.format.wFormatTag == WAVE_FORMAT_IEEE_FLOAT {
                        let floats = std::slice::from_raw_parts(data.cast::<f32>(), n);
                        samples.extend_from_slice(floats);
                    } else {
                        let ints = std::slice::from_raw_parts(data.cast::<i16>(), n);
                        samples.extend(ints.iter().map(|s| f32::from(*s) / f32::from(i16::MAX)));
                    }
                    self.capture.ReleaseBuffer(frames)?;
                }
                if !samples.is_empty() {
                    on_packet(samples, rate, channels);
                }
            }
        }
    }

    impl Drop for Stream {
        fn drop(&mut self) {
            // SAFETY: our own started client and event; the client is stopped before the
            // event it signals is closed.
            unsafe {
                let _ = self.client.Stop();
                let _ = CloseHandle(self.event);
            }
        }
    }

    /// A handle to watch the application itself: the loopback stream goes on in silence
    /// when the process ends, so the process tells us.
    struct Process(HANDLE);

    impl Process {
        fn open(pid: u32) -> Option<Self> {
            // SAFETY: a synchronise-only handle, closed in `Drop`.
            unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }
                .ok()
                .map(Self)
        }

        fn ended(&self) -> bool {
            // SAFETY: a valid process handle; zero timeout = a poll.
            unsafe { WaitForSingleObject(self.0, 0) == WAIT_OBJECT_0 }
        }
    }

    impl Drop for Process {
        fn drop(&mut self) {
            // SAFETY: the handle we opened.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    /// The capture thread: opens the stream, reports readiness, then reads packets until
    /// the stop flag is set, the process ends or the stream fails.
    pub(super) fn run(
        pid: u32,
        stop: &AtomicBool,
        ready: mpsc::Sender<std::result::Result<Waker, String>>,
        mut on_packet: PacketFn,
        on_end: EndFn,
    ) {
        let _com = match Com::init() {
            Ok(com) => com,
            Err(e) => {
                let _ = ready.send(Err(e));
                return;
            }
        };
        // A process we cannot even watch is one we should not try to capture: the loopback
        // of a pid that does not exist activates fine and delivers silence forever.
        let Some(process) = Process::open(pid) else {
            let _ = ready.send(Err(format!("no such process (pid {pid})")));
            return;
        };
        let stream = match Stream::open(pid) {
            Ok(stream) => stream,
            Err(e) => {
                let _ = ready.send(Err(e));
                return;
            }
        };
        let (rate, bits) = (stream.format.nSamplesPerSec, stream.format.wBitsPerSample);
        tracing::info!(pid, rate, bits, "app audio capture running");
        let _ = ready.send(Ok(Waker::duplicate(stream.event)));
        let mut samples = Vec::with_capacity(4096);
        let mut checked = Instant::now();
        let reason = loop {
            // SAFETY: a valid event; the timeout keeps the loop responsive to the stop flag
            // even when the engine stops signalling (a device change).
            let _ = unsafe { WaitForSingleObject(stream.event, 100) };
            if stop.load(Relaxed) {
                break None;
            }
            if let Err(e) = stream.drain(&mut samples, &mut on_packet) {
                break Some(format!("the capture stopped: {e}"));
            }
            if checked.elapsed() >= Duration::from_secs(1) {
                checked = Instant::now();
                if process.ended() {
                    break Some("the application closed".into());
                }
            }
        };
        drop(stream);
        if let Some(reason) = reason {
            tracing::warn!(pid, %reason, "app audio capture ended");
            on_end(reason);
        } else {
            tracing::info!(pid, "app audio capture stopped");
        }
    }
}

#[cfg(not(windows))]
mod os {
    //! No capture on this OS yet (macOS: ScreenCaptureKit is the open follow-up).
    use super::{AppSession, EndFn, PacketFn};
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;

    pub(super) const UNSUPPORTED: &str = "app audio is not available on this system yet";

    pub(super) fn supported() -> bool {
        false
    }

    pub(super) fn sessions() -> Result<Vec<AppSession>, String> {
        Err(UNSUPPORTED.into())
    }

    pub(super) struct Waker;

    impl Waker {
        pub(super) fn wake(&self) {}
    }

    pub(super) fn run(
        _: u32,
        _: &AtomicBool,
        ready: mpsc::Sender<Result<Waker, String>>,
        _: PacketFn,
        _: EndFn,
    ) {
        let _ = ready.send(Err(UNSUPPORTED.into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The list never panics: on Windows it is the real session list (possibly empty on a
    /// CI box without audio), elsewhere a clear "not available".
    #[test]
    fn listing_is_harmless_and_sorted_playing_first() {
        match list_apps() {
            Ok(apps) => {
                assert!(
                    supported() || apps.is_empty(),
                    "a list only where supported"
                );
                assert!(apps
                    .iter()
                    .all(|a| a.pid != 0 && a.pid != std::process::id()));
                let firsts: Vec<bool> = apps.iter().map(|a| a.active).collect();
                let mut sorted = firsts.clone();
                sorted.sort_by(|a, b| b.cmp(a));
                assert_eq!(firsts, sorted, "playing ones first");
            }
            Err(reason) => {
                assert!(!supported(), "{reason}");
            }
        }
    }

    /// The app asks for the list from a thread the webview already put into a
    /// single-threaded apartment; that must not fail with RPC_E_CHANGED_MODE.
    #[cfg(windows)]
    #[allow(unsafe_code)]
    #[test]
    fn listing_works_on_a_single_threaded_apartment() {
        use windows::Win32::System::Com::{
            CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
        };
        std::thread::spawn(|| {
            // SAFETY: plain COM initialisation of this fresh thread, balanced below.
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.unwrap();
            let listed = list_apps();
            // SAFETY: balances the initialisation above, after the list is done.
            unsafe { CoUninitialize() };
            listed.expect("the list works on an STA thread");
        })
        .join()
        .unwrap();
    }

    /// A process that does not exist is refused, not captured — and where the feature is
    /// not supported, the reason says so.
    #[test]
    fn a_missing_process_or_an_old_os_is_a_clear_error() {
        let result = ProcessLoopback::start(u32::MAX - 7, |_, _, _| {}, |_| {});
        let reason = result.err().expect("no such process");
        if supported() {
            assert!(reason.contains("no such process"), "{reason}");
        } else {
            assert!(
                reason.contains("not available") || reason.contains("Windows"),
                "{reason}"
            );
        }
    }

    /// The real thing, on Windows: capturing our own process (which plays nothing) runs
    /// without a crash and stops cleanly — the activation, the stream and the release of
    /// every object on the way.
    #[test]
    fn capturing_the_own_process_starts_and_stops_cleanly() {
        if !supported() {
            return;
        }
        let packets = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = packets.clone();
        let running = ProcessLoopback::start(
            std::process::id(),
            move |samples, rate, channels| {
                assert_eq!((rate, channels), (48_000, 2));
                assert_eq!(samples.len() % 2, 0);
                seen.fetch_add(1, Relaxed);
            },
            |reason| panic!("ended by itself: {reason}"),
        )
        .expect("the own process can be captured");
        // A silent process still delivers packets (silence, flagged by the engine). How fast
        // depends on the machine: a CI runner without a sound card gave 2 in 300 ms where a
        // PC gives 30, so wait for the packets with a generous limit instead of a fixed time.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while packets.load(Relaxed) <= 5 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        drop(running);
        assert!(
            packets.load(Relaxed) > 5,
            "{} packets in 10 s",
            packets.load(Relaxed)
        );
    }
}
