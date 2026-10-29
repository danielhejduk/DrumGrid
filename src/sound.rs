pub(crate) fn load_wav() -> Vec<f32> {
    let bytes = include_bytes!("../sounds/click.wav");
    let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
    let spec = reader.spec();

    match spec.sample_format {
        hound::SampleFormat::Float => {
            reader.samples::<f32>().map(|s| s.unwrap()).collect()
        }
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|s| s.unwrap() as f32 / max).collect()
        }
    }
}
