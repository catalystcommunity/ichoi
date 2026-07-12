//! Configuration: environment variables (`ICHOI_`-prefixed) override an optional TOML
//! file, which overrides defaults (§9).

use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Core,
    Satellite,
}

impl Role {
    fn parse(s: &str) -> Role {
        match s.to_ascii_lowercase().as_str() {
            "satellite" => Role::Satellite,
            _ => Role::Core,
        }
    }
}

/// The TOML file shape; every field optional so a missing/partial file is fine.
#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    role: Option<String>,
    music_dir: Option<PathBuf>,
    audiobook_dir: Option<PathBuf>,
    db_dir: Option<PathBuf>,
    http_addr: Option<String>,
    csil_addr: Option<String>,
    core_addr: Option<String>,
    core_keys: Option<Vec<String>>,
    node_token: Option<String>,
    admin_token: Option<String>,
    ffmpeg: Option<PathBuf>,
    transcode_codec: Option<String>,
    web_dir: Option<PathBuf>,
    log: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub role: Role,
    pub music_dir: Option<PathBuf>,
    pub audiobook_dir: Option<PathBuf>,
    pub db_dir: Option<PathBuf>,
    pub http_addr: String,
    pub csil_addr: String,
    pub core_addr: Option<String>,
    pub core_keys: Vec<String>,
    pub node_token: Option<String>,
    pub admin_token: Option<String>,
    pub ffmpeg: Option<PathBuf>,
    pub transcode_codec: String,
    pub web_dir: PathBuf,
    pub log: String,
    /// Fetch missing cover art from MusicBrainz/CAA at startup (default on; set
    /// `ICHOI_FETCH_ART=0` to disable, e.g. offline).
    pub fetch_art: bool,
    /// Split "dump" folders (many artists / many loose files) into per-artist "Singles"
    /// albums instead of one folder-album (default off; `ICHOI_SPLIT_DUMP_FOLDERS=1`).
    pub split_dump_folders: bool,
}

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|s| !s.is_empty())
}

impl Config {
    /// Resolve config: env → file (`ICHOI_CONFIG` path or `./ichoi.toml`) → defaults.
    pub fn load() -> anyhow::Result<Config> {
        let file_path = env("ICHOI_CONFIG").unwrap_or_else(|| "ichoi.toml".to_string());
        let file: FileConfig = match std::fs::read_to_string(&file_path) {
            Ok(text) => toml::from_str(&text)
                .map_err(|e| anyhow::anyhow!("parsing config {file_path}: {e}"))?,
            Err(_) => FileConfig::default(),
        };

        let pick = |envk: &str, filev: Option<String>, default: &str| -> String {
            env(envk).or(filev).unwrap_or_else(|| default.to_string())
        };
        let pick_opt =
            |envk: &str, filev: Option<String>| -> Option<String> { env(envk).or(filev) };

        let role = Role::parse(&pick("ICHOI_ROLE", file.role, "core"));
        let music_dir =
            pick_opt("ICHOI_MUSIC_DIR", file.music_dir.map(pb_to_string)).map(PathBuf::from);
        let audiobook_dir = pick_opt("ICHOI_AUDIOBOOK_DIR", file.audiobook_dir.map(pb_to_string))
            .map(PathBuf::from);
        let db_dir = pick_opt("ICHOI_DB_DIR", file.db_dir.map(pb_to_string)).map(PathBuf::from);

        Ok(Config {
            role,
            music_dir,
            audiobook_dir,
            db_dir,
            http_addr: pick("ICHOI_HTTP_ADDR", file.http_addr, "0.0.0.0:4042"),
            csil_addr: pick("ICHOI_CSIL_ADDR", file.csil_addr, "0.0.0.0:4043"),
            core_addr: pick_opt("ICHOI_CORE_ADDR", file.core_addr),
            core_keys: pick_opt("ICHOI_CORE_KEYS", file.core_keys.map(|v| v.join(",")))
                .map(|s| {
                    s.split(',')
                        .map(|x| x.trim().to_string())
                        .filter(|x| !x.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            node_token: pick_opt("ICHOI_NODE_TOKEN", file.node_token),
            admin_token: pick_opt("ICHOI_ADMIN_TOKEN", file.admin_token),
            ffmpeg: pick_opt("ICHOI_FFMPEG", file.ffmpeg.map(pb_to_string)).map(PathBuf::from),
            transcode_codec: pick("ICHOI_TRANSCODE_CODEC", file.transcode_codec, "aac"),
            web_dir: PathBuf::from(pick(
                "ICHOI_WEB_DIR",
                file.web_dir.map(pb_to_string),
                "web/themes/default",
            )),
            log: pick("ICHOI_LOG", file.log, "warn"),
            fetch_art: !matches!(
                env("ICHOI_FETCH_ART").as_deref(),
                Some("0") | Some("false") | Some("no")
            ),
            split_dump_folders: matches!(
                env("ICHOI_SPLIT_DUMP_FOLDERS").as_deref(),
                Some("1") | Some("true") | Some("yes")
            ),
        })
    }

    /// The SQLite database file path: `<db_dir or music_dir>/ichoi.db`, else `./ichoi.db`.
    pub fn database_url(&self) -> String {
        let dir = self
            .db_dir
            .clone()
            .or_else(|| self.music_dir.clone())
            .unwrap_or_else(|| PathBuf::from("."));
        dir.join("ichoi.db").to_string_lossy().into_owned()
    }
}

fn pb_to_string(p: PathBuf) -> String {
    p.to_string_lossy().into_owned()
}
