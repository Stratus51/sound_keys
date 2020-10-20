fn smallest_multiple(n: usize, x: usize) -> usize {
    let rem = n % x as usize;
    if rem > 0 {
        n - rem
    } else {
        n
    }
}

pub struct Key {
    pub start: Vec<f32>,
    pub maintain: Vec<f32>,
    pub stop: Vec<f32>,
    pub frame_size: usize,
}

impl Key {
    pub fn from_pattern(pattern: &[f32], start_nb_pattern: usize, stop_nb_pattern: usize) -> Self {
        let sample_duration = pattern.len() * start_nb_pattern;
        let start: Vec<_> = pattern
            .iter()
            .cycle()
            .take(sample_duration)
            .enumerate()
            .map(|(i, v)| v * i as f32 / sample_duration as f32)
            .collect();

        let sample_duration = pattern.len() * stop_nb_pattern;
        let stop: Vec<_> = pattern
            .iter()
            .cycle()
            .take(sample_duration)
            .enumerate()
            .map(|(i, v)| v * ((sample_duration - i - 1) as f32) / (sample_duration as f32))
            .collect();

        let maintain = pattern.to_vec();

        Key {
            start,
            maintain,
            stop,
            frame_size: pattern.len(),
        }
    }

    pub fn from_pattern_timed(
        pattern: &[f32],
        sample_rate: usize,
        start_duration: f32,
        stop_duration: f32,
    ) -> Self {
        let nb_start_cycle = ((start_duration * (sample_rate as f32)) as usize) / pattern.len();
        let nb_stop_cycle = ((stop_duration * (sample_rate as f32)) as usize) / pattern.len();
        Self::from_pattern(pattern, nb_start_cycle, nb_stop_cycle)
    }

    pub fn to_sound(&self) -> Vec<f32> {
        [&self.start[..], &self.maintain, &self.stop].concat()
    }
}
