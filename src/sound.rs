/// Loads the sample as stereo frames. Mono files are duplicated to both channels.
pub(crate) fn load_wav() -> (Vec<[f32; 2]>, f32) {
    let bytes = include_bytes!("../sounds/click.wav");
    let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
    let spec = reader.spec();

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => {
            reader.samples::<f32>().map(|s| s.unwrap()).collect()
        }
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|s| s.unwrap() as f32 / max).collect()
        }
    };

    let channels = spec.channels as usize;
    let frames = samples
        .chunks_exact(channels)
        .map(|frame| if channels == 1 { [frame[0], frame[0]] } else { [frame[0], frame[1]] })
        .collect();
    (frames, spec.sample_rate as f32)
}
