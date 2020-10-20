extern crate input_linux;
extern crate rodio;
extern crate tokio;
use rodio::Source;
use std::iter::Iterator;
use std::time::Duration;
use tokio::sync::mpsc;

mod mixer;
mod sound;
mod source;

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

const KEY_DURATION: Duration = Duration::from_millis(100);

#[derive(Debug)]
enum AnyEvent {
    Key(KeyEvent),
    Source,
}

async fn start_key_watcher(
    mut input: mpsc::Receiver<KeyEvent>,
    mut output: mpsc::Sender<AnyEvent>,
) {
    while let Some(ev) = input.recv().await {
        output.send(AnyEvent::Key(ev)).await.unwrap();
    }
}

async fn start_source_watcher(mut input: mpsc::Receiver<()>, mut output: mpsc::Sender<AnyEvent>) {
    while input.recv().await.is_some() {
        output.send(AnyEvent::Source).await.unwrap();
    }
}

const MAX_FREQ: usize = 1000;
const MIN_FREQ: usize = 100;
const NB_FREQ: usize = 200;
async fn handle_events(input: mpsc::Receiver<KeyEvent>) {
    let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();

    let sample_rate = 48 * 1024;
    let frame_size = 1024;
    let key_duration = KEY_DURATION.as_millis() as f32 / 1000.0;
    let freq_mul = f64::exp(f64::ln(MAX_FREQ as f64 / MIN_FREQ as f64) / (NB_FREQ as f64 - 1.0));
    let keys = (0..NB_FREQ)
        .map(|i| {
            let freq = (MIN_FREQ as f64 * freq_mul.powi(i as i32)) as usize;
            let sound: Vec<_> = sound::sinus_sound(freq, sample_rate)
                .into_iter()
                .map(|v| v * 0.03)
                .collect();
            let start_duration = key_duration / 2.0;
            let stop_duration = key_duration;
            sound::key::Key::from_pattern_timed(&sound, sample_rate, start_duration, stop_duration)
        })
        .collect::<Vec<_>>();

    let (mut source_tx, source_rx) = mpsc::channel(10);
    let (frame_done_tx, frame_done_rx) = mpsc::channel(10);
    let source = source::event_based::Source::new(
        source_rx,
        frame_done_tx,
        sample_rate as u32,
        vec![0.0f32; frame_size],
    );

    let (any_tx, mut any_rx) = mpsc::channel(10);

    tokio::spawn(start_key_watcher(input, any_tx.clone()));
    tokio::spawn(start_source_watcher(frame_done_rx, any_tx));

    let mut mixer = mixer::key_state_based::Mixer::new(keys);

    stream_handle
        .play_raw(source.convert_samples())
        .expect("play_raw");

    source_tx.send(vec![0.0; frame_size]).await.unwrap();
    source_tx.send(vec![0.0; frame_size]).await.unwrap();
    while let Some(event) = any_rx.recv().await {
        match event {
            AnyEvent::Key(event) => {
                let key_id = event.code as usize % NB_FREQ;
                match event.value {
                    KeyState::Up => mixer
                        .change_key_state(key_id, mixer::key_state_based::KeyStateChange::Release),
                    KeyState::Repeat => (),
                    KeyState::Down => mixer
                        .change_key_state(key_id, mixer::key_state_based::KeyStateChange::Press),
                    _ => continue,
                };
            }
            AnyEvent::Source => {
                let frame = mixer.generate_frame(frame_size);
                if let Err(e) = source_tx.send(frame).await {
                    eprintln!("Lost a frame: {}", e);
                }
            }
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
