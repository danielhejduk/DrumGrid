use std::sync::{Arc};
use nih_plug::prelude::*;

pub struct Gain {
    pub(crate) params: Arc<GainParams>,
}

#[derive(Params)]
pub struct GainParams {
    #[id = "attack"]
    pub attack: FloatParam,
    #[id = "release"]
    pub release: FloatParam,
}

impl Default for Gain {
    fn default() -> Self {
        Self {
            params: Arc::new(GainParams::default()),
        }
    }
}
