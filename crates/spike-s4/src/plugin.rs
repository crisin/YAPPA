//! The test plugin: "yAPPA Spike Gain", one mono input, one mono output, output = input × 0.5
//! (−6 dB). It declares the `audio-ports` and `latency` extensions, which is what a host
//! needs to place an effect in a mono chain without guessing.

use clack_extensions::audio_ports::{
    AudioPortFlags, AudioPortInfo, AudioPortInfoWriter, AudioPortType, PluginAudioPorts,
    PluginAudioPortsImpl,
};
use clack_extensions::latency::{PluginLatency, PluginLatencyImpl};
use clack_plugin::prelude::*;
use clack_plugin::process::audio::ChannelPair;
use std::ffi::CStr;

/// The plugin id a host asks the factory for.
pub const ID: &CStr = c"dev.yappa.spike-s4.gain";
/// What the plugin does to the signal: a fixed −6 dB, easy to assert on the host side.
pub const GAIN: f32 = 0.5;

pub struct GainPlugin;

pub struct GainMainThread;
pub struct GainProcessor;

impl Plugin for GainPlugin {
    type AudioProcessor<'a> = GainProcessor;
    type Shared<'a> = ();
    type MainThread<'a> = GainMainThread;

    fn declare_extensions(builder: &mut PluginExtensions<Self>, _shared: Option<&()>) {
        builder
            .register::<PluginAudioPorts>()
            .register::<PluginLatency>();
    }
}

impl DefaultPluginFactory for GainPlugin {
    fn get_descriptor() -> PluginDescriptor {
        use clack_plugin::plugin::features::*;
        PluginDescriptor::new("dev.yappa.spike-s4.gain", "yAPPA Spike Gain")
            .with_vendor("yAPPA")
            .with_features([AUDIO_EFFECT, MONO])
    }

    fn new_shared(_host: HostSharedHandle<'_>) -> Result<(), PluginError> {
        Ok(())
    }

    fn new_main_thread<'a>(
        _host: HostMainThreadHandle<'a>,
        _shared: &'a (),
    ) -> Result<GainMainThread, PluginError> {
        Ok(GainMainThread)
    }
}

impl PluginMainThread<'_, ()> for GainMainThread {}

impl PluginAudioPortsImpl for GainMainThread {
    fn count(&self, _is_input: bool) -> u32 {
        1
    }

    fn get(&self, index: u32, is_input: bool, writer: &mut AudioPortInfoWriter) {
        if index == 0 {
            writer.set(&AudioPortInfo {
                id: ClapId::new(0),
                name: if is_input { b"Mono In" } else { b"Mono Out" },
                channel_count: 1,
                flags: AudioPortFlags::IS_MAIN,
                port_type: Some(AudioPortType::MONO),
                in_place_pair: Some(ClapId::new(0)),
            });
        }
    }
}

impl PluginLatencyImpl for GainMainThread {
    fn get(&self) -> u32 {
        0
    }
}

impl<'a> PluginAudioProcessor<'a, (), GainMainThread> for GainProcessor {
    fn activate(
        _host: HostAudioProcessorHandle<'a>,
        _main_thread: &GainMainThread,
        _shared: &'a (),
        _audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        Ok(GainProcessor)
    }

    fn process(
        &mut self,
        _process: Process,
        mut audio: Audio,
        _events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        for mut port in audio.port_pairs() {
            let mut channels = port
                .channels()?
                .into_f32()
                .ok_or(PluginError::Message("only f32 buffers"))?;
            for pair in channels.iter_mut() {
                match pair {
                    ChannelPair::InputOutput(input, output) => {
                        for (o, i) in output.iter_mut().zip(input) {
                            *o = i * GAIN;
                        }
                    }
                    ChannelPair::InPlace(io) => io.iter_mut().for_each(|s| *s *= GAIN),
                    ChannelPair::OutputOnly(output) => output.fill(0.0),
                    ChannelPair::InputOnly(_) => {}
                }
            }
        }
        Ok(ProcessStatus::Continue)
    }
}

/// The CLAP entry point of the `cdylib`: `clap_entry`, as any host looks it up.
pub type Entry = SinglePluginEntry<GainPlugin>;
clack_export_entry!(Entry);
