use std::collections::HashSet;

pub struct Key {
    pub start: Vec<f32>,
    pub maintain: Vec<f32>,
    pub stop: Vec<f32>,
}

pub enum KeyState {
    Starting,
    Maintaining,
    Stopping,
    Stopped,
}

impl KeyState {
    fn next_state(&self) -> Self {
        match self {
            KeyState::Starting => KeyState::Maintaining,
            KeyState::Maintaining => KeyState::Maintaining,
            KeyState::Stopping => KeyState::Stopped,
            KeyState::Stopped => panic!("No state after stopped."),
        }
    }
    fn is_stopped(&self) -> bool {
        if let KeyState::Stopped = self {
            true
        } else {
            false
        }
    }
}

pub struct MixerKey {
    key: Key,
    state: KeyState,
    cursor: usize,
}

impl MixerKey {
    fn from_key(key: Key) -> Self {
        Self {
            key,
            state: KeyState::Stopped,
            cursor: 0,
        }
    }

    fn restart(&mut self) {
        self.state = KeyState::Starting;
        self.cursor = 0;
    }

    fn force_stop(&mut self) {
        self.state = KeyState::Stopped;
    }

    fn stop(&mut self) {
        match self.state {
            KeyState::Stopping | KeyState::Stopped => (),
            _ => {
                self.state = KeyState::Stopping;
                self.cursor = 0;
            }
        }
    }

    fn take_value(&mut self) -> f32 {
        let MixerKey { key, state, cursor } = self;
        let data = match &state {
            KeyState::Starting => &key.start,
            KeyState::Maintaining => &key.maintain,
            KeyState::Stopping => &key.stop,
            KeyState::Stopped => panic!("Key should not be played after being stopped!"),
        };

        let ret = data[*cursor];
        *cursor += 1;
        if *cursor == data.len() {
            *state = state.next_state();
            *cursor = 0;
        }
        ret
    }
}

pub struct Mixer {
    keys: Vec<MixerKey>,
    active_keys: HashSet<usize>,
}

pub enum KeyStateChange {
    Press,
    Release,
    Stop,
}

impl Mixer {
    pub fn new(keys: Vec<Key>) -> Self {
        Self {
            keys: keys.into_iter().map(MixerKey::from_key).collect(),
            active_keys: HashSet::new(),
        }
    }

    pub fn change_key_state(&mut self, n: usize, change: KeyStateChange) {
        let key = &mut self.keys[n];
        match change {
            KeyStateChange::Press => {
                if key.state.is_stopped() {
                    self.active_keys.insert(n);
                }
                key.restart();
            }
            KeyStateChange::Release => {
                key.stop();
            }
            KeyStateChange::Stop => {
                if !key.state.is_stopped() {
                    key.force_stop();
                    self.active_keys.remove(&n);
                }
            }
        }
    }

    pub fn generate_frame(&mut self, size: usize) -> Vec<f32> {
        if self.active_keys.is_empty() {
            return vec![0.0f32; size];
        }
        let mut ret = vec![];
        let mut key_count = self.active_keys.len();
        for _ in 0..size {
            let mut v = 0.0;
            let mut to_remove = vec![];
            for key_i in self.active_keys.iter() {
                let key = &mut self.keys[*key_i];
                v += key.take_value();
                if key.state.is_stopped() {
                    to_remove.push(*key_i);
                }
            }
            if key_count > 0 {
                v /= key_count as f32;
            }
            key_count -= to_remove.len();
            for i in to_remove.iter().rev() {
                self.active_keys.remove(i);
            }
            ret.push(v);
        }
        ret
    }
}
