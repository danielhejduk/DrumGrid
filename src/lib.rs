#![allow(semicolon_in_expressions_from_macros)]

use nih_plug::prelude::*;
use std::path::Path;
use std::sync::Arc;

use crate::params::Drum;
use crate::voice::VoiceParams;

mod editor;
mod params;
mod sound;
mod voice;

impl Drum {
    fn note_on(&mut self, note: u8, velocity: f32, vp: &VoiceParams) {
        let idx = self
            .voices
            .iter()
            .position(|v| v.active && v.note == note)
            .or_else(|| self.voices.iter().position(|v| !v.active))
            // All busy: steal the quietest voice, preferring released ones.
            .unwrap_or_else(|| {
                (0..self.voices.len())
                    .min_by(|&a, &b| {
                        let key = |i: usize| (!self.voices[i].releasing, self.voices[i].env);
                        key(a).partial_cmp(&key(b)).unwrap()
                    })
                    .unwrap()
            });
        self.voices[idx].start(note, velocity, &self.sample, vp);
    }

    fn voice_params(&self) -> VoiceParams {
        VoiceParams {
            host_rate: self.host_rate,
            attack_ms: self.params.attack.value(),
            release_ms: self.params.release.value(),
            start: self.params.start.value(),
            end: self.params.end.value(),
            scan: self.params.scan.value(),
            scan_mode: self.params.scan_mode.value(),
            jitter: self.params.jitter.value(),
            grain: self.params.grain.value(),
            speed: self.params.speed.smoothed.next(),
            pitch: self.params.tune.value() as f32
                + self.params.fine.smoothed.next() / 100.0
                + self.bend * self.params.bend_range.value() as f32,
        }
    }
}

impl Plugin for Drum {
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

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.slot.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.host_rate = buffer_config.sample_rate;

        // Runs after a saved project is restored, so reload the user's sample here.
        // The embedded copy wins; the path is only a fallback for files too large to embed.
        let path = self.params.sample_path.read().clone();
        let data = self.params.sample_data.read().clone();
        let (sample, name) = if path.is_empty() && data.is_empty() {
            (sound::load_embedded(), sound::EMBEDDED_NAME.to_owned())
        } else {
            let path = Path::new(&path);
            let loaded = if data.is_empty() {
                sound::load_file(path).map_err(|e| e.to_string())
            } else {
                sound::load_embedded_data(&data)
            };
            match loaded {
                Ok(s) => {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
                    (s, name.unwrap_or_else(|| "Embedded sample".to_owned()))
                }
                Err(e) => {
                    nih_log!("Couldn't load {}: {e}; using the built-in sample", path.display());
                    (sound::load_embedded(), sound::EMBEDDED_NAME.to_owned())
                }
            }
        };
        self.sample = sample;
        self.slot.describe(name, &self.sample);
        true
    }

    fn reset(&mut self) {
        self.voices.iter_mut().for_each(|v| v.stop());
        self.bend = 0.0;
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.slot.try_swap(&mut self.sample);
        let mut next_event = context.next_event();

        for (sample_id, channel_samples) in buffer.iter_samples().enumerate() {
            let vp = self.voice_params();

            while let Some(event) = next_event {
                if event.timing() > sample_id as u32 {
                    break;
                }
                match event {
                    NoteEvent::NoteOn { note, velocity, .. } => self.note_on(note, velocity, &vp),
                    NoteEvent::NoteOff { note, .. } => self
                        .voices
                        .iter_mut()
                        .filter(|v| v.active && v.note == note)
                        .for_each(|v| v.release()),
                    NoteEvent::Choke { note, .. } => {
                        self.voices.iter_mut().filter(|v| v.note == note).for_each(|v| v.stop())
                    }
                    NoteEvent::MidiPitchBend { value, ..} => self.bend = (value * 2.0 - 1.0).clamp(-1.0, 1.0),
                    NoteEvent::PolyTuning { note, tuning, .. } => self
                        .voices
                        .iter_mut()
                        .filter(|v| v.active && v.note == note)
                        .for_each(|v| v.set_tuning(tuning)),
                    _ => {}
                }
                next_event = context.next_event();
            }

            let frame = self.voices.iter_mut().fold([0.0f32; 2], |acc, v| {
                let out = v.next(&self.sample, &vp);
                [acc[0] + out[0], acc[1] + out[1]]
            });
            let gain = self.params.gain.smoothed.next();

            let channels = channel_samples.len();
            for (ch, sample) in channel_samples.into_iter().enumerate() {
                let s = if channels == 1 {
                    (frame[0] + frame[1]) * 0.5
                } else {
                    frame[ch.min(1)]
                };
                *sample = s * gain;
            }
        }
        ProcessStatus::Normal
    }

    fn deactivate(&mut self) {}
}

impl ClapPlugin for Drum {
    const CLAP_ID: &'static str = "tech.hejduk.drumgrid";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Turns any sample into a playable tone by retriggering it at the note's frequency");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
        ClapFeature::Mono,
    ];
}

impl Vst3Plugin for Drum {
    const VST3_CLASS_ID: [u8; 16] = *b"DrumGridTechPlug";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(Drum);
nih_export_vst3!(Drum);
