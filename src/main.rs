extern crate input_linux;
extern crate rodio;
extern crate tokio;
use rodio::Source;
use std::iter::Iterator;
use std::time::Duration;
use tokio::sync::mpsc;

const EMPTY_EVENT: input_linux::sys::input_event = input_linux::sys::input_event {
    time: input_linux::sys::timeval {
        tv_sec: 0,
        tv_usec: 0,
    },
    type_: 0,
    code: 0,
    value: 0,
};

#[derive(Debug, Clone, Copy)]
enum KeyState {
    Up,
    Down,
    Repeat,
    Unknown,
}

impl KeyState {
    fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Up,
            1 => Self::Down,
            2 => Self::Repeat,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct KeyEvent {
    sec: i64,
    usec: i64,
    code: u16,
    value: KeyState,
}

const EV_KEY: u16 = input_linux::sys::EV_KEY as u16;

fn watch_device(device_path: &str, mut output: mpsc::Sender<KeyEvent>) {
    let device_path = device_path.to_string();
    std::thread::spawn(move || {
        println!("Opening file {}", device_path);
        let fd =
            std::fs::File::open(&device_path).expect("Designated paths should be valid devices");
        let input = input_linux::evdev::EvdevHandle::new(fd);
        loop {
            let mut events: [input_linux::sys::input_event; 1] = [EMPTY_EVENT; 1];
            let n = match input.read(&mut events) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("{}", e);
                    return;
                }
            };
            for ev in events.iter().take(n) {
                if ev.type_ == EV_KEY {
                    let ev = KeyEvent {
                        sec: ev.time.tv_sec,
                        usec: ev.time.tv_usec,
                        code: ev.code,
                        value: KeyState::from_i32(ev.value),
                    };
                    while let Err(e) = output.try_send(ev) {
                        eprintln!("Error sending event to main thread: {:?}", e);
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        }
    });
}

const KEY_DURATION: Duration = Duration::from_millis(200);

enum Side {
    Left,
    Right,
    Middle,
}

enum MixerCommand {
    Ting { track_id: usize, side: Side },
}
struct FullMixerCommand {
    cmd: MixerCommand,
}

struct TrackIndex {
    track_id: usize,
    cursor: i32,
}

struct Mixer {
    cmd_input: mpsc::Receiver<FullMixerCommand>,
    sample_rate: u32,
    recalculation_rate: u32,
    sounds: Vec<Vec<f32>>,
    tracks: Vec<TrackIndex>,
    until_recalculation: u32,
}

impl Mixer {
    fn new(
        cmd_input: mpsc::Receiver<FullMixerCommand>,
        sample_rate: u32,
        recalculation_rate: u32,
        sounds: Vec<Vec<f32>>,
    ) -> Self {
        Self {
            cmd_input,
            sample_rate,
            recalculation_rate,
            sounds,
            tracks: vec![],
            until_recalculation: 0,
        }
    }

    fn process_events(&mut self) {
        while let Ok(cmd) = self.cmd_input.try_recv() {
            let FullMixerCommand { cmd } = cmd;
            match cmd {
                MixerCommand::Ting { track_id, side } => self.tracks.push(TrackIndex {
                    track_id,
                    cursor: 0,
                }),
            }
        }
    }
}

impl Iterator for Mixer {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.until_recalculation == 0 {
            self.process_events();
            self.until_recalculation = self.recalculation_rate;
        }
        self.until_recalculation -= 1;

        let mut ret = 0.0;
        let mut to_remove = vec![];
        for (i, track) in self.tracks.iter_mut().enumerate() {
            let sound = &self.sounds[track.track_id];
            if track.cursor > 0 {
                ret += sound[track.cursor as usize];
            }
            track.cursor += 1;
            if track.cursor > 0 && track.cursor as usize == sound.len() {
                to_remove.push(i);
            }
        }
        for i in to_remove.into_iter().rev() {
            self.tracks.remove(i);
        }
        Some(ret)
    }
}

impl Source for Mixer {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn channels(&self) -> u16 {
        2
    }
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

async fn handle_events(mut input: mpsc::Receiver<KeyEvent>) {
    let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
    let silence = rodio::source::Zero::<f32>::new(1, 48 * 1024);

    let (mut mix_tx, mix_rx) = mpsc::channel(50);
    let sample_rate = 48 * 1024;
    let recalculation_rate = 1024;
    let sounds = vec![rodio::source::SineWave::new(440)
        .take_crossfade_with(silence.clone(), KEY_DURATION)
        .amplify(0.2)
        .take(KEY_DURATION.as_millis() as usize * sample_rate / 1000)
        .collect()];
    let mixer = Mixer::new(mix_rx, sample_rate as u32, recalculation_rate, sounds);

    stream_handle
        .play_raw(mixer.convert_samples())
        .expect("play_raw");

    while let Some(event) = input.recv().await {
        println!("Received event {:?}", event);
        if let Err(e) = mix_tx
            .send(FullMixerCommand {
                cmd: MixerCommand::Ting {
                    track_id: 0,
                    side: Side::Middle,
                },
            })
            .await
        {
            eprintln!("Error: {}", e);
        }
    }
}

#[tokio::main]
async fn main() {
    // Arguments parsing
    let paths: Vec<_> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        panic!("No device paths specified");
    }

    // Catch thread panics
    let orig_handler = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |i| {
        orig_handler(i);
        std::process::exit(-1)
    }));

    // Watch devices events
    let (event_tx, event_rx) = mpsc::channel(paths.len() * 50);
    for path in paths {
        watch_device(&path, event_tx.clone());
    }

    // Process events
    handle_events(event_rx).await
}
