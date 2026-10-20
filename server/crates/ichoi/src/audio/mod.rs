//! Audio output detection (§6.1). The host audio library is loaded at RUNTIME via `dlopen`,
//! never linked, so the binary starts in a scratch container with no audio stack and simply
//! reports zero outputs.

#[cfg(any(target_os = "macos", target_os = "windows", test))]
mod convert;

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

/// Verify that satellite playback can open the selected system output. This check does not
/// install packages or change audio configuration.
pub fn validate_satellite_output() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    {
        linux::validate_satellite_output()
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        if native::enumerate().is_empty() {
            anyhow::bail!("Ichoi did not find a default audio output")
        }
        let _sink = native::NativeSink::open(48_000, 2)?;
        Ok(())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        anyhow::bail!("satellite audio is not supported on this platform")
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

    /// Let queued audio finish playing at the end of a track instead of cutting it off.
    /// Returns early once `cancel` is set.
    pub fn drain(&mut self, cancel: &std::sync::atomic::AtomicBool) -> anyhow::Result<()> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            self.inner.drain(cancel)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = cancel;
            Ok(())
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod native {
    use std::collections::{BTreeSet, VecDeque};
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{
        BufferSize, SampleFormat, SampleRate, Stream, StreamConfig, SupportedBufferSize,
        SupportedStreamConfig,
    };

    use super::convert::FormatConverter;
    use super::AudioOutput;

    /// Upper bound on waiting for queued audio at the end of a track.
    const DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

    /// Device buffer length. The WASAPI default is one engine period (about 10 ms), which a
    /// busy low-power CPU misses often enough to be heard. A satellite does not need low latency.
    const DEVICE_BUFFER_MS: u32 = 200;

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
        /// Chunks sent to the stream callback that it has not finished playing.
        queued: Arc<AtomicUsize>,
        /// Present when the device cannot take the track format directly.
        converter: Option<FormatConverter>,
        stats: Arc<Underruns>,
        reported: bool,
        // The stream must stay alive for callbacks to continue.
        _stream: Stream,
    }

    /// Playback gaps, counted by the stream callback.
    #[derive(Default)]
    struct Underruns {
        /// The device buffer emptied before the callback refilled it.
        device: AtomicU64,
        /// The callback found no decoded audio to play.
        queue: AtomicU64,
    }

    impl NativeSink {
        pub fn open(sample_rate: u32, channels: u16) -> anyhow::Result<Self> {
            let host = cpal::default_host();
            let device = host
                .default_output_device()
                .ok_or_else(|| anyhow::anyhow!("no default audio output"))?;
            let (supported, converter) = match select_config(&device, sample_rate, channels) {
                Ok(supported) => (supported, None),
                Err(unsupported) => {
                    // Shared-mode WASAPI accepts only the device mix format, so convert to it.
                    let mix = device.default_output_config().map_err(|e| {
                        anyhow::anyhow!("{unsupported}, and its mix format is unavailable: {e}")
                    })?;
                    log::info!(
                        "native output converts {sample_rate} Hz/{channels} ch to {} Hz/{} ch ({:?})",
                        mix.sample_rate().0,
                        mix.channels(),
                        mix.sample_format()
                    );
                    let converter = FormatConverter::new(
                        sample_rate,
                        channels,
                        mix.sample_rate().0,
                        mix.channels(),
                    )?;
                    (mix, Some(converter))
                }
            };
            let format = supported.sample_format();
            let mut config = supported.config();
            config.buffer_size = device_buffer_size(&supported);
            log::debug!("native output buffer: {:?}", config.buffer_size);
            let (sender, receiver) = sync_channel::<Vec<i16>>(8);
            let queued = Arc::new(AtomicUsize::new(0));
            let stats = Arc::new(Underruns::default());
            let buffer_samples = match config.buffer_size {
                BufferSize::Fixed(frames) => frames as usize * usize::from(config.channels),
                BufferSize::Default => usize::MAX,
            };
            let feed = Feed {
                queue: VecDeque::new(),
                receiver,
                queued: queued.clone(),
                holding: false,
                stats: stats.clone(),
                buffer_samples,
                started: false,
                boosted: false,
                ran_dry: false,
            };
            let err = |e| log::error!("native audio output stream failed: {e}");
            let stream = match format {
                SampleFormat::I16 => build_i16(&device, &config, feed, err)?,
                SampleFormat::F32 => build_f32(&device, &config, feed, err)?,
                SampleFormat::U16 => build_u16(&device, &config, feed, err)?,
                other => anyhow::bail!("unsupported native output sample format {other:?}"),
            };
            stream.play()?;
            Ok(Self {
                sender,
                queued,
                converter,
                stats,
                reported: false,
                _stream: stream,
            })
        }

        /// Queue interleaved PCM in the track format. Returns the track frames accepted.
        pub fn write_s16le(&mut self, bytes: &[u8], channels: u16) -> anyhow::Result<u64> {
            let samples: Vec<i16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| i16::from_le_bytes(*b))
                .collect();
            let frames = samples.len() as u64 / u64::from(channels.max(1));
            let samples = match &mut self.converter {
                Some(converter) => converter.process(&samples)?,
                None => samples,
            };
            self.send(samples)?;
            Ok(frames)
        }

        pub fn drain(&mut self, cancel: &AtomicBool) -> anyhow::Result<()> {
            // Report before the queue empties on purpose at the end of the track.
            self.report();
            if let Some(converter) = &mut self.converter {
                let tail = converter.finish()?;
                self.send(tail)?;
            }
            let started = Instant::now();
            while self.queued.load(Ordering::Acquire) > 0
                && started.elapsed() < DRAIN_TIMEOUT
                && !cancel.load(Ordering::Relaxed)
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            // The device buffer still holds up to DEVICE_BUFFER_MS of audio.
            let played_out = Instant::now() + Duration::from_millis(u64::from(DEVICE_BUFFER_MS));
            while Instant::now() < played_out && !cancel.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(())
        }

        fn report(&mut self) {
            if self.reported {
                return;
            }
            self.reported = true;
            let device = self.stats.device.load(Ordering::Relaxed);
            let queue = self.stats.queue.load(Ordering::Relaxed);
            if device > 0 || queue > 0 {
                log::warn!(
                    "native output gaps this track: device buffer ran dry {device} times, decoded audio ran out {queue} times"
                );
            }
        }

        fn send(&self, samples: Vec<i16>) -> anyhow::Result<()> {
            if samples.is_empty() {
                return Ok(());
            }
            self.queued.fetch_add(1, Ordering::AcqRel);
            self.sender.send(samples).map_err(|_| {
                self.queued.fetch_sub(1, Ordering::AcqRel);
                anyhow::anyhow!("native audio output stream closed")
            })
        }
    }

    impl Drop for NativeSink {
        fn drop(&mut self) {
            self.report();
        }
    }

    /// Ask for a device buffer of `DEVICE_BUFFER_MS`, within what the device supports.
    fn device_buffer_size(supported: &SupportedStreamConfig) -> BufferSize {
        let target = supported.sample_rate().0 * DEVICE_BUFFER_MS / 1000;
        match supported.buffer_size() {
            SupportedBufferSize::Range { min, max } => {
                BufferSize::Fixed(target.clamp(*min, (*max).max(*min)))
            }
            SupportedBufferSize::Unknown => BufferSize::Default,
        }
    }

    /// The stream callback's side of the sample queue.
    struct Feed {
        queue: VecDeque<i16>,
        receiver: Receiver<Vec<i16>>,
        queued: Arc<AtomicUsize>,
        /// True while `queue` holds samples from a chunk counted in `queued`.
        holding: bool,
        stats: Arc<Underruns>,
        /// Samples in the whole device buffer.
        buffer_samples: usize,
        /// True once the first decoded sample has played.
        started: bool,
        boosted: bool,
        ran_dry: bool,
    }

    impl Feed {
        /// Called at the start of each callback with the number of samples requested.
        fn begin(&mut self, requested: usize) {
            if !self.boosted {
                self.boosted = true;
                boost_audio_thread();
            }
            // A WASAPI callback normally refills one engine period. A request for the whole
            // buffer means the device played it all out first. CoreAudio always asks for the
            // whole buffer, so this check applies only to Windows.
            if cfg!(target_os = "windows") && self.started && requested >= self.buffer_samples {
                self.stats.device.fetch_add(1, Ordering::Relaxed);
            }
            self.ran_dry = false;
        }

        /// Called at the end of each callback.
        fn end(&mut self) {
            if self.ran_dry {
                self.stats.queue.fetch_add(1, Ordering::Relaxed);
            }
        }

        fn next(&mut self) -> Option<i16> {
            if self.queue.is_empty() {
                if self.holding {
                    self.queued.fetch_sub(1, Ordering::AcqRel);
                    self.holding = false;
                }
                match self.receiver.try_recv() {
                    Ok(samples) => {
                        self.queue.extend(samples);
                        self.holding = true;
                    }
                    Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
                }
            }
            let sample = self.queue.pop_front();
            match sample {
                Some(_) => self.started = true,
                None if self.started => self.ran_dry = true,
                None => {}
            }
            sample
        }
    }

    /// Give the stream callback thread real-time audio scheduling. cpal 0.15 tries to raise
    /// its priority but passes a thread id where a handle is required, so the call fails and
    /// the thread stays at normal priority.
    #[cfg(target_os = "windows")]
    fn boost_audio_thread() {
        use std::ffi::c_void;

        #[link(name = "avrt")]
        extern "system" {
            fn AvSetMmThreadCharacteristicsW(task: *const u16, index: *mut u32) -> *mut c_void;
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentThread() -> *mut c_void;
            fn SetThreadPriority(thread: *mut c_void, priority: i32) -> i32;
        }
        const THREAD_PRIORITY_TIME_CRITICAL: i32 = 15;

        let task: Vec<u16> = "Pro Audio\0".encode_utf16().collect();
        let mut index = 0u32;
        // SAFETY: `task` is a NUL-terminated UTF-16 string and `index` is a valid out pointer.
        let mmcss = unsafe { AvSetMmThreadCharacteristicsW(task.as_ptr(), &mut index) };
        if !mmcss.is_null() {
            log::debug!("native output thread registered with MMCSS Pro Audio");
            return;
        }
        // SAFETY: the pseudo-handle from GetCurrentThread is always valid for this thread.
        let raised =
            unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL) } != 0;
        log::warn!(
            "native output thread could not use MMCSS: {}; time-critical priority {}",
            std::io::Error::last_os_error(),
            if raised { "set" } else { "also failed" }
        );
    }

    /// CoreAudio already runs its callbacks on a real-time thread.
    #[cfg(not(target_os = "windows"))]
    fn boost_audio_thread() {}

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

    fn build_i16<E>(
        device: &cpal::Device,
        config: &StreamConfig,
        mut feed: Feed,
        err: E,
    ) -> Result<Stream, cpal::BuildStreamError>
    where
        E: FnMut(cpal::StreamError) + Send + 'static,
    {
        device.build_output_stream(
            config,
            move |out: &mut [i16], _| {
                feed.begin(out.len());
                for sample in out {
                    *sample = feed.next().unwrap_or(0);
                }
                feed.end();
            },
            err,
            None,
        )
    }

    fn build_f32<E>(
        device: &cpal::Device,
        config: &StreamConfig,
        mut feed: Feed,
        err: E,
    ) -> Result<Stream, cpal::BuildStreamError>
    where
        E: FnMut(cpal::StreamError) + Send + 'static,
    {
        device.build_output_stream(
            config,
            move |out: &mut [f32], _| {
                feed.begin(out.len());
                for sample in out {
                    *sample = feed.next().map(|s| f32::from(s) / 32768.0).unwrap_or(0.0);
                }
                feed.end();
            },
            err,
            None,
        )
    }

    fn build_u16<E>(
        device: &cpal::Device,
        config: &StreamConfig,
        mut feed: Feed,
        err: E,
    ) -> Result<Stream, cpal::BuildStreamError>
    where
        E: FnMut(cpal::StreamError) + Send + 'static,
    {
        device.build_output_stream(
            config,
            move |out: &mut [u16], _| {
                feed.begin(out.len());
                for sample in out {
                    *sample = feed
                        .next()
                        .map(|s| (i32::from(s) + 32768) as u16)
                        .unwrap_or(32768);
                }
                feed.end();
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

    pub fn validate_satellite_output() -> anyhow::Result<()> {
        if let Err(error) = unsafe { libloading::Library::new("libasound.so.2") } {
            let detail = error.to_string();
            if detail.contains("Dynamic loading not supported") {
                anyhow::bail!(
                    "this is the static core build and cannot load ALSA; download the GNU satellite release for this architecture"
                );
            }
            anyhow::bail!(
                "Ichoi cannot load libasound.so.2 ({detail}). Install it, then run this command again. {}",
                alsa_install_command()
            );
        }
        AlsaSink::open(48_000, 2).map(|_| ()).map_err(|error| {
            anyhow::anyhow!(
                "Ichoi cannot open the ALSA default output: {error}. Select a working default output, then run this command again"
            )
        })
    }

    fn alsa_install_command() -> &'static str {
        let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
        if release.contains("ID=arch") || release.contains("ID_LIKE=arch") {
            "Run: sudo pacman -S alsa-lib"
        } else if release.contains("ID=fedora") || release.contains("ID_LIKE=\"rhel fedora\"") {
            "Run: sudo dnf install alsa-lib"
        } else if release.contains("ID=opensuse") || release.contains("ID_LIKE=\"suse") {
            "Run: sudo zypper install libasound2"
        } else {
            "For Debian or Ubuntu, run: sudo apt install libasound2"
        }
    }

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
