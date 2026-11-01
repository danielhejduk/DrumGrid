use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use parking_lot::RwLock;
use std::sync::Arc;

use crate::sound::{self, Sample, SampleSlot};
use crate::voice::Voice;

pub const NUM_VOICES: usize = 16;

pub struct Drum {
    pub params: Arc<DrumParams>,
    pub sample: Sample,
    pub slot: Arc<SampleSlot>,
    pub host_rate: f32,
    pub voices: [Voice; NUM_VOICES],
}

#[derive(Params)]
pub struct DrumParams {
    /// Empty means the built-in sample.
    #[persist = "sample-path"]
    pub sample_path: Arc<RwLock<String>>,
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,
    /// Base64 of the original file, so projects survive the file moving. Empty when the
    /// sample is built-in or larger than `sound::MAX_EMBED_BYTES`.
    #[persist = "sample-data"]
    pub sample_data: Arc<RwLock<String>>,

    #[id = "gain"]
    pub gain: FloatParam,
    #[id = "attack"]
    pub attack: FloatParam,
    #[id = "release"]
    pub release: FloatParam,
    #[id = "start"]
    pub start: FloatParam,
    #[id = "grain"]
    pub grain: FloatParam,
    #[id = "speed"]
    pub speed: FloatParam,
}

fn ms_param(name: &str, default: f32, min: f32, max: f32, skew: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Skewed { min, max, factor: FloatRange::skew_factor(skew) })
        .with_unit(" ms")
        .with_value_to_string(formatters::v2s_f32_rounded(2))
}

impl Default for DrumParams {
    fn default() -> Self {
        Self {
            sample_path: Arc::new(RwLock::new(String::new())),
            editor_state: EguiState::from_size(420, 320),
            sample_data: Arc::new(RwLock::new(String::new())),

            gain: FloatParam::new(
                "Gain",
                util::db_to_gain(-6.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-36.0),
                    max: util::db_to_gain(6.0),
                    factor: FloatRange::gain_skew_factor(-36.0, 6.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            attack: ms_param("Attack", 10.0, 0.1, 1000.0, -1.5),
            release: ms_param("Release", 100.0, 0.1, 1000.0, -1.5),
            start: ms_param("Start", 0.0, 0.0, 500.0, -1.0),
            grain: FloatParam::new("Grain", 1.0, FloatRange::Linear { min: 0.05, max: 1.0 })
                .with_unit("%")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            speed: FloatParam::new(
                "Speed",
                1.0,
                FloatRange::Skewed { min: 0.25, max: 4.0, factor: FloatRange::gain_skew_factor(-12.04, 12.04) },
            )
            .with_unit("x")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
        }
    }
}

impl Default for Drum {
    fn default() -> Self {
        let sample = sound::load_embedded();
        Self {
            params: Arc::new(DrumParams::default()),
            host_rate: sample.rate,
            sample,
            slot: Arc::new(SampleSlot::new(sound::EMBEDDED_NAME)),
            voices: [Voice::default(); NUM_VOICES],
        }
    }
}
