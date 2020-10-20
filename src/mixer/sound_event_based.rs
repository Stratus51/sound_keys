pub struct Mixer {
    sound_library: Box<[Box<[f32]>]>,
    frame_size: usize,
    next_frames: Vec<Box<[f32]>>,
}

pub enum SoundEvent {
    Known(usize),
    Unknown(Box<[f32]>),
}

impl Mixer {
    pub fn new(sound_library: Box<[Box<[f32]>]>, frame_size: usize) -> Self {
        Self {
            sound_library,
            frame_size,
            next_frames: vec![],
        }
    }

    pub fn push_event(&mut self, event: SoundEvent) {
        let Self {
            sound_library,
            frame_size,
            next_frames,
        } = self;
        let data = match &event {
            SoundEvent::Known(i) => &sound_library[*i],
            SoundEvent::Unknown(data) => data,
        };

        // TODO Optimize by iterating on frame_i and cursor instead
        for (i, d) in data.iter().enumerate() {
            let frame_i = i / *frame_size;
            let cursor = i % *frame_size;
            if frame_i >= next_frames.len() {
                next_frames.push(vec![0.0; *frame_size].into_boxed_slice());
            }
            next_frames[frame_i][cursor] += d;
        }
    }

    pub fn generate_frame(&mut self) -> Option<Box<[f32]>> {
        if self.next_frames.is_empty() {
            None
        } else {
            Some(self.next_frames.remove(0))
        }
    }
}
