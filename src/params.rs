use std::sync::{Arc};
use nih_plug::prelude::*;

pub struct Drum {
    pub(crate) params: Arc<DrumParams>,
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
        }
    }
}
