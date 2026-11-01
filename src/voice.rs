use crate::params::ScanMode;
use crate::sound::Sample;
use nih_plug::util;

/// Longest crossfade between consecutive cycles, in host samples.
const MAX_XFADE: f32 = 64.0;

#[derive(Clone, Copy)]
pub struct VoiceParams {
    pub host_rate: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub start: f32,
    pub end: f32,
    pub scan: f32,
    pub scan_mode: ScanMode,
    pub jitter: f32,
    /// Fraction of each cycle (0..=1) that sounds.
    pub grain: f32,
    pub speed: f32,
    pub pitch: f32,
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
    /// Per-note tuning in semitones, from CLAP/VST3 note expressions.
    tuning: f32,
    /// Total pitch offset `period` was computed for.
    offset: f32,
    /// Host samples since the cycle started.
    phase: f32,
    /// Read position in sample frames.
    pos: f64,
    last_gain: f32,
    /// The previous cycle, fading out.
    fade_pos: f64,
    fade_gain: f32,
    fade_dec: f32,
    travel: f64,
    dir: f64,
    rng: u32,
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

fn region(sample: &Sample, p: &VoiceParams) -> (f64, f64, f64) {
    let len = sample.frames.len() as f64;
    let start = p.start.clamp(0.0, 1.0) as f64 * len;
    let end = p.end.clamp(0.0, 1.0) as f64 * len;
    (start, start.min(end), start.max(end))
}

fn period_for(note: u8, offset: f32, host_rate: f32) -> f32 {
    (host_rate / util::f32_midi_note_to_freq(note as f32 + offset)).max(1.0)
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
            period: period_for(note, p.pitch, p.host_rate),
            offset: p.pitch,
            pos: region(sample, p).0,
            fade_pos: self.fade_pos,
            fade_gain: self.fade_gain,
            fade_dec: self.fade_dec,
            dir: 1.0,
            rng: self.rng ^ (note as u32).wrapping_mul(0x9E37_79B9),
            ..Self::default()
        };
    }

    fn next_rand(&mut self) -> f32 {
        self.rng = self.rng.wrapping_mul(1_644_525).wrapping_add(1_013_904_223);
        (self.rng >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }

    fn advance_scan(&mut self, sample: &Sample, p: &VoiceParams, step: f64) {
        let (start, lo, mut hi) = region(sample, p);
        if hi - lo < 1.0 {
            self.travel = 0.0;
            return;
        }
        if p.scan_mode == ScanMode::Hold {
            hi = (hi - self.period as f64 * step).max(lo);
        }

        let scan = p.scan as f64;
        let mut x = start + self.travel + scan * (sample.rate / p.host_rate) as f64 * self.dir;
        if x < lo || x > hi {
            let width = hi - lo;
            x = match p.scan_mode {
                ScanMode::Hold => x.clamp(lo, hi),
                ScanMode::Loop if width > 0.0 => lo + (x - lo).rem_euclid(width),
                ScanMode::Loop => lo,
                ScanMode::PingPong => {
                    let (edge, inward) = if x > hi { (hi, -1.0) } else { (lo, 1.0) };
                    self.dir = inward * scan.signum();
                    (2.0 * edge - x).clamp(lo, hi)
                }
            }
        }
        self.travel = x - start;
    }

    pub fn set_tuning(&mut self, semitones: f32) {
        self.tuning = semitones;
    }

    fn retune(&mut self, p: &VoiceParams) {
        let offset = p.pitch + self.tuning;
        if offset != self.offset {
            self.offset = offset;
            self.period = period_for(self.note, offset, p.host_rate);
        }
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

        self.retune(p);
        self.advance_scan(sample, p, step);

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
            let jitter = if p.jitter > 0.0 {
                self.next_rand() as f64 * p.jitter as f64 * self.period as f64 * step
            } else {
                0.0
            };
            let (start, ..) = region(sample, p);
            self.pos = (start + self.travel + jitter).max(0.0) + self.phase as f64 * step;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn params() -> VoiceParams {
        VoiceParams {
            host_rate: RATE,
            attack_ms: 1.0,
            release_ms: 10.0,
            start: 0.0,
            end: 1.0,
            scan: 0.0,
            scan_mode: ScanMode::Loop,
            jitter: 0.0,
            grain: 1.0,
            speed: 1.0,
            pitch: 0.0,
        }
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

    fn best_lag(out: &[f32], lags: std::ops::Range<usize>) -> usize {
        let corr = |lag: usize| -> f32 {
            let (a, b) = (&out[..out.len() - lag], &out[lag..]);
            let dot = |x: &[f32], y: &[f32]| x.iter().zip(y).map(|(x, y)| x * y).sum::<f32>();
            dot(a, b) / (dot(a, a) * dot(b, b)).sqrt()
        };
        lags.max_by(|&a, &b| corr(a).total_cmp(&corr(b))).unwrap()
    }

    #[test]
    fn a4_repeats_at_440_hz() {
        let (s, p) = (noise_sample(), params());
        let mut v = Voice::default();
        v.start(69, 1.0, &s, &p);
        let best = best_lag(&render(&mut v, &s, &p, 9600)[1000..], 40..200);
        assert!((best as f32 - RATE / 440.0).abs() <= 1.0, "best lag {best}");
    }

    #[test]
    fn pitch_offset_retunes_a_held_note() {
        let s = noise_sample();
        let mut v = Voice::default();
        v.start(69, 1.0, &s, &params());
        render(&mut v, &s, &params(), 2000);

        // Bending up an octave mid-note: A4 becomes A5.
        let up = VoiceParams { pitch: 12.0, ..params() };
        let best = best_lag(&render(&mut v, &s, &up, 9600)[1000..], 30..100);
        assert!((v.period - RATE / 880.0).abs() < 1e-3, "period {}", v.period);
        assert!((best as f32 - RATE / 880.0).abs() <= 1.0, "best lag {best}");

        // Per-note tuning stacks on top: down a fifth from A5 is D5.
        v.set_tuning(-7.0);
        let best = best_lag(&render(&mut v, &s, &up, 9600)[1000..], 30..150);
        let d5 = util::f32_midi_note_to_freq(74.0);
        assert!((v.period - RATE / d5).abs() < 1e-3, "period {}", v.period);
        assert!((best as f32 - RATE / d5).abs() <= 1.0, "best lag {best}");
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

    fn scan_positions(v: &mut Voice, s: &Sample, p: &VoiceParams, n: usize) -> Vec<f64> {
        let start = region(s, p).0;
        (0..n)
            .map(|_| {
                v.next(s, p);
                start + v.travel
            })
            .collect()
    }

    #[test]
    fn scan_at_100_percent_moves_one_second_per_second() {
        let (s, p) = (noise_sample(), VoiceParams { scan: 1.0, ..params() });
        let mut v = Voice::default();
        v.start(60, 1.0, &s, &p);
        render(&mut v, &s, &p, 24_000);
        assert!((v.travel - 24_000.0).abs() < 1.0, "travel {}", v.travel);
    }

    #[test]
    fn loop_stays_inside_region() {
        let (s, p) = (
            noise_sample(),
            VoiceParams {
                start: 0.1,
                end: 0.2,
                scan: 2.0,
                ..params()
            },
        );
        let (_, lo, hi) = region(&s, &p);
        let mut v = Voice::default();
        v.start(60, 1.0, &s, &p);
        let xs = scan_positions(&mut v, &s, &p, 96_000);
        assert!(xs.iter().all(|x| (lo..=hi).contains(x)));
        // Each step either advances by the scan rate or wraps back to the region start.
        let width = hi - lo;
        assert!(xs.windows(2).all(|w| {
            let d = w[1] - w[0];
            (d - 2.0).abs() < 1e-6 || (d - (2.0 - width)).abs() < 1e-6
        }));
        assert!(xs.windows(2).any(|w| w[1] < w[0]), "never wrapped");
    }

    #[test]
    fn ping_pong_reverses() {
        let (s, p) = (
            noise_sample(),
            VoiceParams {
                start: 0.1,
                end: 0.2,
                scan: 2.0,
                scan_mode: ScanMode::PingPong,
                ..params()
            },
        );
        let (_, lo, hi) = region(&s, &p);
        let mut v = Voice::default();
        v.start(60, 1.0, &s, &p);
        let xs = scan_positions(&mut v, &s, &p, 96_000);
        assert!(xs.iter().all(|x| (lo..=hi).contains(x)));
        // No jumps: the position only ever moves by the scan rate.
        assert!(xs.windows(2).all(|w| (w[1] - w[0]).abs() <= 2.0 + 1e-6));
        assert!(xs.windows(2).any(|w| w[1] < w[0]), "never reversed");
    }

    #[test]
    fn hold_stops_one_cycle_before_end() {
        let (s, p) = (
            noise_sample(),
            VoiceParams {
                start: 0.1,
                end: 0.2,
                scan: 2.0,
                scan_mode: ScanMode::Hold,
                ..params()
            },
        );
        let (_, _, hi) = region(&s, &p);
        let mut v = Voice::default();
        v.start(60, 1.0, &s, &p);
        let xs = scan_positions(&mut v, &s, &p, 96_000);
        let held = hi - v.period as f64;
        assert!((xs[xs.len() - 1] - held).abs() < 1e-6, "ended at {}", xs[xs.len() - 1]);
        assert_eq!(xs[xs.len() - 1], xs[xs.len() / 2]);
    }

    #[test]
    fn jitter_changes_the_output_and_never_reads_before_the_sample() {
        let s = noise_sample();
        let run = |jitter: f32| {
            let p = VoiceParams { jitter, ..params() };
            let mut v = Voice::default();
            v.start(57, 1.0, &s, &p);
            (0..20_000)
                .map(|_| {
                    let out = v.next(&s, &p)[0];
                    assert!(v.pos >= 0.0);
                    out
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(run(0.0), run(0.0));
        assert_ne!(run(0.0), run(0.5));
    }
}
