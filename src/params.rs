use std::sync::{Arc};
use nih_plug::prelude::*;

use crate::sound;

pub struct Drum {
    pub params: Arc<DrumParams>,
    pub sound: Vec<f32>,
    pub pos: usize,
    pub playing: bool,
    pub velocity: f32,
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
        Self {
            params: Arc::new(DrumParams::default()),
            sound: sound::load_wav(),
            pos: 0,
            playing: false,
            velocity: 0.0,
        }
    }
}
