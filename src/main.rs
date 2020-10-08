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

fn watch_device(
    device_path: &str,
    mut output: tokio::sync::mpsc::Sender<input_linux::sys::input_event>,
) {
    let device_path = device_path.to_string();
    std::thread::spawn(move || {
        println!("Opening file {}", device_path);
        let fd =
            std::fs::File::open(device_path).expect("Designated paths should be valid devices");
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
                while let Err(e) = output.try_send(*ev) {
                    eprintln!("Error sending event to main thread: {:?}", e);
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    });
}

async fn handle_events(mut input: tokio::sync::mpsc::Receiver<input_linux::sys::input_event>) {
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
