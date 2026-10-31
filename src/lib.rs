// Bugs in nih-plug
#![allow(semicolon_in_expressions_from_macros)]
#![allow(future_incompatible)]

use nih_plug::prelude::*;
use std::sync::{Arc};
use crate::params::DrumParams;

mod params;
mod sound;

impl Default for DrumParams {
    fn default() -> Self {
        Self {
            attack: FloatParam::new(
                "Attack",
                10.0,
                FloatRange::Skewed { min: 0.1, max: 1000.0, factor: FloatRange::skew_factor(-1.5) },
            )
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            release: FloatParam::new(
                "Release",
                10.0,
                FloatRange::Skewed { min: 0.1, max: 1000.0, factor: FloatRange::skew_factor(-1.5) },
            )
                .with_value_to_string(formatters::v2s_f32_rounded(2))
        }
    }
}

fn read_frame(sound: &[[f32; 2]], pos: f64) -> Option<[f32; 2]> {
    let idx = pos as usize;
    if idx + 1 >= sound.len() {
        return None;
    }
    let frac = (pos - idx as f64) as f32;
    let (a, b) = (sound[idx], sound[idx + 1]);
    Some([a[0] + (b[0] - a[0]) * frac, a[1] + (b[1] - a[1]) * frac])
}

impl Plugin for params::Drum {
    const NAME: &'static str = "DrumGrid";
    const VENDOR: &'static str = "Daniel Hejduk";
    const URL: &'static str = "https://github.com/danielhejduk/drumgrid";
    const EMAIL: &'static str = "drumgrid@hejduk.tech";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: None,
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: None,
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool
    {
        self.host_rate = buffer_config.sample_rate;
        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut next_event = context.next_event();

        let step = self.sound_rate as f64 / self.host_rate as f64;
        let samples_per_ms = self.host_rate / 1000.0;
        let fade_dec = 1.0 / (2.0 * samples_per_ms);

        for (sample_id, channel_samples) in buffer.iter_samples().enumerate() {
            while let Some(event) = next_event {
                if event.timing() > sample_id as u32 {
                    break;
                }
                
                if let Some(NoteEvent::NoteOn { velocity, ..}) = next_event {
                    if self.playing {
                        self.fade_pos = self.pos;
                        self.fade_gain = self.last_gain;
                    }
                    self.pos = 0.0;
                    self.env = 0.0;
                    self.playing = true;
                    self.velocity = velocity;
                }
                next_event = context.next_event();
            }
            let idx = self.pos as usize;
            let frame = if self.playing && idx + 1 < self.sound.len() {
                let frac = (self.pos - idx as f64) as f32;
                let a = self.sound[idx];
                let b = self.sound[idx + 1];
                let f = [a[0] + (b[0] - a[0]) * frac, a[1] + (b[1] - a[1]) * frac];

                let attack_samples = (self.params.attack.value() * samples_per_ms).max(1.0);
                self.env = (self.env + 1.0 / attack_samples).min(1.0);
                let release_samples = (self.params.release.value() * samples_per_ms).max(1.0);
                let remaining = ((self.sound.len() - 1) as f64 - self.pos) / step;
                let release_gain = (remaining as f32 / release_samples).min(1.0);

                self.pos += step;

                let gain = self.velocity * self.env * release_gain;
                [f[0] * gain, f[1] * gain]
            } else {
                self.playing = false;
                [0.0, 0.0]
            };
            let channels = channel_samples.len();
            for (ch, sample) in channel_samples.into_iter().enumerate() {
                *sample = if channels == 1 { (frame[0] + frame[1]) * 0.5 } else { frame[ch.min(1)] };
            }
        }
        ProcessStatus::Normal
    }

    fn deactivate(&mut self) {}
}

impl ClapPlugin for params::Drum {
    const CLAP_ID: &'static str = "tech.hejduk.drumgrid";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Making sounds out of drum samples");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Drum,
    ];
}

impl Vst3Plugin for params::Drum {
    const VST3_CLASS_ID: [u8; 16] = *b"DrumGridTechPlug";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument];
}

nih_export_clap!(params::Drum);
nih_export_vst3!(params::Drum);
