//! HTTP media delivery — a **testing bridge** (not the pure design).
//!
//! DESIGN §5 routes media over CSIL-Events to a Symphonia→WASM decoder. That decoder does
//! not exist yet, so browsers cannot decode the CSIL media stream. Until it lands, this
//! endpoint serves audio over plain HTTP so native `<audio>` can actually play it: direct
//! byte-range for client-decodable codecs, or an ffmpeg transcode pipe otherwise / on a
//! bitrate cap. Documented in IMPLEMENTATION.md.

use std::path::PathBuf;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use libichoi::csil::types::{StreamPref, TranscodeCodec};
use serde::Deserialize;
use tokio_util::io::ReaderStream;

use crate::db::store;
use crate::media;

use super::http::AppState;

#[derive(Deserialize)]
pub struct MediaQuery {
    /// Cap bitrate in kbps (triggers transcode).
    bitrate: Option<u32>,
    /// Force a transcode target: `aac` or `mp3`.
    format: Option<String>,
}

pub async fn stream_media(
    State(state): State<AppState>,
    Path(track_id): Path<String>,
    Query(q): Query<MediaQuery>,
    headers: HeaderMap,
) -> Response {
    let app = state.app.clone();

    // Resolve the track's absolute path and plan (blocking DB work off the async thread).
    let planned = tokio::task::spawn_blocking(move || resolve(&app, &track_id, &q))
        .await
        .unwrap_or(Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "task join".to_string(),
        )));

    let plan = match planned {
        Ok(p) => p,
        Err((code, msg)) => return (code, msg).into_response(),
    };

    match plan {
        Resolved::Direct { path, codec } => serve_direct(path, &codec, &headers).await,
        Resolved::Transcode { path, spec, ffmpeg } => serve_transcode(ffmpeg, path, spec).await,
    }
}

enum Resolved {
    Direct {
        path: PathBuf,
        codec: String,
    },
    Transcode {
        path: PathBuf,
        spec: media::TranscodeSpec,
        ffmpeg: PathBuf,
    },
}

fn resolve(
    app: &crate::handlers::App,
    track_id: &str,
    q: &MediaQuery,
) -> Result<Resolved, (StatusCode, String)> {
    let mut conn = app
        .pool
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let track = store::get_track(&mut conn, track_id)
        .ok()
        .flatten()
        .ok_or((StatusCode::NOT_FOUND, "track not found".to_string()))?;
    let library = store::get_library(&mut conn, &track.library_id)
        .ok()
        .flatten()
        .ok_or((StatusCode::NOT_FOUND, "library not found".to_string()))?;
    let path = PathBuf::from(library.path).join(&track.root_relative_path);
    if !path.is_file() {
        return Err((StatusCode::NOT_FOUND, "file missing".to_string()));
    }

    let pref = StreamPref {
        max_bitrate_kbps: q.bitrate.map(u64::from),
        prefer_original: Some(q.format.is_none() && q.bitrate.is_none()),
        transcode_codec: match q.format.as_deref() {
            Some("mp3") => Some(TranscodeCodec::Mp3),
            Some("aac") => Some(TranscodeCodec::Aac),
            _ => None,
        },
    };
    let plan = media::plan_stream(&app.config, &track, &pref);

    match plan.transcode {
        None => Ok(Resolved::Direct {
            path,
            codec: track.codec,
        }),
        Some(spec) => {
            let ffmpeg = media::resolve_ffmpeg(&app.config).ok_or((
                StatusCode::SERVICE_UNAVAILABLE,
                "transcode requested but no ffmpeg found (bundled or on PATH)".to_string(),
            ))?;
            Ok(Resolved::Transcode { path, spec, ffmpeg })
        }
    }
}

fn content_type(codec: &str) -> &'static str {
    match codec {
        "flac" => "audio/flac",
        "mp3" => "audio/mpeg",
        "aac" => "audio/aac",
        "wav" => "audio/wav",
        "vorbis" => "audio/ogg",
        "opus" => "audio/ogg",
        "alac" => "audio/mp4",
        _ => "application/octet-stream",
    }
}

/// Serve the original file, honoring a single `Range` request for seeking.
async fn serve_direct(path: PathBuf, codec: &str, headers: &HeaderMap) -> Response {
    let data = match tokio::fs::read(&path).await {
        Ok(d) => d,
        Err(e) => return (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    };
    let len = data.len() as u64;
    let ct = content_type(codec);

    if let Some(range) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        if let Some((start, end)) = parse_range(range, len) {
            let slice = data[start as usize..=(end as usize)].to_vec();
            return Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, ct)
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"))
                .header(header::CONTENT_LENGTH, (end - start + 1).to_string())
                .body(Body::from(slice))
                .unwrap();
        }
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, ct)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, len.to_string())
        .body(Body::from(data))
        .unwrap()
}

fn parse_range(range: &str, len: u64) -> Option<(u64, u64)> {
    let spec = range.strip_prefix("bytes=")?;
    let (a, b) = spec.split_once('-')?;
    let start: u64 = a.parse().ok()?;
    let end: u64 = if b.is_empty() {
        len - 1
    } else {
        b.parse().ok()?
    };
    if start > end || end >= len {
        return None;
    }
    Some((start, end))
}

/// Pipe an ffmpeg transcode straight to the response body (chunked, no Content-Length).
async fn serve_transcode(ffmpeg: PathBuf, path: PathBuf, spec: media::TranscodeSpec) -> Response {
    let ct = if spec.codec == "mp3" {
        "audio/mpeg"
    } else {
        "audio/aac"
    };
    let mut cmd = tokio::process::Command::from(media::transcode_command(&ffmpeg, &path, &spec, 0));
    cmd.kill_on_drop(true);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, "no ffmpeg stdout").into_response(),
    };
    // Detach the child; kill_on_drop cleans it up when the body stream is dropped.
    tokio::spawn(async move {
        let _ = child.wait().await;
    });
    let stream = ReaderStream::new(stdout);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, ct)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from_stream(stream))
        .unwrap()
}
