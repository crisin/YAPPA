//! The host side: what the engine's effect slot would do with a CLAP plugin.

use clack_extensions::audio_ports::{AudioPortInfoBuffer, PluginAudioPorts};
use clack_extensions::latency::PluginLatency;
use clack_host::prelude::*;
use std::ffi::{CStr, CString};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::{BLOCK, SAMPLE_RATE};

/// Our host, as the plugin sees it: no extensions, no callbacks acted on. A real host
/// answers `request_restart` / `request_process` / `request_callback` from its main thread.
pub struct YappaHost;

pub struct Shared;

impl SharedHandler<'_> for Shared {
    fn request_restart(&self) {}
    fn request_process(&self) {}
    fn request_callback(&self) {}
}

impl HostHandlers for YappaHost {
    type Shared<'a> = Shared;
    type MainThread<'a> = ();
    type AudioProcessor<'a> = ();
}

/// One plugin of an entry, as the factory describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Described {
    pub id: String,
    pub name: String,
    pub vendor: String,
}

/// An audio port, as the plugin describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Port {
    pub name: String,
    pub channels: u32,
    pub is_main: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ports {
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
}

/// A loaded CLAP entry (one `.clap` file or the built-in test plugin).
pub struct Loaded {
    entry: PluginEntry,
}

impl Loaded {
    /// The test plugin, linked in: no file, no `unsafe`.
    pub fn builtin() -> Result<Self, String> {
        let entry = PluginEntry::load_from_clack::<crate::plugin::Entry>(c"<builtin>")
            .map_err(|e| format!("load built-in entry: {e}"))?;
        Ok(Self { entry })
    }

    /// A `.clap` file from disk — foreign code runs from here on (its `init`), which is
    /// why this is the one place with `unsafe`.
    pub fn from_path(path: &Path) -> Result<Self, String> {
        // SAFETY: the file is a CLAP bundle the user chose; clack checks the entry's
        // version and that the exported symbol has the CLAP layout. Beyond that, loading
        // a plugin is trusting it — the same trust every DAW extends.
        let entry = unsafe { PluginEntry::load(path) }
            .map_err(|e| format!("load {}: {e}", path.display()))?;
        Ok(Self { entry })
    }

    pub fn clap_version(&self) -> String {
        let v = self.entry.version();
        format!("{}.{}.{}", v.major, v.minor, v.revision)
    }

    /// Every plugin the entry offers.
    pub fn describe(&self) -> Vec<Described> {
        let Some(factory) = self.entry.get_plugin_factory() else {
            return Vec::new();
        };
        let text = |s: Option<&CStr>| {
            s.map(|c| c.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        factory
            .plugin_descriptors()
            .map(|d| Described {
                id: text(d.id()),
                name: text(d.name()),
                vendor: text(d.vendor()),
            })
            .collect()
    }

    /// Instantiate `id`, ready to be activated.
    pub fn instantiate(&self, id: &str) -> Result<Instance, String> {
        let host_info = HostInfo::new("yAPPA", "yAPPA", "https://github.com/crisin/YAPPA", "0.2.0")
            .map_err(|e| format!("host info: {e}"))?;
        let id = CString::new(id).map_err(|e| e.to_string())?;
        let instance =
            PluginInstance::<YappaHost>::new(|_| Shared, |_| (), &self.entry, &id, &host_info)
                .map_err(|e| format!("instantiate {}: {e}", id.to_string_lossy()))?;
        Ok(Instance { instance })
    }
}

/// An instantiated, not yet activated plugin (main-thread object).
pub struct Instance {
    instance: PluginInstance<YappaHost>,
}

impl Instance {
    /// What the plugin says about its audio ports — `None` when it does not implement the
    /// extension (then a host can only guess).
    pub fn ports(&mut self) -> Option<Ports> {
        let handle = self.instance.plugin_handle();
        let ext = handle.get_extension::<PluginAudioPorts>()?;
        let mut ports = Ports::default();
        for (is_input, list) in [(true, &mut ports.inputs), (false, &mut ports.outputs)] {
            for index in 0..ext.count(&handle, is_input) {
                let mut buffer = AudioPortInfoBuffer::new();
                if let Some(info) = ext.get(&handle, index, is_input, &mut buffer) {
                    list.push(Port {
                        name: String::from_utf8_lossy(info.name).into_owned(),
                        channels: info.channel_count,
                        is_main: info
                            .flags
                            .contains(clack_extensions::audio_ports::AudioPortFlags::IS_MAIN),
                    });
                }
            }
        }
        Some(ports)
    }

    /// The plugin's latency in samples, `None` without the extension.
    pub fn latency(&mut self) -> Option<u32> {
        let handle = self.instance.plugin_handle();
        let ext = handle.get_extension::<PluginLatency>()?;
        Some(ext.get(&handle))
    }

    /// Activate for the send chain's format and start processing. The returned processor
    /// may move to the pump thread; the instance stays on the main thread.
    pub fn start(mut self, channels: usize) -> Result<Running, String> {
        let config = PluginAudioConfiguration {
            sample_rate: SAMPLE_RATE,
            min_frames_count: BLOCK as u32,
            max_frames_count: BLOCK as u32,
        };
        let stopped = self
            .instance
            .activate(|_, _| (), config)
            .map_err(|e| format!("activate: {e}"))?;
        let processor = stopped
            .start_processing()
            .map_err(|e| format!("start processing: {e}"))?;
        Ok(Running {
            instance: self.instance,
            processor: Some(processor),
            inputs: AudioPorts::with_capacity(channels, 1),
            outputs: AudioPorts::with_capacity(channels, 1),
            channels,
            input: vec![vec![0.0; BLOCK]; channels],
            output: vec![vec![0.0; BLOCK]; channels],
            steady_time: 0,
        })
    }
}

/// A plugin processing blocks. Dropping it stops and deactivates the plugin in order.
pub struct Running {
    instance: PluginInstance<YappaHost>,
    processor: Option<StartedPluginAudioProcessor<YappaHost>>,
    inputs: AudioPorts,
    outputs: AudioPorts,
    channels: usize,
    input: Vec<Vec<f32>>,
    output: Vec<Vec<f32>>,
    steady_time: u64,
}

impl Running {
    /// One block through the plugin, in place, all channels the same mono block (the send
    /// chain is mono; a stereo plugin gets the voice on both sides and we keep the left).
    /// No allocation after the first call: the port structures are reused.
    pub fn process(&mut self, block: &mut [f32]) -> Result<ProcessStatus, String> {
        for ch in &mut self.input {
            ch[..block.len()].copy_from_slice(block);
        }
        let processor = self.processor.as_mut().ok_or("plugin stopped")?;
        let input_audio = self.inputs.with_input_buffers([AudioPortBuffer {
            latency: 0,
            channels: AudioPortBufferType::f32_input_only(
                self.input.iter_mut().map(InputChannel::variable),
            ),
        }]);
        let mut output_audio = self.outputs.with_output_buffers([AudioPortBuffer {
            latency: 0,
            channels: AudioPortBufferType::f32_output_only(
                self.output.iter_mut().map(|c| c.as_mut_slice()),
            ),
        }]);
        let status = processor
            .process(
                &input_audio,
                &mut output_audio,
                &InputEvents::empty(),
                &mut OutputEvents::void(),
                Some(self.steady_time),
                None,
            )
            .map_err(|e| format!("process: {e}"))?;
        self.steady_time += block.len() as u64;
        block.copy_from_slice(&self.output[0][..block.len()]);
        Ok(status)
    }

    pub fn channels(&self) -> usize {
        self.channels
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(processor) = self.processor.take() {
            let stopped = processor.stop_processing();
            self.instance.deactivate(stopped);
        }
    }
}

/// Per-block cost over `blocks` blocks of a 440 Hz tone: mean, p99 and max, plus the RMS
/// in and out of the first and last block — the numbers the spike document wants.
#[derive(Debug, Clone, PartialEq)]
pub struct Measurement {
    pub blocks: usize,
    pub mean: Duration,
    pub p99: Duration,
    pub max: Duration,
    pub rms_in: f32,
    pub rms_out: f32,
}

pub fn measure(running: &mut Running, blocks: usize) -> Result<Measurement, String> {
    let mut times = Vec::with_capacity(blocks);
    let mut rms_in = 0.0;
    let mut rms_out = 0.0;
    for b in 0..blocks {
        let mut block: Vec<f32> = (0..BLOCK)
            .map(|i| {
                let t = (b * BLOCK + i) as f32 / SAMPLE_RATE as f32;
                (std::f32::consts::TAU * 440.0 * t).sin() * 0.5
            })
            .collect();
        rms_in = dsp::rms(&block);
        let started = Instant::now();
        running.process(&mut block)?;
        times.push(started.elapsed());
        rms_out = dsp::rms(&block);
    }
    times.sort();
    let sum: Duration = times.iter().sum();
    Ok(Measurement {
        blocks,
        mean: sum / blocks as u32,
        p99: times[(blocks * 99 / 100).min(blocks - 1)],
        max: *times.last().unwrap(),
        rms_in,
        rms_out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin;

    #[test]
    fn the_builtin_plugin_is_described_with_mono_ports_and_no_latency() {
        let loaded = Loaded::builtin().unwrap();
        let described = loaded.describe();
        assert_eq!(described.len(), 1);
        assert_eq!(described[0].id, plugin::ID.to_str().unwrap());
        assert_eq!(described[0].name, "yAPPA Spike Gain");
        let mut instance = loaded.instantiate(&described[0].id).unwrap();
        let ports = instance.ports().expect("audio-ports extension");
        assert_eq!(ports.inputs.len(), 1);
        assert_eq!(ports.outputs.len(), 1);
        assert_eq!(ports.inputs[0].channels, 1);
        assert!(ports.inputs[0].is_main);
        assert_eq!(instance.latency(), Some(0));
    }

    #[test]
    fn blocks_go_through_the_plugin_and_come_back_at_minus_6_db() {
        let loaded = Loaded::builtin().unwrap();
        let instance = loaded.instantiate(plugin::ID.to_str().unwrap()).unwrap();
        let mut running = instance.start(1).unwrap();
        let mut block = vec![0.5f32; BLOCK];
        running.process(&mut block).unwrap();
        assert!(block.iter().all(|s| (*s - 0.25).abs() < 1e-6));
        let m = measure(&mut running, 200).unwrap();
        assert!((m.rms_out / m.rms_in - plugin::GAIN).abs() < 1e-3);
        assert!(m.max < Duration::from_millis(5), "{m:?}");
    }

    #[test]
    fn an_unknown_plugin_id_is_an_error_not_a_crash() {
        let loaded = Loaded::builtin().unwrap();
        assert!(loaded.instantiate("dev.yappa.nope").is_err());
    }

    #[test]
    fn a_stereo_host_layout_still_works_with_the_mono_plugin() {
        // The plugin declares mono; a host that insists on two channels per port gets the
        // plugin's answer for the first channel pair and silence or the gain on the second.
        let loaded = Loaded::builtin().unwrap();
        let instance = loaded.instantiate(plugin::ID.to_str().unwrap()).unwrap();
        let mut running = instance.start(2).unwrap();
        let mut block = vec![0.5f32; BLOCK];
        running.process(&mut block).unwrap();
        assert!(block.iter().all(|s| (*s - 0.25).abs() < 1e-6));
    }
}
