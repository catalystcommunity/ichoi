//! HTTP media delivery — a **testing bridge** (not the pure design).
//!
//! DESIGN §5 routes media over CSIL-Events to a Symphonia→WASM decoder. That decoder does
//! not exist yet, so browsers cannot decode the CSIL media stream. Until it lands, this
//! endpoint serves audio over plain HTTP so native `<audio>` can actually play it: direct
//! byte-range for client-decodable codecs, or an ffmpeg transcode pipe otherwise / on a
//! bitrate cap. Documented in IMPLEMENTATION.md.

use std::path::PathBuf;

use axum::body::Body;
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use libichoi::csil::types::{StreamPref, TranscodeCodec};
use serde::Deserialize;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
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
    connect: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    let app = state.app.clone();
    if !super::http::request_allowed(&app, &headers, connect) {
        return (StatusCode::UNAUTHORIZED, "sign in required").into_response();
    }

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
    let mut file = match tokio::fs::File::open(&path).await {
        Ok(file) => file,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let len = match file.metadata().await {
        Ok(meta) => meta.len(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let ct = if matches!(extension.as_str(), "m4a" | "m4b" | "mp4") {
        "audio/mp4"
    } else {
        content_type(codec)
    };
    let mut response = Response::builder()
        .header(header::CONTENT_TYPE, ct)
        .header(header::ACCEPT_RANGES, "bytes");
    let size = if let Some(range) = headers.get(header::RANGE) {
        let Some((start, end)) = range
            .to_str()
            .ok()
            .and_then(|range| parse_range(range, len))
        else {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{len}"))
                .body(Body::empty())
                .unwrap();
        };
        if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        response = response
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"));
        end - start + 1
    } else {
        len
    };
    // Large M4B files must not be read into RAM for each seek request.
    response
        .header(header::CONTENT_LENGTH, size)
        .body(Body::from_stream(ReaderStream::new(file.take(size))))
        .unwrap()
}

fn parse_range(range: &str, len: u64) -> Option<(u64, u64)> {
    if len == 0 {
        return None;
    }
    let (start, end) = range.strip_prefix("bytes=")?.split_once('-')?;
    if start.is_empty() {
        let suffix: u64 = end.parse().ok()?;
        return (suffix > 0).then_some((len.saturating_sub(suffix), len - 1));
    }
    let start: u64 = start.parse().ok()?;
    let end = if end.is_empty() {
        len - 1
    } else {
        end.parse::<u64>().ok()?.min(len - 1)
    };
    (start <= end && start < len).then_some((start, end))
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
