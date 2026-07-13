//! The media plane (§5): decide direct-vs-transcode, locate ffmpeg, and build transcode
//! commands. The demuxer (direct-mode packet extraction) lives in `demux`.

pub mod demux;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use libichoi::csil::types::{StreamPref, TranscodeCodec};

use crate::config::Config;
use crate::db::models::Track;

pub struct TranscodeSpec {
    pub codec: String,
    pub bitrate_kbps: u32,
}

pub struct StreamPlan {
    /// `None` = direct mode (original packets); `Some` = transcode via ffmpeg.
    pub transcode: Option<TranscodeSpec>,
}

/// Choose the streaming mode for a track and a consumer's preferences (§5.1, §5.2).
pub fn plan_stream(config: &Config, track: &Track, pref: &StreamPref) -> StreamPlan {
    let prefer_original = pref.prefer_original.unwrap_or(false);
    let max = pref.max_bitrate_kbps.map(|b| b as u32);
    let src = track.bitrate_kbps.map(|b| b as u32);

    if libichoi::codec::needs_transcode(&track.codec, prefer_original, max, src) {
        let codec = match pref.transcode_codec {
            Some(TranscodeCodec::Mp3) => "mp3".to_string(),
            Some(TranscodeCodec::Aac) => "aac".to_string(),
            None => config.transcode_codec.clone(),
        };
        let bitrate_kbps = max.unwrap_or(if codec == "mp3" { 192 } else { 256 });
        StreamPlan {
            transcode: Some(TranscodeSpec {
                codec,
                bitrate_kbps,
            }),
        }
    } else {
        StreamPlan { transcode: None }
    }
}

/// Resolve the ffmpeg binary: explicit override, then bundled next to our binary, then
/// system `PATH` (§5.3). `None` means transcoding is unavailable and only direct mode works.
pub fn resolve_ffmpeg(config: &Config) -> Option<PathBuf> {
    if let Some(p) = &config.ffmpeg {
        if p.exists() {
            return Some(p.clone());
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join(if cfg!(windows) {
                "ffmpeg.exe"
            } else {
                "ffmpeg"
            });
            if bundled.is_file() {
                return Some(bundled);
            }
        }
    }
    which("ffmpeg")
}

fn which(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join(&exe))
            .find(|p| p.is_file())
    })
}

/// Build the ffmpeg transcode command that writes the encoded stream to stdout. `-ss` before
/// `-i` gives fast keyframe input seeking for transcode-offset seeking (§5.1). The command is
/// built entirely from validated parameters — there is never a user-editable template (§5.3).
pub fn transcode_command(
    ffmpeg: &Path,
    input: &Path,
    spec: &TranscodeSpec,
    seek_ms: u64,
) -> Command {
    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-v").arg("error").arg("-nostdin");
    if seek_ms > 0 {
        cmd.arg("-ss")
            .arg(format!("{:.3}", seek_ms as f64 / 1000.0));
    }
    cmd.arg("-i").arg(input).arg("-vn").arg("-map").arg("0:a:0");
    match spec.codec.as_str() {
        "mp3" => {
            cmd.arg("-c:a")
                .arg("libmp3lame")
                .arg("-b:a")
                .arg(format!("{}k", spec.bitrate_kbps))
                .arg("-f")
                .arg("mp3");
        }
        _ => {
            cmd.arg("-c:a")
                .arg("aac")
                .arg("-b:a")
                .arg(format!("{}k", spec.bitrate_kbps))
                .arg("-f")
                .arg("adts");
        }
    }
    cmd.arg("pipe:1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(codec: &str, bitrate: Option<i32>) -> Track {
        Track {
            id: "t".into(),
            library_id: "l".into(),
            root_relative_path: "a.flac".into(),
            title: "t".into(),
            artist_id: None,
            album_id: None,
            track_no: None,
            disc_no: None,
            duration_ms: 1000,
            codec: codec.into(),
            bitrate_kbps: bitrate,
            sample_rate: 44100,
            channels: 2,
            bit_depth: Some(16),
            size_bytes: 10,
            mtime: "0".into(),
            content_hash: None,
            trim_start_samples: 0,
            trim_end_samples: 0,
        }
    }

    #[test]
    fn opus_source_transcodes_to_default_aac() {
        let cfg = Config {
            role: crate::config::Role::Core,
            music_dir: None,
            audiobook_dir: None,
            db_dir: None,
            http_addr: String::new(),
            csil_addr: String::new(),
            core_addr: None,
            core_keys: vec![],
            node_token: None,
            admin_token: None,
            ffmpeg: None,
            transcode_codec: "aac".into(),
            web_dir: PathBuf::from("."),
            log: "warn".into(),
            fetch_art: false,
            split_dump_folders: false,
            require_music: false,
        };
        let pref = StreamPref {
            max_bitrate_kbps: None,
            prefer_original: None,
            transcode_codec: None,
        };
        let plan = plan_stream(&cfg, &track("opus", Some(128)), &pref);
        assert_eq!(plan.transcode.unwrap().codec, "aac");

        let direct = plan_stream(&cfg, &track("flac", Some(900)), &pref);
        assert!(direct.transcode.is_none());
    }
}
