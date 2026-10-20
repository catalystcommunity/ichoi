//! PCM format conversion for native outputs. Windows shared-mode WASAPI accepts only the
//! device mix format, so a 44.1 kHz stereo track must be resampled (and sometimes remapped to
//! more channels, e.g. HDMI) before it reaches the device.

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

/// Frames per resampler chunk. Small enough to keep latency low, large enough for FFT quality.
const CHUNK_FRAMES: usize = 1024;

/// Converts interleaved signed 16-bit PCM from a track format to an output device format.
pub struct FormatConverter {
    in_channels: usize,
    out_channels: usize,
    resampler: Option<Fft<f32>>,
    /// Interleaved input (at `in_channels`) waiting for a full resampler chunk.
    pending: Vec<f32>,
    /// Resampled frames still to discard so the resampler's startup delay is not heard.
    delay_frames: usize,
    input_frames: u64,
    output_frames: u64,
    resampled: Vec<f32>,
}

impl FormatConverter {
    pub fn new(
        in_rate: u32,
        in_channels: u16,
        out_rate: u32,
        out_channels: u16,
    ) -> anyhow::Result<FormatConverter> {
        let in_channels = usize::from(in_channels.max(1));
        let resampler = if in_rate == out_rate {
            None
        } else {
            Some(Fft::<f32>::new(
                in_rate as usize,
                out_rate as usize,
                CHUNK_FRAMES,
                in_channels,
                FixedSync::Input,
            )?)
        };
        let delay_frames = resampler.as_ref().map_or(0, |r| r.output_delay());
        Ok(FormatConverter {
            in_channels,
            out_channels: usize::from(out_channels.max(1)),
            resampler,
            pending: Vec::new(),
            delay_frames,
            input_frames: 0,
            output_frames: 0,
            resampled: Vec::new(),
        })
    }

    /// Convert `input`. Output may be shorter or empty while the resampler fills a chunk.
    pub fn process(&mut self, input: &[i16]) -> anyhow::Result<Vec<i16>> {
        self.pending
            .extend(input.iter().map(|&s| f32::from(s) / 32768.0));
        self.input_frames += (input.len() / self.in_channels) as u64;
        let Some(chunk_frames) = self.resampler.as_ref().map(|r| r.input_frames_next()) else {
            let out = remap(&self.pending, self.in_channels, self.out_channels);
            self.pending.clear();
            return Ok(out);
        };
        let mut out = Vec::new();
        // The input side is fixed, so every chunk needs the same number of frames.
        while self.pending.len() / self.in_channels >= chunk_frames {
            self.resample_chunk(usize::MAX, &mut out)?;
        }
        Ok(out)
    }

    /// Convert the input still buffered at the end of a track, including the frames held in
    /// the resampler's delay line.
    pub fn finish(&mut self) -> anyhow::Result<Vec<i16>> {
        let mut out = Vec::new();
        let Some(resampler) = &self.resampler else {
            return Ok(out);
        };
        let target = (self.input_frames as f64 * resampler.resample_ratio()).round() as u64;
        while self.output_frames < target {
            let remaining = (target - self.output_frames) as usize;
            if self.resample_chunk(remaining, &mut out)? == 0 {
                break;
            }
        }
        self.pending.clear();
        Ok(out)
    }

    /// Resample one chunk and emit at most `limit` frames. A short final chunk is padded with
    /// silence. Returns the number of frames the resampler produced.
    fn resample_chunk(&mut self, limit: usize, out: &mut Vec<i16>) -> anyhow::Result<usize> {
        let resampler = self.resampler.as_mut().expect("resampler is present");
        let ch = self.in_channels;
        let in_frames = resampler.input_frames_next();
        let out_frames = resampler.output_frames_next();
        let available = (self.pending.len() / ch).min(in_frames);
        self.pending
            .resize(self.pending.len().max(in_frames * ch), 0.0);
        self.resampled.clear();
        self.resampled.resize(out_frames * ch, 0.0);
        let input = InterleavedSlice::new(&self.pending[..in_frames * ch], ch, in_frames)?;
        let mut output = InterleavedSlice::new_mut(&mut self.resampled, ch, out_frames)?;
        let indexing = (available < in_frames).then(|| Indexing::new().partial_len(available));
        let (_, written) = resampler.process_into_buffer(&input, &mut output, indexing.as_ref())?;
        self.pending.drain(..available * ch);
        if available < in_frames {
            // Remove the silence padding added above.
            self.pending.clear();
        }
        let skip = self.delay_frames.min(written);
        self.delay_frames -= skip;
        let emitted = (written - skip).min(limit);
        out.extend(remap(
            &self.resampled[skip * ch..(skip + emitted) * ch],
            ch,
            self.out_channels,
        ));
        self.output_frames += emitted as u64;
        Ok(written)
    }
}

/// Map interleaved frames between channel counts. Mono feeds front left and right, a mono
/// output receives the average of left and right, and extra output channels stay silent.
fn remap(input: &[f32], in_channels: usize, out_channels: usize) -> Vec<i16> {
    let mut out = Vec::with_capacity(input.len() / in_channels * out_channels);
    for frame in input.chunks_exact(in_channels) {
        for channel in 0..out_channels {
            let sample = if in_channels == 1 {
                if channel < 2 {
                    frame[0]
                } else {
                    0.0
                }
            } else if out_channels == 1 {
                (frame[0] + frame[1]) / 2.0
            } else if channel < in_channels {
                frame[channel]
            } else {
                0.0
            };
            out.push((sample * 32768.0).round().clamp(-32768.0, 32767.0) as i16);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, channels: u16, frames: usize) -> Vec<i16> {
        (0..frames)
            .flat_map(|i| {
                let t = i as f64 / f64::from(rate);
                let s = ((t * 440.0 * std::f64::consts::TAU).sin() * 16000.0) as i16;
                std::iter::repeat_n(s, usize::from(channels))
            })
            .collect()
    }

    #[test]
    fn same_format_passes_samples_through() {
        let mut converter = FormatConverter::new(48_000, 2, 48_000, 2).unwrap();
        let input = sine(48_000, 2, 100);
        assert_eq!(converter.process(&input).unwrap(), input);
        assert!(converter.finish().unwrap().is_empty());
    }

    #[test]
    fn stereo_is_placed_in_the_front_channels_of_a_wider_output() {
        let mut converter = FormatConverter::new(48_000, 2, 48_000, 8).unwrap();
        let out = converter.process(&[100, -100, 200, -200]).unwrap();
        assert_eq!(out.len(), 16);
        assert_eq!(&out[..8], &[100, -100, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&out[8..], &[200, -200, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn mono_feeds_both_front_channels() {
        let mut converter = FormatConverter::new(48_000, 1, 48_000, 2).unwrap();
        assert_eq!(
            converter.process(&[300, -300]).unwrap(),
            [300, 300, -300, -300]
        );
    }

    #[test]
    fn resampling_keeps_duration_and_level() {
        let seconds = 2;
        let mut converter = FormatConverter::new(44_100, 2, 48_000, 2).unwrap();
        let input = sine(44_100, 2, 44_100 * seconds);
        let mut out = Vec::new();
        // Feed decoder-sized packets that do not line up with the resampler chunk size.
        for packet in input.chunks(4608 * 2) {
            out.extend(converter.process(packet).unwrap());
        }
        out.extend(converter.finish().unwrap());
        let frames = out.len() / 2;
        let expected = 48_000 * seconds;
        assert!(
            frames.abs_diff(expected) <= 2,
            "expected about {expected} frames, got {frames}"
        );
        let peak = out.iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert!((15_000..=17_000).contains(&peak), "peak {peak}");
        // No startup delay: the first frames follow the sine from zero, not silence then a jump.
        assert!(out[..20].iter().any(|&s| s != 0));
    }
}
