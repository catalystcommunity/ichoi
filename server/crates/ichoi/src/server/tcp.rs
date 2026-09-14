//! Native CSIL/TCP surface. Binary clients use length-prefixed CSIL-Events envelopes.
//! The old line-delimited JSON form remains for shell/debug tooling only.

use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

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
type MediaOutput = (String, Option<Vec<u8>>);

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
    let (media_tx, mut media_rx) = mpsc::unbounded_channel::<MediaOutput>();
    let (in_tx, mut in_rx) = mpsc::unbounded_channel::<anyhow::Result<Vec<u8>>>();
    let conn_id = TCP_CONN_ID.fetch_add(1, Ordering::Relaxed);
    let mut ident = Identity::Anonymous;
    let mut media_cancels = HashMap::<String, Arc<AtomicBool>>::new();

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
                let (ident_out, mut reply, effects) =
                    tokio::task::spawn_blocking(move || {
                        let allow_guest =
                            app2.config.access_mode == crate::config::AccessMode::Open;
                        transport::handle_events_frame(&app2, ident_in, allow_guest, &frame)
                    }).await?;
                ident = ident_out;
                if let Some((ref player_id, active)) = effects.player_subscription {
                    if active {
                        app.subs.subscribe(player_id.clone(), conn_id, tx.clone());
                        reply = transport::player_subscription_snapshot(&app, &ident, player_id)
                            .or(reply);
                    } else {
                        app.subs.unsubscribe(player_id, conn_id);
                    }
                }
                if let Some(player_id) = effects.attach {
                    if app.presence.attach(player_id, conn_id) {
                        app.changes.publish(libichoi::csil::types::ChangeTopic::Players);
                    }
                }
                if let Some(player_id) = effects.node_session {
                    if app.nodes.subscribe(player_id.clone(), conn_id, node_tx.clone()) {
                        app.changes.publish(libichoi::csil::types::ChangeTopic::Players);
                        let _ = app.reconcile_player_output(&player_id);
                    }
                }
                if let Some(active) = effects.watch_changes {
                    if active {
                        app.changes.subscribe(conn_id, tx.clone());
                    } else {
                        app.changes.unsubscribe(conn_id);
                    }
                }
                // Send the snapshot that was read after attach. A later state change is queued
                // behind it on this connection.
                if let Some(reply) = reply {
                    write_frame(&mut write_half, &reply).await?;
                }
                if let Some(open) = effects.media_open {
                    let stream_id = open.stream_id.clone();
                    let cancel = Arc::new(AtomicBool::new(false));
                    if let Some(previous) = media_cancels.insert(stream_id, cancel.clone()) {
                        previous.store(true, Ordering::Relaxed);
                    }
                    spawn_media_stream(app.clone(), open, cancel, media_tx.clone());
                }
                if let Some(stream_id) = effects.media_stop {
                    if let Some(cancel) = media_cancels.get(&stream_id) {
                        cancel.store(true, Ordering::Relaxed);
                    }
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
            Some((stream_id, frame)) = media_rx.recv() => {
                if let Some(frame) = frame {
                    let active = media_cancels
                        .get(&stream_id)
                        .is_some_and(|cancel| !cancel.load(Ordering::Relaxed));
                    if active {
                        write_frame(&mut write_half, &frame).await?;
                    }
                } else {
                    media_cancels.remove(&stream_id);
                }
            }
        }
    }
    app.subs.unsubscribe_conn(conn_id);
    let players_changed = app.nodes.unsubscribe_conn(conn_id) | app.presence.detach_conn(conn_id);
    app.changes.unsubscribe(conn_id);
    if players_changed {
        app.changes
            .publish(libichoi::csil::types::ChangeTopic::Players);
    }
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

fn spawn_media_stream(
    app: App,
    open: MediaOpen,
    cancel: Arc<AtomicBool>,
    tx: mpsc::UnboundedSender<MediaOutput>,
) {
    tokio::task::spawn_blocking(move || {
        let stream_id = open.stream_id.clone();
        if let Err(e) = send_media_stream(app, open, &cancel, &tx) {
            let fail = MediaEvent::Variant3(MediaFail {
                kind: "error".to_string(),
                stream_id: stream_id.clone(),
                error: ServiceError {
                    code: 500,
                    message: e.to_string(),
                },
            });
            if !cancel.load(Ordering::Relaxed) {
                let _ = tx.send((stream_id.clone(), Some(media_frame(&fail))));
            }
        }
        let _ = tx.send((stream_id, None));
    });
}

fn send_media_stream(
    app: App,
    open: MediaOpen,
    cancel: &AtomicBool,
    tx: &mpsc::UnboundedSender<MediaOutput>,
) -> anyhow::Result<()> {
    let stream_id = open.stream_id.clone();
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
        stream_id: stream_id.clone(),
        codec,
        transcoded: plan.transcode.is_some(),
        sample_rate: track.sample_rate.max(0) as u64,
        channels: track.channels.max(0) as u64,
        duration_ms: Some(track.duration_ms.max(0) as u64),
        trim_start_samples: 0,
        trim_end_samples: 0,
        codec_config: None,
    });
    tx.send((stream_id.clone(), Some(media_frame(&header))))?;

    let mut seq = 0u64;
    if let Some(spec) = plan.transcode {
        let ffmpeg = media::resolve_ffmpeg(&app.config)
            .ok_or_else(|| anyhow::anyhow!("ffmpeg not available for transcode"))?;
        let mut child = media::transcode_command(&ffmpeg, &path, &spec, 0).spawn()?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("ffmpeg stdout unavailable"))?;
        if send_reader_chunks(&mut stdout, &stream_id, cancel, tx, &mut seq)? {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(());
        }
        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("ffmpeg exited with status {status}");
        }
    } else {
        let mut file = std::fs::File::open(path)?;
        if send_reader_chunks(&mut file, &stream_id, cancel, tx, &mut seq)? {
            return Ok(());
        }
    }

    let end = MediaEvent::Variant2(MediaEnd {
        kind: "end".to_string(),
        stream_id: stream_id.clone(),
        reason: Some(MediaEndReason::Eos),
    });
    tx.send((stream_id, Some(media_frame(&end))))?;
    Ok(())
}

fn send_reader_chunks(
    reader: &mut dyn Read,
    stream_id: &str,
    cancel: &AtomicBool,
    tx: &mpsc::UnboundedSender<MediaOutput>,
    seq: &mut u64,
) -> anyhow::Result<bool> {
    let mut buf = [0u8; 16 * 1024];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(true);
        }
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let chunk = MediaEvent::Variant1(MediaChunk {
            kind: "chunk".to_string(),
            stream_id: stream_id.to_string(),
            seq: *seq,
            timestamp_ms: None,
            data: buf[..n].to_vec(),
        });
        tx.send((stream_id.to_string(), Some(media_frame(&chunk))))?;
        *seq += 1;
    }
    Ok(false)
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
        allow_guest: app.config.access_mode == crate::config::AccessMode::Open,
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
    if app.nodes.unsubscribe_conn(conn_id) {
        app.changes
            .publish(libichoi::csil::types::ChangeTopic::Players);
    }
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
    if app.nodes.subscribe(player_id.clone(), conn_id, tx) {
        app.changes
            .publish(libichoi::csil::types::ChangeTopic::Players);
        let _ = app.reconcile_player_output(&player_id);
    }
    Some(player_id)
}
