pub mod key;

const TWO_PI: f32 = std::f64::consts::PI as f32 * 2.0;

pub fn sinus_sound(frequency: usize, sample_rate: usize) -> Vec<f32> {
    let wave_length = sample_rate / frequency;
    (0..wave_length)
        .map(|i| {
            let progress = i as f32 / wave_length as f32;
            f32::sin(TWO_PI * progress)
        })
        .collect()
}

pub fn triangle_sound(frequency: usize, sample_rate: usize) -> Vec<f32> {
    let wave_length = sample_rate / frequency;
    (0..wave_length)
        .map(|i| {
            let progress = i as f32 / wave_length as f32;
            match progress {
                progress if progress < 0.25 => progress * 4.0,
                progress if progress < 0.75 => 1.0 - (progress - 0.25) * 4.0,
                progress => -1.0 + (progress - 0.75) * 4.0,
            }
        })
        .collect()
}
