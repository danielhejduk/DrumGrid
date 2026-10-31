use parking_lot::Mutex;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

pub const EMBEDDED_NAME: &str = "click.wav (built-in)";

pub struct Sample {
    pub frames: Vec<[f32; 2]>,
    pub rate: f32,
}

/// Decodes a WAV into stereo frames. Mono files are duplicated to both channels.
pub fn decode_wav<R: Read>(reader: R) -> Result<Sample, hound::Error> {
    let mut reader = hound::WavReader::new(reader)?;
    let spec = reader.spec();

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|s| s.map(|s| s as f32 / max)).collect::<Result<_, _>>()?
        }
    };

    let channels = spec.channels.max(1) as usize;
    let frames = samples
        .chunks_exact(channels)
        .map(|f| if channels == 1 { [f[0], f[0]] } else { [f[0], f[1]] })
        .collect();
    Ok(Sample { frames, rate: spec.sample_rate as f32 })
}

pub fn load_embedded() -> Sample {
    decode_wav(std::io::Cursor::new(include_bytes!("../sounds/click.wav"))).expect("embedded sample is valid")
}

pub fn load_file(path: &Path) -> Result<Sample, hound::Error> {
    decode_wav(std::io::BufReader::new(std::fs::File::open(path)?))
}

/// Passes samples loaded in the GUI to the audio thread. The audio thread swaps the pending
/// sample with its current one, so the old buffer is freed later on the GUI thread, never in
/// `process()`.
pub struct SampleSlot {
    pending: Mutex<Option<Sample>>,
    ready: AtomicBool,
    pub name: Mutex<String>,
}

impl SampleSlot {
    pub fn new(name: &str) -> Self {
        Self { pending: Mutex::new(None), ready: AtomicBool::new(false), name: Mutex::new(name.to_owned()) }
    }

    /// GUI thread.
    pub fn offer(&self, sample: Sample, name: String) {
        let old = self.pending.lock().replace(sample);
        self.ready.store(true, Ordering::Release);
        drop(old);
        *self.name.lock() = name;
    }

    /// Audio thread. Never blocks; retries next block if the GUI holds the lock.
    pub fn try_swap(&self, current: &mut Sample) {
        if !self.ready.load(Ordering::Acquire) {
            return;
        }
        if let Some(mut pending) = self.pending.try_lock() {
            if let Some(new) = pending.as_mut() {
                std::mem::swap(new, current);
            }
            self.ready.store(false, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_sample_decodes() {
        let s = load_embedded();
        assert!(!s.frames.is_empty() && s.rate > 0.0);
    }

    #[test]
    fn slot_swaps_once() {
        let slot = SampleSlot::new("a");
        let mut current = Sample { frames: vec![[0.0; 2]; 4], rate: 1.0 };
        slot.offer(Sample { frames: vec![[1.0; 2]; 8], rate: 2.0 }, "b".into());
        slot.try_swap(&mut current);
        slot.try_swap(&mut current);
        assert_eq!(current.frames.len(), 8);
    }
}
