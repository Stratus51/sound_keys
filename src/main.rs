extern crate input_linux;
extern crate tokio;

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

fn watch_device(device_path: &str, mut output: tokio::sync::mpsc::Sender<KeyEvent>) {
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
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
        }
    });
}

async fn handle_events(mut input: tokio::sync::mpsc::Receiver<KeyEvent>) {
    while let Some(event) = input.recv().await {
        println!("Received event {:?}", event);
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
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(paths.len() * 50);
    for path in paths {
        watch_device(&path, event_tx.clone());
    }

    // Process events
    handle_events(event_rx).await
}
