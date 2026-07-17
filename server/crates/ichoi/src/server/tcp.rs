//! Native CSIL/TCP surface. Binary clients use length-prefixed CSIL-Events envelopes.
//! The old line-delimited JSON form remains for shell/debug tooling only.

use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};

use libichoi::csil::codec::decode_node_report;
use libichoi::csil::types::{
    Codec, MediaChunk, MediaEnd, MediaEndReason, MediaEvent, MediaFail, MediaHeader, MediaOpen,
    ServiceError,
};
use libichoi::csil_channel::encode_media_event;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::db::store;
use crate::handlers::{App, Ctx, Identity};
use crate::{media, transport};

static TCP_CONN_ID: AtomicU64 = AtomicU64::new(10_000);

#[derive(Deserialize)]
struct WireEnvelope {
    service: String,
    op: String,
    #[serde(default)]
    id: u64,
    #[serde(default)]
    payload_hex: String,
}

#[derive(Serialize)]
struct WirePush {
    service: &'static str,
    op: &'static str,
    payload_hex: String,
}

pub async fn serve_tcp(app: App, addr: String) -> anyhow::Result<()> {
    let identity = crate::tls::core_identity(&app.config)?;
    log::info!(
        "CSIL/TLS core fingerprint {} (certificate {})",
        identity.fingerprint,
        identity.cert_path.display()
    );
    let acceptor = tokio_rustls::TlsAcceptor::from(identity.server_config);
    let listener = TcpListener::bind(&addr).await?;
    log::info!("CSIL/TLS listening on {addr}");
    loop {
        let (stream, _peer) = listener.accept().await?;
        let app = app.clone();
        let acceptor = acceptor.clone();
        tokio::spawn(async move {
            let result = async {
                let stream = acceptor.accept(stream).await?;
                handle_conn(stream, app).await
            }
            .await;
            if let Err(e) = result {
                log::debug!("TLS connection closed: {e}");
            }
        });
    }
}

async fn handle_conn<S>(stream: S, app: App) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut stream = BufReader::new(stream);
    let first = stream.fill_buf().await?;
    if first.is_empty() {
        return Ok(());
    }
    if first[0] == b'{' {
        handle_json_conn(stream, app).await
    } else {
        handle_binary_conn(stream, app).await
    }
}

async fn handle_binary_conn<S>(stream: S, app: App) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut read_half, mut write_half) = tokio::io::split(stream);
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (node_tx, mut node_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (in_tx, mut in_rx) = mpsc::unbounded_channel::<anyhow::Result<Vec<u8>>>();
    let conn_id = TCP_CONN_ID.fetch_add(1, Ordering::Relaxed);
    let mut ident = Identity::Anonymous;

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

    loop {
        tokio::select! {
            frame = in_rx.recv() => {
                let Some(frame) = frame else { break };
                let frame = frame?;
                let app2 = app.clone();
                let ident_in = ident.clone();
                let (ident_out, reply, effects) =
                    tokio::task::spawn_blocking(move || transport::handle_events_frame(&app2, ident_in, &frame)).await?;
                ident = ident_out;
                if let Some(reply) = reply {
                    write_frame(&mut write_half, &reply).await?;
                }
                if let Some(player_id) = effects.subscribe {
                    app.subs.subscribe(player_id, conn_id, tx.clone());
                }
                if let Some(player_id) = effects.attach {
                    app.presence.attach(player_id, conn_id);
                }
                if let Some(player_id) = effects.node_session {
                    app.nodes.subscribe(player_id, conn_id, node_tx.clone());
                }
                if let Some(open) = effects.media_open {
                    spawn_media_stream(app.clone(), open, tx.clone());
                }
            }
            Some(frame) = rx.recv() => {
                write_frame(&mut write_half, &frame).await?;
            }
            Some(payload) = node_rx.recv() => {
                let frame = transport::encode_event_envelope(&transport::EventEnvelope {
                    service: Some("node".to_string()),
                    event: "session".to_string(),
                    id: None,
                    payload,
                });
                write_frame(&mut write_half, &frame).await?;
            }
        }
    }
    app.subs.unsubscribe_conn(conn_id);
    app.nodes.unsubscribe_conn(conn_id);
    app.presence.detach_conn(conn_id);
    Ok(())
}

async fn read_frame<R>(read: &mut R) -> anyhow::Result<Option<Vec<u8>>>
where
    R: AsyncRead + Unpin,
{
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

async fn write_frame<W>(write: &mut W, frame: &[u8]) -> anyhow::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let len = u32::try_from(frame.len())?;
    write.write_all(&len.to_be_bytes()).await?;
    write.write_all(frame).await?;
    Ok(())
}

fn spawn_media_stream(app: App, open: MediaOpen, tx: mpsc::UnboundedSender<Vec<u8>>) {
    tokio::task::spawn_blocking(move || {
        if let Err(e) = send_media_stream(app, open, &tx) {
            let fail = MediaEvent::Variant3(MediaFail {
                kind: "fail".to_string(),
                error: ServiceError {
                    code: 500,
                    message: e.to_string(),
                },
            });
            let _ = tx.send(media_frame(&fail));
        }
    });
}

fn send_media_stream(
    app: App,
    open: MediaOpen,
    tx: &mpsc::UnboundedSender<Vec<u8>>,
) -> anyhow::Result<()> {
    let mut conn = app.pool.get()?;
    let track = store::get_track(&mut conn, &open.track_id)?
        .ok_or_else(|| anyhow::anyhow!("track not found"))?;
    let root = app
        .config
        .music_dir
        .clone()
        .ok_or_else(|| anyhow::anyhow!("no music directory configured"))?;
    let path = root.join(&track.root_relative_path);
    let plan = media::plan_stream(&app.config, &track, &open.pref);

    let codec = match plan
        .transcode
        .as_ref()
        .map(|s| s.codec.as_str())
        .unwrap_or(track.codec.as_str())
    {
        "mp3" => Codec::Mp3,
        "aac" => Codec::Aac,
        "flac" => Codec::Flac,
        "opus" => Codec::Opus,
        "vorbis" => Codec::Vorbis,
        "alac" => Codec::Alac,
        "wma" => Codec::Wma,
        _ => Codec::Mp3,
    };
    let header = MediaEvent::Variant0(MediaHeader {
        kind: "header".to_string(),
        codec,
        transcoded: plan.transcode.is_some(),
        sample_rate: track.sample_rate.max(0) as u64,
        channels: track.channels.max(0) as u64,
        duration_ms: Some(track.duration_ms.max(0) as u64),
        trim_start_samples: 0,
        trim_end_samples: 0,
        codec_config: None,
    });
    tx.send(media_frame(&header))?;

    let mut seq = 0u64;
    if let Some(spec) = plan.transcode {
        let ffmpeg = media::resolve_ffmpeg(&app.config)
            .ok_or_else(|| anyhow::anyhow!("ffmpeg not available for transcode"))?;
        let mut child = media::transcode_command(&ffmpeg, &path, &spec, 0).spawn()?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("ffmpeg stdout unavailable"))?;
        send_reader_chunks(&mut stdout, tx, &mut seq)?;
        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("ffmpeg exited with status {status}");
        }
    } else {
        let mut file = std::fs::File::open(path)?;
        send_reader_chunks(&mut file, tx, &mut seq)?;
    }

    let end = MediaEvent::Variant2(MediaEnd {
        kind: "end".to_string(),
        reason: Some(MediaEndReason::Eos),
    });
    tx.send(media_frame(&end))?;
    Ok(())
}

fn send_reader_chunks(
    reader: &mut dyn Read,
    tx: &mpsc::UnboundedSender<Vec<u8>>,
    seq: &mut u64,
) -> anyhow::Result<()> {
    let mut buf = [0u8; 16 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let chunk = MediaEvent::Variant1(MediaChunk {
            kind: "chunk".to_string(),
            seq: *seq,
            timestamp_ms: None,
            data: buf[..n].to_vec(),
        });
        tx.send(media_frame(&chunk))?;
        *seq += 1;
    }
    Ok(())
}

fn media_frame(event: &MediaEvent) -> Vec<u8> {
    transport::encode_event_envelope(&transport::EventEnvelope {
        service: Some("media".to_string()),
        event: "stream".to_string(),
        id: None,
        payload: encode_media_event(event),
    })
}

async fn handle_json_conn<S>(stream: S, app: App) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut lines = BufReader::new(read_half).lines();
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let conn_id = TCP_CONN_ID.fetch_add(1, Ordering::Relaxed);
    let ctx = Ctx {
        identity: Identity::Anonymous,
    };
    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(line) = line? else { break };
                if line.trim().is_empty() {
                    continue;
                }
                if let Some(player_id) = handle_node_session_line(&app, &line, conn_id, tx.clone()) {
                    log::debug!("node session attached for {player_id}");
                    continue;
                }
                let app2 = app.clone();
                let ctx2 = ctx.clone();
                let reply =
                    tokio::task::spawn_blocking(move || transport::handle_json(&app2, &ctx2, &line))
                        .await?;
                write_half.write_all(reply.as_bytes()).await?;
                write_half.write_all(b"\n").await?;
            }
            Some(payload) = rx.recv() => {
                let line = serde_json::to_string(&WirePush {
                    service: "NodeService",
                    op: "session",
                    payload_hex: hex::encode(payload),
                })?;
                write_half.write_all(line.as_bytes()).await?;
                write_half.write_all(b"\n").await?;
            }
        }
    }
    app.nodes.unsubscribe_conn(conn_id);
    Ok(())
}

fn handle_node_session_line(
    app: &App,
    line: &str,
    conn_id: u64,
    tx: mpsc::UnboundedSender<Vec<u8>>,
) -> Option<String> {
    let env: WireEnvelope = serde_json::from_str(line).ok()?;
    let service = env
        .service
        .strip_suffix("Service")
        .unwrap_or(&env.service)
        .to_ascii_lowercase();
    if service != "node" || env.op != "session" || env.id != 0 {
        return None;
    }
    let payload = hex::decode(env.payload_hex).ok()?;
    let report = decode_node_report(&payload).ok()?;
    let player_id = report.player_id.clone();
    let _ = app.record_node_report(report);
    app.nodes.subscribe(player_id.clone(), conn_id, tx);
    Some(player_id)
}
