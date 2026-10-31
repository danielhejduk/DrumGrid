use std::sync::{Arc};
use nih_plug::prelude::*;

use crate::sound;

pub struct Drum {
    pub params: Arc<DrumParams>,
    pub sound: Vec<[f32; 2]>,
    pub sound_rate: f32,
    pub host_rate: f32,
    pub pos: f64,
    pub playing: bool,
    pub velocity: f32,
    pub env: f32,

    pub last_gain: f32,
    pub fade_pos: f64,
    pub fade_gain: f32,
}

#[derive(Params)]
pub struct DrumParams {
    #[id = "attack"]
    pub attack: FloatParam,
    #[id = "release"]
    pub release: FloatParam,
}

impl Default for Drum {
    fn default() -> Self {
        let (sound, sound_rate) = sound::load_wav();
        Self {
            params: Arc::new(DrumParams::default()),
            sound,
            sound_rate,
            host_rate: sound_rate,
            pos: 0.0,
            playing: false,
            velocity: 0.0,
            env: 0.0,

            last_gain: 0.0,
            fade_pos: 0.0,
            fade_gain: 0.0,
        }
    }
}
