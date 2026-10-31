use crate::sound::Sample;
use nih_plug::util;

/// Longest crossfade between consecutive cycles, in host samples.
const MAX_XFADE: f32 = 64.0;

#[derive(Clone, Copy)]
pub struct VoiceParams {
    pub host_rate: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub start_ms: f32,
    /// Fraction of each cycle (0..=1) that sounds.
    pub grain: f32,
    pub speed: f32,
}

/// One held note: restarts the sample `freq` times a second, turning it into a pitched tone.
#[derive(Clone, Copy, Default)]
pub struct Voice {
    pub active: bool,
    pub note: u8,
    pub releasing: bool,
    pub env: f32,
    velocity: f32,
    /// Cycle length in host samples.
    period: f32,
    /// Host samples since the cycle started.
    phase: f32,
    /// Read position in sample frames.
    pos: f64,
    last_gain: f32,
    /// The previous cycle, fading out.
    fade_pos: f64,
    fade_gain: f32,
    fade_dec: f32,
}

pub fn read_frame(sound: &[[f32; 2]], pos: f64) -> Option<[f32; 2]> {
    let idx = pos as usize;
    if idx + 1 >= sound.len() {
        return None;
    }
    let frac = (pos - idx as f64) as f32;
    let (a, b) = (sound[idx], sound[idx + 1]);
    Some([a[0] + (b[0] - a[0]) * frac, a[1] + (b[1] - a[1]) * frac])
}

fn start_frames(sample: &Sample, p: &VoiceParams) -> f64 {
    (p.start_ms / 1000.0 * sample.rate) as f64
}

fn xfade_len(period: f32) -> f32 {
    (period * 0.25).clamp(1.0, MAX_XFADE)
}

impl Voice {
    pub fn start(&mut self, note: u8, velocity: f32, sample: &Sample, p: &VoiceParams) {
        if self.active {
            self.begin_fade();
        }
        *self = Self {
            active: true,
            note,
            velocity,
            period: (p.host_rate / util::midi_note_to_freq(note)).max(1.0),
            pos: start_frames(sample, p),
            fade_pos: self.fade_pos,
            fade_gain: self.fade_gain,
            fade_dec: self.fade_dec,
            ..Self::default()
        };
    }

    pub fn release(&mut self) {
        self.releasing = true;
    }

    pub fn stop(&mut self) {
        *self = Self::default();
    }

    fn begin_fade(&mut self) {
        self.fade_pos = self.pos;
        self.fade_gain = self.last_gain;
        self.fade_dec = 1.0 / xfade_len(self.period);
    }

    pub fn next(&mut self, sample: &Sample, p: &VoiceParams) -> [f32; 2] {
        let mut out = [0.0f32; 2];
        let step = sample.rate as f64 / p.host_rate as f64 * p.speed as f64;

        if self.fade_gain > 0.0 {
            if let Some(f) = read_frame(&sample.frames, self.fade_pos) {
                out = [f[0] * self.fade_gain, f[1] * self.fade_gain];
            }
            self.fade_pos += step;
            self.fade_gain = (self.fade_gain - self.fade_dec).max(0.0);
        }
        if !self.active {
            return out;
        }

        let samples_per_ms = p.host_rate / 1000.0;
        if self.releasing {
            self.env -= 1.0 / (p.release_ms * samples_per_ms).max(1.0);
            if self.env <= 0.0 {
                self.active = false;
                self.env = 0.0;
                self.last_gain = 0.0;
                return out;
            }
        } else {
            self.env = (self.env + 1.0 / (p.attack_ms * samples_per_ms).max(1.0)).min(1.0);
        }

        // Fade in at the cycle start; fade out at the end of a shortened grain.
        let mut window = ((self.phase + 1.0) / xfade_len(self.period)).min(1.0);
        if p.grain < 0.999 {
            let grain_len = self.period * p.grain;
            let ramp = (grain_len * 0.25).clamp(1.0, 32.0);
            window *= ((grain_len - self.phase) / ramp).clamp(0.0, 1.0);
        }

        let gain = window * self.env * self.velocity;
        self.last_gain = gain;
        if gain > 0.0
            && let Some(f) = read_frame(&sample.frames, self.pos)
        {
            out[0] += f[0] * gain;
            out[1] += f[1] * gain;
        }

        self.pos += step;
        self.phase += 1.0;
        if self.phase >= self.period {
            self.phase -= self.period;
            self.begin_fade();
            // Keep the fractional phase so non-integer periods stay in tune.
            self.pos = start_frames(sample, p) + self.phase as f64 * step;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn params() -> VoiceParams {
        VoiceParams { host_rate: RATE, attack_ms: 1.0, release_ms: 10.0, start_ms: 0.0, grain: 1.0, speed: 1.0 }
    }

    /// Decaying pseudo-random noise standing in for a drum hit.
    fn noise_sample() -> Sample {
        let mut x: u32 = 12345;
        let frames = (0..RATE as usize)
            .map(|i| {
                x = x.wrapping_mul(1664525).wrapping_add(1013904223);
                let v = ((x >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0) * (-(i as f32) / 4000.0).exp();
                [v, v]
            })
            .collect();
        Sample { frames, rate: RATE }
    }

    fn render(v: &mut Voice, s: &Sample, p: &VoiceParams, n: usize) -> Vec<f32> {
        (0..n).map(|_| v.next(s, p)[0]).collect()
    }

    #[test]
    fn read_frame_interpolates_and_stops_at_end() {
        let s = [[0.0, 0.0], [1.0, 2.0], [2.0, 4.0]];
        assert_eq!(read_frame(&s, 0.5), Some([0.5, 1.0]));
        assert_eq!(read_frame(&s, 2.0), None);
    }

    #[test]
    fn a4_repeats_at_440_hz() {
        let (s, p) = (noise_sample(), params());
        let mut v = Voice::default();
        v.start(69, 1.0, &s, &p);
        let out = &render(&mut v, &s, &p, 9600)[1000..];

        let corr = |lag: usize| -> f32 {
            let (a, b) = (&out[..out.len() - lag], &out[lag..]);
            let dot = |x: &[f32], y: &[f32]| x.iter().zip(y).map(|(x, y)| x * y).sum::<f32>();
            dot(a, b) / (dot(a, a) * dot(b, b)).sqrt()
        };
        let best = (40..400).max_by(|&a, &b| corr(a).total_cmp(&corr(b))).unwrap();
        assert!((best as f32 - RATE / 440.0).abs() <= 1.0, "best lag {best}");
    }

    #[test]
    fn short_grain_leaves_silence_in_each_cycle() {
        let (s, p) = (noise_sample(), VoiceParams { grain: 0.5, ..params() });
        let mut v = Voice::default();
        v.start(57, 1.0, &s, &p);
        let out = &render(&mut v, &s, &p, 20_000)[1000..];
        let silent = out.iter().filter(|x| **x == 0.0).count() as f32 / out.len() as f32;
        assert!((0.4..0.6).contains(&silent), "silent fraction {silent}");
    }

    #[test]
    fn release_ends_the_voice() {
        let (s, p) = (noise_sample(), params());
        let mut v = Voice::default();
        v.start(60, 1.0, &s, &p);
        render(&mut v, &s, &p, 1000);
        v.release();
        render(&mut v, &s, &p, 500);
        assert!(!v.active);
    }
}
