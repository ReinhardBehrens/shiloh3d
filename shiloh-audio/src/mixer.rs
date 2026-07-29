//! Software mixer skeleton (f32 interleaved).

use parking_lot::Mutex;

use crate::source::{AudioSource, Listener};

pub struct AudioMixer {
    sample_rate: u32,
    sources: Mutex<Vec<AudioSource>>,
    listener: Mutex<Listener>,
}

impl AudioMixer {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            sources: Mutex::new(Vec::new()),
            listener: Mutex::new(Listener::default()),
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn set_listener(&self, listener: Listener) {
        *self.listener.lock() = listener;
    }

    pub fn add_source(&self, source: AudioSource) -> usize {
        let mut sources = self.sources.lock();
        sources.push(source);
        sources.len() - 1
    }

    /// Fills `out` with silence for now (wire sample playback next).
    pub fn mix(&self, out: &mut [f32]) {
        out.fill(0.0);
    }
}
