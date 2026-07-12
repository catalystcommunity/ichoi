//! Audio output detection (§6.1). The host audio library is loaded at RUNTIME via `dlopen`,
//! never linked, so the binary starts in a scratch container with no audio stack and simply
//! reports zero outputs. Full PCM playback through the dlopen'd API is TODO (§16).

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
    #[cfg(not(target_os = "linux"))]
    {
        // TODO: cpal/native backend on macOS/Windows, where the framework always exists.
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

#[cfg(target_os = "linux")]
mod linux {
    use super::AudioOutput;

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
}
