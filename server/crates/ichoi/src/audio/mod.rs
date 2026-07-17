//! Audio output detection (§6.1). The host audio library is loaded at RUNTIME via `dlopen`,
//! never linked, so the binary starts in a scratch container with no audio stack and simply
//! reports zero outputs.

#[derive(Debug, Clone)]
pub struct AudioOutput {
    pub os_device_id: String,
    pub friendly_name: String,
    pub channels: u16,
    pub sample_rates: Vec<u32>,
    pub is_default: bool,
}

/// Enumerate host audio outputs, or an empty list when none are available.
pub fn enumerate() -> Vec<AudioOutput> {
    #[cfg(target_os = "linux")]
    {
        linux::enumerate()
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        native::enumerate()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Vec::new()
    }
}

/// `"some"` when at least one output is present, else `"none"` (the value reported in node
/// state, §6.1).
pub fn state_label() -> &'static str {
    if enumerate().is_empty() {
        "none"
    } else {
        "some"
    }
}

pub struct PcmSink {
    #[cfg(target_os = "linux")]
    inner: linux::AlsaSink,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    inner: native::NativeSink,
    channels: u16,
}

unsafe impl Send for PcmSink {}

impl PcmSink {
    pub fn open(sample_rate: u32, channels: u16) -> anyhow::Result<PcmSink> {
        #[cfg(target_os = "linux")]
        {
            Ok(PcmSink {
                inner: linux::AlsaSink::open(sample_rate, channels)?,
                channels,
            })
        }
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            Ok(PcmSink {
                inner: native::NativeSink::open(sample_rate, channels)?,
                channels,
            })
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = (sample_rate, channels);
            anyhow::bail!("PCM output is not implemented on this platform")
        }
    }

    pub fn write_s16le(&mut self, bytes: &[u8]) -> anyhow::Result<u64> {
        #[cfg(target_os = "linux")]
        {
            self.inner.write_s16le(bytes, self.channels)
        }
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            self.inner.write_s16le(bytes, self.channels)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = bytes;
            anyhow::bail!("PCM output is not implemented on this platform")
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod native {
    use std::collections::{BTreeSet, VecDeque};
    use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError};

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{SampleFormat, SampleRate, Stream, StreamConfig, SupportedStreamConfig};

    use super::AudioOutput;

    pub fn enumerate() -> Vec<AudioOutput> {
        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            return Vec::new();
        };
        let Ok(name) = device.name() else {
            return Vec::new();
        };
        let Ok(configs) = device.supported_output_configs() else {
            return Vec::new();
        };
        let configs: Vec<_> = configs.collect();
        let channels = configs.iter().map(|c| c.channels()).max().unwrap_or(2);
        let mut rates = BTreeSet::new();
        if let Ok(default) = device.default_output_config() {
            rates.insert(default.sample_rate().0);
        }
        for config in &configs {
            rates.insert(config.min_sample_rate().0);
            rates.insert(config.max_sample_rate().0);
        }
        vec![AudioOutput {
            os_device_id: "default".to_string(),
            friendly_name: name,
            channels,
            sample_rates: rates.into_iter().collect(),
            is_default: true,
        }]
    }

    pub struct NativeSink {
        sender: SyncSender<Vec<i16>>,
        // The stream must stay alive for callbacks to continue.
        _stream: Stream,
    }

    impl NativeSink {
        pub fn open(sample_rate: u32, channels: u16) -> anyhow::Result<Self> {
            let host = cpal::default_host();
            let device = host
                .default_output_device()
                .ok_or_else(|| anyhow::anyhow!("no default audio output"))?;
            let supported = select_config(&device, sample_rate, channels)?;
            let format = supported.sample_format();
            let config = supported.config();
            let (sender, receiver) = sync_channel::<Vec<i16>>(8);
            let err = |e| log::error!("native audio output stream failed: {e}");
            let stream = match format {
                SampleFormat::I16 => build_i16(&device, &config, receiver, err)?,
                SampleFormat::F32 => build_f32(&device, &config, receiver, err)?,
                SampleFormat::U16 => build_u16(&device, &config, receiver, err)?,
                other => anyhow::bail!("unsupported native output sample format {other:?}"),
            };
            stream.play()?;
            Ok(Self {
                sender,
                _stream: stream,
            })
        }

        pub fn write_s16le(&mut self, bytes: &[u8], channels: u16) -> anyhow::Result<u64> {
            let samples: Vec<i16> = bytes
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .collect();
            let frames = samples.len() as u64 / u64::from(channels.max(1));
            self.sender
                .send(samples)
                .map_err(|_| anyhow::anyhow!("native audio output stream closed"))?;
            Ok(frames)
        }
    }

    fn select_config(
        device: &cpal::Device,
        sample_rate: u32,
        channels: u16,
    ) -> anyhow::Result<SupportedStreamConfig> {
        let mut configs = device.supported_output_configs()?;
        configs
            .find(|c| {
                c.channels() == channels
                    && c.min_sample_rate().0 <= sample_rate
                    && c.max_sample_rate().0 >= sample_rate
                    && matches!(
                        c.sample_format(),
                        SampleFormat::I16 | SampleFormat::F32 | SampleFormat::U16
                    )
            })
            .map(|c| c.with_sample_rate(SampleRate(sample_rate)))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "default output does not support {sample_rate} Hz with {channels} channels"
                )
            })
    }

    fn refill(queue: &mut VecDeque<i16>, receiver: &Receiver<Vec<i16>>) {
        if queue.is_empty() {
            match receiver.try_recv() {
                Ok(samples) => queue.extend(samples),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
            }
        }
    }

    fn build_i16<E>(
        device: &cpal::Device,
        config: &StreamConfig,
        receiver: Receiver<Vec<i16>>,
        err: E,
    ) -> Result<Stream, cpal::BuildStreamError>
    where
        E: FnMut(cpal::StreamError) + Send + 'static,
    {
        let mut queue = VecDeque::new();
        device.build_output_stream(
            config,
            move |out: &mut [i16], _| {
                for sample in out {
                    refill(&mut queue, &receiver);
                    *sample = queue.pop_front().unwrap_or(0);
                }
            },
            err,
            None,
        )
    }

    fn build_f32<E>(
        device: &cpal::Device,
        config: &StreamConfig,
        receiver: Receiver<Vec<i16>>,
        err: E,
    ) -> Result<Stream, cpal::BuildStreamError>
    where
        E: FnMut(cpal::StreamError) + Send + 'static,
    {
        let mut queue = VecDeque::new();
        device.build_output_stream(
            config,
            move |out: &mut [f32], _| {
                for sample in out {
                    refill(&mut queue, &receiver);
                    *sample = queue
                        .pop_front()
                        .map(|s| f32::from(s) / 32768.0)
                        .unwrap_or(0.0);
                }
            },
            err,
            None,
        )
    }

    fn build_u16<E>(
        device: &cpal::Device,
        config: &StreamConfig,
        receiver: Receiver<Vec<i16>>,
        err: E,
    ) -> Result<Stream, cpal::BuildStreamError>
    where
        E: FnMut(cpal::StreamError) + Send + 'static,
    {
        let mut queue = VecDeque::new();
        device.build_output_stream(
            config,
            move |out: &mut [u16], _| {
                for sample in out {
                    refill(&mut queue, &receiver);
                    *sample = queue
                        .pop_front()
                        .map(|s| (i32::from(s) + 32768) as u16)
                        .unwrap_or(32768);
                }
            },
            err,
            None,
        )
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int, c_uint, c_ulong, c_void};
    use std::time::Instant;

    use super::AudioOutput;

    type SndPcmT = c_void;
    type SndPcmSFramesT = isize;
    type SndPcmUFramesT = c_ulong;

    const SND_PCM_STREAM_PLAYBACK: c_int = 0;
    const SND_PCM_ACCESS_RW_INTERLEAVED: c_int = 3;
    const SND_PCM_FORMAT_S16_LE: c_int = 2;
    const EPIPE: c_int = 32;

    /// Probe for `libasound.so.2` at runtime. Its presence tells us this host can output
    /// audio; its absence (scratch container) yields zero outputs — no crash, no link-time
    /// dependency. Enumerating individual devices via `snd_device_name_hint` is TODO.
    pub fn enumerate() -> Vec<AudioOutput> {
        // SAFETY: we only load the library and drop it; we call nothing through it yet.
        match unsafe { libloading::Library::new("libasound.so.2") } {
            Ok(_lib) => vec![AudioOutput {
                os_device_id: "default".to_string(),
                friendly_name: "Default (ALSA)".to_string(),
                channels: 2,
                sample_rates: vec![44_100, 48_000],
                is_default: true,
            }],
            Err(_) => Vec::new(),
        }
    }

    pub struct AlsaSink {
        _lib: libloading::Library,
        pcm: *mut SndPcmT,
        writei: unsafe extern "C" fn(*mut SndPcmT, *const c_void, SndPcmUFramesT) -> SndPcmSFramesT,
        prepare: unsafe extern "C" fn(*mut SndPcmT) -> c_int,
        close: unsafe extern "C" fn(*mut SndPcmT) -> c_int,
    }

    unsafe impl Send for AlsaSink {}

    impl AlsaSink {
        pub fn open(sample_rate: u32, channels: u16) -> anyhow::Result<AlsaSink> {
            unsafe {
                let lib = libloading::Library::new("libasound.so.2")?;
                let open = *lib.get::<unsafe extern "C" fn(
                    *mut *mut SndPcmT,
                    *const c_char,
                    c_int,
                    c_int,
                ) -> c_int>(b"snd_pcm_open")?;
                let set_params = *lib.get::<unsafe extern "C" fn(
                    *mut SndPcmT,
                    c_int,
                    c_int,
                    c_uint,
                    c_uint,
                    c_int,
                    c_uint,
                ) -> c_int>(b"snd_pcm_set_params")?;
                let writei = *lib.get::<unsafe extern "C" fn(
                    *mut SndPcmT,
                    *const c_void,
                    SndPcmUFramesT,
                ) -> SndPcmSFramesT>(b"snd_pcm_writei")?;
                let prepare =
                    *lib.get::<unsafe extern "C" fn(*mut SndPcmT) -> c_int>(b"snd_pcm_prepare")?;
                let close =
                    *lib.get::<unsafe extern "C" fn(*mut SndPcmT) -> c_int>(b"snd_pcm_close")?;

                let name = CString::new("default").unwrap();
                let mut pcm: *mut SndPcmT = std::ptr::null_mut();
                let rc = open(&mut pcm, name.as_ptr(), SND_PCM_STREAM_PLAYBACK, 0);
                if rc < 0 {
                    anyhow::bail!("snd_pcm_open(default) failed: {rc}");
                }
                let rc = set_params(
                    pcm,
                    SND_PCM_FORMAT_S16_LE,
                    SND_PCM_ACCESS_RW_INTERLEAVED,
                    channels as c_uint,
                    sample_rate as c_uint,
                    1,
                    500_000,
                );
                if rc < 0 {
                    let _ = close(pcm);
                    anyhow::bail!("snd_pcm_set_params failed: {rc}");
                }

                Ok(AlsaSink {
                    _lib: lib,
                    pcm,
                    writei,
                    prepare,
                    close,
                })
            }
        }

        pub fn write_s16le(&mut self, bytes: &[u8], channels: u16) -> anyhow::Result<u64> {
            let frame_bytes = usize::from(channels) * 2;
            if frame_bytes == 0 {
                return Ok(0);
            }
            let complete = bytes.len() - (bytes.len() % frame_bytes);
            let mut offset = 0usize;
            let mut written = 0u64;
            while offset < complete {
                let frames = ((complete - offset) / frame_bytes).min(4096);
                let start = Instant::now();
                let rc = unsafe {
                    (self.writei)(
                        self.pcm,
                        bytes[offset..].as_ptr().cast::<c_void>(),
                        frames as SndPcmUFramesT,
                    )
                };
                let elapsed = start.elapsed();
                if rc == -(EPIPE as isize) {
                    log::warn!("ALSA playback underrun; preparing stream and continuing");
                    unsafe {
                        (self.prepare)(self.pcm);
                    }
                    continue;
                }
                if rc < 0 {
                    anyhow::bail!("snd_pcm_writei failed: {rc}");
                }
                if elapsed.as_millis() > 500 {
                    log::warn!(
                        "ALSA write stalled for {}ms while writing {} frame(s)",
                        elapsed.as_millis(),
                        rc
                    );
                }
                let actual = rc as usize;
                offset += actual * frame_bytes;
                written += actual as u64;
            }
            Ok(written)
        }
    }

    impl Drop for AlsaSink {
        fn drop(&mut self) {
            unsafe {
                (self.close)(self.pcm);
            }
        }
    }
}
