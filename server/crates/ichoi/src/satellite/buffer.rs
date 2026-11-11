//! Destination-side PCM cache. Decoder and output have independent cursors.
use super::PlaybackIdentity;
use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};

pub(super) struct Buffer {
    pub state: Mutex<State>,
    pub changed: Condvar,
}
pub(super) struct State {
    chunks: VecDeque<(u64, Vec<u8>)>,
    pub rate: u32,
    pub channels: u16,
    pub end: u64,
    pub cursor: u64,
    pub revision: u64,
    pub paused: bool,
    pub done: bool,
    pub terminal: bool,
    pub error: Option<String>,
    pub identity: PlaybackIdentity,
}
impl Buffer {
    pub fn new(position_ms: u64, identity: PlaybackIdentity) -> Self {
        Self {
            state: Mutex::new(State {
                chunks: VecDeque::new(),
                rate: 0,
                channels: 0,
                end: 0,
                cursor: position_ms,
                revision: 0,
                paused: false,
                done: false,
                terminal: false,
                error: None,
                identity,
            }),
            changed: Condvar::new(),
        }
    }
    pub fn seek(&self, position_ms: u64, identity: PlaybackIdentity) -> bool {
        let mut state = self.state.lock().unwrap();
        if state.rate == 0 || state.error.is_some() || state.terminal {
            return false;
        }
        let frame = position_ms.saturating_mul(u64::from(state.rate)) / 1000;
        if frame < state.start() || frame >= state.end {
            return false;
        }
        state.cursor = frame;
        state.identity = identity;
        state.revision += 1;
        state.paused = false;
        self.changed.notify_all();
        true
    }
    pub fn pause(&self, paused: bool) {
        let mut state = self.state.lock().unwrap();
        state.paused = paused;
        state.revision += 1;
        self.changed.notify_all();
    }
}
impl State {
    pub fn configure(&mut self, rate: u32, channels: u16) {
        if self.rate == 0 {
            self.cursor = self.cursor.saturating_mul(u64::from(rate)) / 1000;
            self.rate = rate;
            self.channels = channels;
        }
    }
    pub fn start(&self) -> u64 {
        self.chunks.front().map_or(self.end, |c| c.0)
    }
    fn capacity(&self) -> u64 {
        // Ten minutes for normal stereo, with a 512 MiB safety ceiling for extreme formats.
        (u64::from(self.rate) * 600).min(512 * 1024 * 1024 / (u64::from(self.channels.max(1)) * 2))
    }
    pub fn push(&mut self, bytes: &[u8]) -> bool {
        let frames = bytes.len() as u64 / (u64::from(self.channels.max(1)) * 2);
        while self.end + frames - self.start() > self.capacity() {
            let Some((start, chunk)) = self.chunks.front() else {
                break;
            };
            let end = start + chunk.len() as u64 / (u64::from(self.channels) * 2);
            if end > self.cursor {
                return false;
            }
            self.chunks.pop_front();
        }
        self.chunks.push_back((self.end, bytes.to_vec()));
        self.end += frames;
        true
    }
    pub fn read(&mut self) -> Option<Vec<u8>> {
        if self.paused {
            return None;
        }
        let frame_bytes = usize::from(self.channels) * 2;
        for (start, bytes) in &self.chunks {
            let end = start + (bytes.len() / frame_bytes) as u64;
            if self.cursor >= *start && self.cursor < end {
                let offset = (self.cursor - start) as usize * frame_bytes;
                let len =
                    (bytes.len() - offset).min((self.rate as usize / 50).max(1) * frame_bytes);
                self.cursor += (len / frame_bytes) as u64;
                return Some(bytes[offset..offset + len].to_vec());
            }
        }
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn identity(id: &str) -> PlaybackIdentity {
        PlaybackIdentity {
            queue_item_id: 1,
            playback_id: id.into(),
        }
    }
    #[test]
    fn ten_minutes_can_seek_without_fetch_or_decode_and_keep_new_identity() {
        let buffer = Buffer::new(0, identity("old"));
        {
            let mut s = buffer.state.lock().unwrap();
            s.configure(100, 1);
            assert!(s.push(&vec![7; 100 * 2 * 600]));
            assert!(!s.push(&[0; 200]));
        }
        assert!(buffer.seek(599_000, identity("new")));
        let mut s = buffer.state.lock().unwrap();
        assert_eq!(s.identity.playback_id, "new");
        assert_eq!(s.read().unwrap(), vec![7; 4]);
        drop(s);
        assert!(!buffer.seek(601_000, identity("miss")));
    }
    #[test]
    fn pause_retains_audio_and_eviction_cannot_remove_unplayed_samples() {
        let buffer = Buffer::new(0, identity("one"));
        let mut s = buffer.state.lock().unwrap();
        s.configure(1, 1);
        for _ in 0..600 {
            assert!(s.push(&[1, 2]));
        }
        assert!(!s.push(&[3, 4]));
        assert_eq!(s.read(), Some(vec![1, 2]));
        assert!(s.push(&[3, 4]));
        assert_eq!(s.start(), 1);
        drop(s);
        buffer.pause(true);
        assert!(buffer.state.lock().unwrap().read().is_none());
        buffer.pause(false);
        assert!(buffer.state.lock().unwrap().read().is_some());
    }
}
