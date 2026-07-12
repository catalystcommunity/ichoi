//! Direct-mode demuxing (§5.1): Symphonia reads the container and yields raw codec packets
//! plus a stream header. Pre-alpha: probe + a packet-length walk are implemented; wiring the
//! packet push loop into the transport is TODO (§16).

use std::path::Path;

use symphonia::core::codecs::CodecType;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Technical stream facts extracted without decoding (mirrors CSIL `MediaHeader`).
#[derive(Debug, Clone)]
pub struct ProbeInfo {
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: u64,
    pub codec_config: Option<Vec<u8>>,
}

/// Probe a file: identify the codec, sample rate, channels, duration, and decoder init data.
pub fn probe(path: &Path) -> anyhow::Result<ProbeInfo> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let track = probed
        .format
        .default_track()
        .ok_or_else(|| anyhow::anyhow!("no default track"))?;
    let params = &track.codec_params;

    let sample_rate = params.sample_rate.unwrap_or(0);
    let channels = params.channels.map(|c| c.count() as u16).unwrap_or(0);
    let duration_ms = match (params.n_frames, params.sample_rate) {
        (Some(n), Some(sr)) if sr > 0 => (u128::from(n) * 1000 / u128::from(sr)) as u64,
        _ => 0,
    };

    Ok(ProbeInfo {
        codec: codec_name(params.codec),
        sample_rate,
        channels,
        duration_ms,
        codec_config: params.extra_data.as_ref().map(|d| d.to_vec()),
    })
}

fn codec_name(codec: CodecType) -> String {
    use symphonia::core::codecs::{
        CODEC_TYPE_AAC, CODEC_TYPE_ALAC, CODEC_TYPE_FLAC, CODEC_TYPE_MP3, CODEC_TYPE_VORBIS,
    };
    match codec {
        CODEC_TYPE_MP3 => "mp3",
        CODEC_TYPE_AAC => "aac",
        CODEC_TYPE_VORBIS => "vorbis",
        CODEC_TYPE_FLAC => "flac",
        CODEC_TYPE_ALAC => "alac",
        _ => "unknown",
    }
    .to_string()
}
