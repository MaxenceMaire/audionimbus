use audionimbus::Sample;

/// A repeating tone-burst generator.
///
/// `on_secs` seconds of a sine wave followed by `off_secs` seconds of silence.
pub struct ToneBurst {
    phase: f32,
    phase_increment: f32,
    amplitude: f32,
    /// Total cycle length in samples.
    cycle_samples: u32,
    /// Number of samples within each cycle that carry a non-zero signal.
    on_samples: u32,
    /// Current position within the cycle.
    cursor: u32,
    /// Duration of the fade-in and fade-out in samples.
    fade_samples: u32,
}

impl ToneBurst {
    /// Creates a new tone-burst generator.
    pub fn new(
        frequency: f32,
        amplitude: f32,
        on_secs: f32,
        off_secs: f32,
        sample_rate: u32,
    ) -> Self {
        let phase_increment = std::f32::consts::TAU * frequency / sample_rate as f32;
        let on_samples = (on_secs * sample_rate as f32) as u32;
        let off_samples = (off_secs * sample_rate as f32) as u32;
        let fade_samples = (0.02 * sample_rate as f32) as u32;

        Self {
            phase: 0.0,
            phase_increment,
            amplitude,
            cycle_samples: on_samples + off_samples,
            on_samples,
            cursor: 0,
            fade_samples,
        }
    }

    /// Fills `buffer` with the next block of samples.
    pub fn fill(&mut self, buffer: &mut [Sample]) {
        for sample in buffer.iter_mut() {
            if self.cursor < self.on_samples {
                let remaining = self.on_samples - self.cursor;
                let fade_gain = if self.cursor < self.fade_samples {
                    self.cursor as f32 / self.fade_samples as f32
                } else if remaining <= self.fade_samples {
                    remaining as f32 / self.fade_samples as f32
                } else {
                    1.0
                };
                *sample = self.amplitude * self.phase.sin() * fade_gain;
                self.phase = (self.phase + self.phase_increment).rem_euclid(std::f32::consts::TAU);
            } else {
                *sample = 0.0;
            }
            self.cursor = (self.cursor + 1) % self.cycle_samples;
        }
    }
}

/// Adds `source`, scaled by `gain`, into `destination`.
///
/// Both slices must have the same length and channel layout.
pub fn mix_into(destination: &mut [Sample], source: &[Sample], gain: f32) {
    for (destination_sample, source_sample) in destination.iter_mut().zip(source) {
        *destination_sample += source_sample * gain;
    }
}
