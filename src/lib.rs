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
                0.0,
                FloatRange::Skewed { min: 0.1, max: 1000.0, factor: FloatRange::skew_factor(-1.5) },
            )
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            release: FloatParam::new(
                "Release",
                0.0,
                FloatRange::Skewed { min: 0.1, max: 1000.0, factor: FloatRange::skew_factor(-1.5) },
            )
                .with_value_to_string(formatters::v2s_f32_rounded(2))
        }
    }
}

impl Plugin for params::Drum {
    const NAME: &'static str = "DrumGrid";
    const VENDOR: &'static str = "Daniel Hejduk";
    const URL: &'static str = "https://github.com/danielhejduk/drumgrid";
    const EMAIL: &'static str = "info@example.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),

            aux_input_ports: &[],
            aux_output_ports: &[],

            names: PortNames::const_default(),
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
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

    fn process(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut next_event = context.next_event();

        for (sample_id, channel_samples) in buffer.iter_samples().enumerate() {
            while let Some(event) = next_event {
                if event.timing() > sample_id as u32 {
                    break;
                }
                
                match event {
                    NoteEvent::NoteOn { velocity, ..} => {
                        self.pos = 0;
                        self.playing = true;
                        self.velocity = velocity;
                    }
                    _ => (),
                }
                next_event = context.next_event();
            }
            let value = if self.playing && self.pos < self.sound.len() {
                let v = self.sound[self.pos] * self.velocity;
                self.pos += 1;
                v
            } else {
                self.playing = false;
                0.0
            };
            for sample in channel_samples {
                *sample = value;
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
    ];
}

impl Vst3Plugin for params::Drum {
    const VST3_CLASS_ID: [u8; 16] = *b"DrumGridTechPlug";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument];
}

nih_export_clap!(params::Drum);
nih_export_vst3!(params::Drum);
