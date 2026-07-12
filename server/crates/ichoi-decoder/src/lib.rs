//! ichoi-decoder — the shared audio decode core.
//!
//! Intended to compile to `wasm32-unknown-unknown` for the browser (§5.5, decode via
//! Symphonia→WASM, fed by postMessage) and to run natively on satellite nodes. It turns
//! the codec packets the server streams — original codec in direct mode, AAC-LC/MP3 in
//! transcoded mode — into interleaved PCM, honoring the gapless trims from `MediaHeader`.
//!
//! Pre-alpha status: this is the interface plus a passthrough skeleton. Wiring Symphonia
//! and the `wasm-bindgen` surface is TODO (§16); the web UI codes against this shape via
//! its TypeScript `IchoiDecoder` interface.

/// Stream header describing the packets that follow (mirrors CSIL `MediaHeader`).
#[derive(Debug, Clone)]
pub struct DecodeHeader {
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub trim_start_samples: u64,
    pub trim_end_samples: u64,
    pub codec_config: Option<Vec<u8>>,
}

/// Turns codec packets into interleaved f32 PCM.
pub trait Decoder {
    fn configure(&mut self, header: &DecodeHeader);
    /// Decode one packet into interleaved PCM (empty if it only primed the decoder).
    fn decode(&mut self, packet: &[u8]) -> Vec<f32>;
}

/// Placeholder decoder. Real Symphonia wiring is TODO (§5.5, §16).
#[derive(Default)]
pub struct PassthroughDecoder {
    header: Option<DecodeHeader>,
}

impl Decoder for PassthroughDecoder {
    fn configure(&mut self, header: &DecodeHeader) {
        self.header = Some(header.clone());
    }

    fn decode(&mut self, _packet: &[u8]) -> Vec<f32> {
        Vec::new()
    }
}
