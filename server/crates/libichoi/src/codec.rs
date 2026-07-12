//! Codec classification driving the media pipeline's direct-vs-transcode decision (§5).

/// Codecs the edge (Symphonia, natively or as WASM) can decode. Opus and HE-AAC are
/// deliberately absent — they are read only via transcode-to-AAC-LC (§5.6).
pub fn is_client_decodable(codec: &str) -> bool {
    matches!(codec, "mp3" | "aac" | "vorbis" | "flac" | "alac" | "wav")
}

/// Decide whether a source must be transcoded for a given consumer.
///
/// A source is transcoded when it is not client-decodable (Opus/HE-AAC), or when the
/// consumer caps bitrate below the source's. `prefer_original` only matters when the
/// source is decodable and within the cap.
pub fn needs_transcode(
    codec: &str,
    prefer_original: bool,
    max_bitrate_kbps: Option<u32>,
    source_bitrate_kbps: Option<u32>,
) -> bool {
    if !is_client_decodable(codec) {
        return true;
    }
    if let (Some(max), Some(br)) = (max_bitrate_kbps, source_bitrate_kbps) {
        if max > 0 && br > max {
            return true;
        }
    }
    let _ = prefer_original;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_always_transcodes() {
        assert!(!is_client_decodable("opus"));
        assert!(needs_transcode("opus", true, None, Some(96)));
    }

    #[test]
    fn flac_is_direct_within_cap() {
        assert!(!needs_transcode("flac", false, None, Some(900)));
        assert!(!needs_transcode("flac", false, Some(0), Some(900)));
        assert!(needs_transcode("flac", false, Some(320), Some(900)));
    }
}
