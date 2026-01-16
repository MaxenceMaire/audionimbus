// Produce samples following a sine wave.
pub fn sine_wave(frequency: f32, amplitude: f32, duration_secs: f32, sample_rate: u32) -> Vec<f32> {
    let num_samples = (duration_secs * sample_rate as f32) as usize;
    let phase_increment = 2.0 * std::f32::consts::PI * frequency / sample_rate as f32;

    (0..num_samples)
        .map(|i| {
            let phase = i as f32 * phase_increment;
            amplitude * phase.sin()
        })
        .collect()
}
