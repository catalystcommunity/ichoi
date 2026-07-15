//! Outbound satellite runtime. Satellites never listen and never use HTTP; they connect to
//! the core CSIL/TCP listener and exchange length-prefixed CSIL-Events envelopes.

use std::collections::HashMap;
use std::io::{Read, Seek};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ciborium::value::Value;
use libichoi::csil::codec::{
    decode_register_node_response, encode_node_report, encode_register_node_request,
};
use libichoi::csil::types::{
    AudioOutput as WireAudioOutput, Codec, MediaControl, MediaEndReason, MediaEvent, MediaOpen,
    NodeDirective, NodeReport, PlayerStatus, RegisterNodeRequest, StreamPref, TranscodeCodec,
};
use libichoi::csil_channel::{decode_media_event, decode_node_directive, encode_media_control};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::transport::{decode_event_envelope, encode_event_envelope, EventEnvelope};

pub async fn run(config: Config) -> anyhow::Result<()> {
    let core = config
        .core_addr
        .clone()
        .ok_or_else(|| anyhow::anyhow!("satellite role requires ICHOI_CORE_ADDR"))?;
    let node_token = config
        .node_token
        .clone()
        .ok_or_else(|| anyhow::anyhow!("satellite role requires ICHOI_NODE_TOKEN"))?;

    loop {
        match run_once(&core, &node_token).await {
            Ok(()) => log::warn!("satellite connection closed; reconnecting"),
            Err(e) => log::warn!("satellite connection failed: {e}; reconnecting"),
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

async fn run_once(core_addr: &str, node_token: &str) -> anyhow::Result<()> {
    let stream = TcpStream::connect(core_addr).await?;
    let (mut read_half, mut write_half) = stream.into_split();

    write_frame(&mut write_half, &hello_frame(node_token)).await?;
    let first = read_frame(&mut read_half)
        .await?
        .ok_or_else(|| anyhow::anyhow!("core closed before hello ack"))?;
    let ack = decode_event_envelope(&first)?;
    if ack.event != "$hello-ack" {
        anyhow::bail!("core did not acknowledge node hello");
    }

    let outputs = crate::audio::enumerate()
        .into_iter()
        .map(|o| WireAudioOutput {
            os_device_id: o.os_device_id,
            friendly_name: Some(o.friendly_name),
            channels: u64::from(o.channels),
            sample_rates: o.sample_rates.into_iter().map(u64::from).collect(),
            is_default: o.is_default,
        })
        .collect();
    let req = RegisterNodeRequest {
        hostname: crate::app::hostname(),
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        outputs,
    };
    write_frame(
        &mut write_half,
        &event_frame(
            Some("node"),
            "register",
            Some(1),
            encode_register_node_request(&req),
        ),
    )
    .await?;

    let reply = read_frame(&mut read_half)
        .await?
        .ok_or_else(|| anyhow::anyhow!("core closed before register response"))?;
    let env = decode_event_envelope(&reply)?;
    if env.id != Some(1) {
        anyhow::bail!("unexpected response before register completion");
    }
    let registered = decode_register_node_response(&env.payload)?;
    log::info!(
        "registered satellite {} with {} player(s)",
        registered.node_id,
        registered.players.len()
    );

    let (in_tx, mut in_rx) = mpsc::unbounded_channel::<anyhow::Result<Vec<u8>>>();
    tokio::spawn(async move {
        loop {
            match read_frame(&mut read_half).await {
                Ok(Some(frame)) => {
                    if in_tx.send(Ok(frame)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    let _ = in_tx.send(Err(e));
                    break;
                }
            }
        }
    });

    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let mut states = HashMap::new();
    let mut playback = HashMap::<String, PlaybackTask>::new();
    let mut media = ActiveMedia::default();
    for player in &registered.players {
        states.insert(player.id.clone(), PlayerStatus::Stopped);
        out_tx.send(report_frame(&player.id, PlayerStatus::Stopped, None))?;
    }

    loop {
        tokio::select! {
            inbound = in_rx.recv() => {
                let Some(frame) = inbound else { break };
                let frame = frame?;
                let env = decode_event_envelope(&frame)?;
                let service = env.service.as_deref().unwrap_or("").strip_suffix("Service").unwrap_or(env.service.as_deref().unwrap_or("")).to_ascii_lowercase();
                match (service.as_str(), env.event.as_str()) {
                    ("node", "session") => {
                        let directive = decode_node_directive(&env.payload)?;
                        apply_directive(&out_tx, &mut states, &mut playback, &mut media, directive).await?;
                    }
                    ("media", "stream") => {
                        handle_media_event(&out_tx, &mut playback, &mut media, decode_media_event(&env.payload)?).await?;
                    }
                    _ => {}
                }
            }
            Some(frame) = out_rx.recv() => {
                write_frame(&mut write_half, &frame).await?;
            }
        }
    }

    Ok(())
}

fn hello_frame(node_token: &str) -> Vec<u8> {
    let mut payload = Vec::new();
    let _ = ciborium::into_writer(
        &Value::Map(vec![(
            Value::Text("node_token".to_string()),
            Value::Text(node_token.to_string()),
        )]),
        &mut payload,
    );
    event_frame(None, "$hello", None, payload)
}

fn event_frame(service: Option<&str>, event: &str, id: Option<u64>, payload: Vec<u8>) -> Vec<u8> {
    encode_event_envelope(&EventEnvelope {
        service: service.map(str::to_string),
        event: event.to_string(),
        id,
        payload,
    })
}

fn report_frame(player_id: &str, status: PlayerStatus, position_ms: Option<u64>) -> Vec<u8> {
    let report = NodeReport {
        player_id: player_id.to_string(),
        status,
        position_ms,
    };
    event_frame(Some("node"), "session", None, encode_node_report(&report))
}

async fn read_frame(read: &mut tokio::net::tcp::OwnedReadHalf) -> anyhow::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match read.read_exact(&mut len).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let len = u32::from_be_bytes(len) as usize;
    if len > 16 * 1024 * 1024 {
        anyhow::bail!("CSIL frame too large: {len}");
    }
    let mut buf = vec![0u8; len];
    read.read_exact(&mut buf).await?;
    Ok(Some(buf))
}

async fn write_frame(
    write: &mut tokio::net::tcp::OwnedWriteHalf,
    frame: &[u8],
) -> anyhow::Result<()> {
    let len = u32::try_from(frame.len())?;
    write.write_all(&len.to_be_bytes()).await?;
    write.write_all(frame).await?;
    Ok(())
}

#[derive(Default)]
struct ActiveMedia {
    player_id: Option<String>,
    position_ms: u64,
}

async fn apply_directive(
    out_tx: &mpsc::UnboundedSender<Vec<u8>>,
    states: &mut HashMap<String, PlayerStatus>,
    playback: &mut HashMap<String, PlaybackTask>,
    media: &mut ActiveMedia,
    directive: NodeDirective,
) -> anyhow::Result<()> {
    match directive {
        NodeDirective::Variant0(load) => {
            log::info!(
                "satellite load {} on {} at {:?}ms",
                load.track_id,
                load.player_id,
                load.position_ms
            );
            stop_player(playback, &load.player_id);
            let player_id = load.player_id.clone();
            let position_ms = load.position_ms.unwrap_or(0);
            media.player_id = Some(player_id.clone());
            media.position_ms = position_ms;
            let open = MediaControl::Variant0(MediaOpen {
                kind: "open".to_string(),
                track_id: load.track_id,
                pref: StreamPref {
                    max_bitrate_kbps: load.pref.max_bitrate_kbps,
                    prefer_original: Some(false),
                    transcode_codec: Some(TranscodeCodec::Aac),
                },
            });
            out_tx.send(event_frame(
                Some("media"),
                "stream",
                None,
                encode_media_control(&open),
            ))?;
            states.insert(player_id.clone(), PlayerStatus::Playing);
            out_tx.send(report_frame(
                &player_id,
                PlayerStatus::Playing,
                Some(position_ms),
            ))?;
        }
        NodeDirective::Variant1(pause) => {
            stop_player(playback, &pause.player_id);
            states.insert(pause.player_id.clone(), PlayerStatus::Paused);
            out_tx.send(report_frame(&pause.player_id, PlayerStatus::Paused, None))?;
        }
        NodeDirective::Variant2(resume) => {
            states.insert(resume.player_id.clone(), PlayerStatus::Playing);
            out_tx.send(report_frame(&resume.player_id, PlayerStatus::Playing, None))?;
        }
        NodeDirective::Variant3(stop) => {
            stop_player(playback, &stop.player_id);
            if media.player_id.as_deref() == Some(&stop.player_id) {
                media.player_id = None;
            }
            states.insert(stop.player_id.clone(), PlayerStatus::Stopped);
            out_tx.send(report_frame(
                &stop.player_id,
                PlayerStatus::Stopped,
                Some(0),
            ))?;
        }
        NodeDirective::Variant4(vol) => {
            let status = states
                .get(&vol.player_id)
                .cloned()
                .unwrap_or(PlayerStatus::Stopped);
            log::info!("satellite volume {} on {}", vol.volume, vol.player_id);
            out_tx.send(report_frame(&vol.player_id, status, None))?;
        }
    }
    Ok(())
}

async fn handle_media_event(
    out_tx: &mpsc::UnboundedSender<Vec<u8>>,
    playback: &mut HashMap<String, PlaybackTask>,
    media: &mut ActiveMedia,
    event: MediaEvent,
) -> anyhow::Result<()> {
    match event {
        MediaEvent::Variant0(header) => {
            if let Some(player_id) = media.player_id.clone() {
                stop_player(playback, &player_id);
                playback.insert(
                    player_id.clone(),
                    PlaybackTask::start_streaming(
                        header.codec,
                        media.position_ms,
                        player_id,
                        out_tx.clone(),
                    ),
                );
            }
        }
        MediaEvent::Variant1(chunk) => {
            if let Some(player_id) = media.player_id.as_deref() {
                if let Some(task) = playback.get(player_id) {
                    task.push(chunk.data).await?;
                }
            }
        }
        MediaEvent::Variant2(end) => {
            if !matches!(end.reason, Some(MediaEndReason::Stopped)) {
                if let Some(player_id) = media.player_id.as_deref() {
                    if let Some(task) = playback.get(player_id) {
                        task.finish().await;
                    }
                }
            }
        }
        MediaEvent::Variant3(fail) => {
            log::warn!("satellite media stream failed: {}", fail.error.message);
            if let Some(player_id) = media.player_id.take() {
                stop_player(playback, &player_id);
                out_tx.send(report_frame(&player_id, PlayerStatus::Stopped, Some(0)))?;
            }
        }
    }
    Ok(())
}

struct PlaybackTask {
    cancel: Arc<AtomicBool>,
    chunks: mpsc::Sender<Option<Vec<u8>>>,
    handle: JoinHandle<()>,
}

impl PlaybackTask {
    fn start_streaming(
        codec: Codec,
        seek_ms: u64,
        player_id: String,
        out_tx: mpsc::UnboundedSender<Vec<u8>>,
    ) -> PlaybackTask {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel2 = cancel.clone();
        let (chunks, rx) = mpsc::channel::<Option<Vec<u8>>>(32);
        let handle = tokio::task::spawn_blocking(move || {
            let result = decode_stream_to_default_output(
                StreamingSource::new(rx),
                codec_extension(&codec),
                seek_ms,
                cancel2.clone(),
                &player_id,
                out_tx.clone(),
            );
            if let Err(e) = result {
                log::warn!("satellite playback failed: {e}");
            }
            if !cancel2.load(Ordering::Relaxed) {
                let _ = out_tx.send(report_frame(&player_id, PlayerStatus::Stopped, Some(0)));
            }
        });
        PlaybackTask {
            cancel,
            chunks,
            handle,
        }
    }

    async fn push(&self, chunk: Vec<u8>) -> anyhow::Result<()> {
        self.chunks
            .send(Some(chunk))
            .await
            .map_err(|_| anyhow::anyhow!("playback decoder is not accepting media chunks"))
    }

    async fn finish(&self) {
        let _ = self.chunks.send(None).await;
    }
}

impl Drop for PlaybackTask {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.handle.abort();
    }
}

fn stop_player(playback: &mut HashMap<String, PlaybackTask>, player_id: &str) {
    if let Some(task) = playback.remove(player_id) {
        task.cancel.store(true, Ordering::Relaxed);
    }
}

struct StreamingSource {
    rx: Mutex<mpsc::Receiver<Option<Vec<u8>>>>,
    buf: Vec<u8>,
    pos: usize,
    done: bool,
}

impl StreamingSource {
    fn new(rx: mpsc::Receiver<Option<Vec<u8>>>) -> StreamingSource {
        StreamingSource {
            rx: Mutex::new(rx),
            buf: Vec::new(),
            pos: 0,
            done: false,
        }
    }
}

impl Read for StreamingSource {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        while self.pos >= self.buf.len() && !self.done {
            match self.rx.lock().unwrap().blocking_recv() {
                Some(Some(chunk)) => {
                    self.buf = chunk;
                    self.pos = 0;
                }
                Some(None) | None => self.done = true,
            }
        }
        if self.done && self.pos >= self.buf.len() {
            return Ok(0);
        }
        let n = (self.buf.len() - self.pos).min(out.len());
        out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

impl Seek for StreamingSource {
    fn seek(&mut self, _pos: std::io::SeekFrom) -> std::io::Result<u64> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "satellite media stream is not seekable",
        ))
    }
}

impl symphonia::core::io::MediaSource for StreamingSource {
    fn is_seekable(&self) -> bool {
        false
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}

fn decode_stream_to_default_output(
    source: StreamingSource,
    extension: &'static str,
    seek_ms: u64,
    cancel: Arc<AtomicBool>,
    player_id: &str,
    out_tx: mpsc::UnboundedSender<Vec<u8>>,
) -> anyhow::Result<()> {
    use symphonia::core::audio::RawSampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::errors::Error as SymphoniaError;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    if cancel.load(Ordering::Relaxed) {
        return Ok(());
    }

    let mss = MediaSourceStream::new(Box::new(source), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();
    hint.with_extension(extension);
    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow::anyhow!("no supported audio track"))?;
    let track_id = track.id;
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut sink: Option<crate::audio::PcmSink> = None;
    let mut samples: Option<RawSampleBuffer<i16>> = None;
    let mut skip_frames: u64 = 0;
    let mut played_frames: u64 = 0;
    let mut sample_rate: u32 = 0;
    let mut last_report_ms = seek_ms;

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(e)) => {
                log::debug!("decode packet skipped: {e}");
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        let spec = *decoded.spec();
        if sample_rate == 0 {
            sample_rate = spec.rate;
            skip_frames = seek_ms.saturating_mul(u64::from(sample_rate)) / 1000;
        }
        let channels = spec.channels.count().max(1).min(u16::MAX as usize) as u16;
        let sink = sink.get_or_insert(crate::audio::PcmSink::open(spec.rate, channels)?);
        let samples = samples
            .get_or_insert_with(|| RawSampleBuffer::<i16>::new(decoded.capacity() as u64, spec));
        samples.copy_interleaved_ref(decoded);
        let frame_bytes = usize::from(channels) * 2;
        let total_frames = (samples.as_bytes().len() / frame_bytes) as u64;
        let bytes = if skip_frames >= total_frames {
            skip_frames -= total_frames;
            continue;
        } else if skip_frames > 0 {
            let byte_offset = (skip_frames as usize) * frame_bytes;
            skip_frames = 0;
            &samples.as_bytes()[byte_offset..]
        } else {
            samples.as_bytes()
        };
        played_frames += sink.write_s16le(bytes)?;
        if sample_rate > 0 {
            let position_ms =
                seek_ms + (played_frames.saturating_mul(1000) / u64::from(sample_rate));
            if position_ms.saturating_sub(last_report_ms) >= 1000 {
                let _ = out_tx.send(report_frame(
                    player_id,
                    PlayerStatus::Playing,
                    Some(position_ms),
                ));
                last_report_ms = position_ms;
            }
        }
    }
    Ok(())
}

fn codec_extension(codec: &Codec) -> &'static str {
    match codec {
        Codec::Aac => "aac",
        Codec::Mp3 => "mp3",
        Codec::Flac => "flac",
        Codec::Opus => "opus",
        Codec::Vorbis => "ogg",
        Codec::Alac => "m4a",
        Codec::Wav => "wav",
        Codec::Wma => "wma",
    }
}
