// Bugs in nih-plug
#![allow(semicolon_in_expressions_from_macros)]
#![allow(future_incompatible)]

use nih_plug::prelude::*;
use std::sync::{Arc};
use crate::params::GainParams;

mod params;

impl Default for GainParams {
    fn default() -> Self {
        Self {
            attack: FloatParam::new(
                "Attack",
                0.0,
                FloatRange::Skewed { min: 0.0, max: 2.0, factor: FloatRange::skew_factor(-2.0) },
            ),
            release: FloatParam::new(
                "Release",
                0.0,
                FloatRange::Skewed { min: 0.0, max: 2.0, factor: FloatRange::skew_factor(-2.0) },
            )
        }
    }
}

impl Plugin for params::Gain {
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

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        ProcessStatus::Normal
    }

    fn deactivate(&mut self) {}
}

impl ClapPlugin for params::Gain {
    const CLAP_ID: &'static str = "tech.hejduk.drumgrid";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Making sounds out of drum samples");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for params::Gain {
    const VST3_CLASS_ID: [u8; 16] = *b"DrumGridTechPlug";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}

nih_export_clap!(params::Gain);
nih_export_vst3!(params::Gain);
