use std::time::Duration;
use tokio::sync::mpsc;

pub struct Source {
    frame_input: mpsc::Receiver<Vec<f32>>,
    frame_done: mpsc::Sender<()>,

    sample_rate: u32,
    default_frame: Vec<f32>,
    current_frame: Vec<f32>,
    frame_i: usize,
}

impl Source {
    pub fn new(
        frame_input: mpsc::Receiver<Vec<f32>>,
        frame_done: mpsc::Sender<()>,
        sample_rate: u32,
        default_frame: Vec<f32>,
    ) -> Self {
        Self {
            frame_input,
            frame_done,

            sample_rate,
            current_frame: default_frame.clone(),
            default_frame,
            frame_i: 0,
        }
    }
}

impl Iterator for Source {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.frame_i == self.current_frame.len() {
            // Ignore errors. We don't care if the queue is full.
            drop(self.frame_done.try_send(()));
            self.current_frame = match self.frame_input.try_recv() {
                Ok(frame) => frame,
                Err(_) => self.default_frame.clone(),
            };
            self.frame_i = 0;
        }
        let val = self.current_frame[self.frame_i];
        self.frame_i += 1;
        Some(val)
    }
}

impl rodio::Source for Source {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn channels(&self) -> u16 {
        1
    }
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
