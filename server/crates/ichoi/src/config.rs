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
    tls_cert: Option<PathBuf>,
    tls_key: Option<PathBuf>,
    node_token: Option<String>,
    admin_token: Option<String>,
    ffmpeg: Option<PathBuf>,
    transcode_codec: Option<String>,
    web_dir: Option<PathBuf>,
    log: Option<String>,
    require_music: Option<bool>,
    album_subfolder_flat: Option<bool>,
    album_subfolder_words: Option<Vec<String>>,
    linkkeys_local_rp: Option<bool>,
    linkkeys_local_rp_name: Option<String>,
    linkkeys_trusted_identities: Option<Vec<String>>,
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
    /// Core CSIL/TLS certificate. Generated on first core startup when absent.
    pub tls_cert: Option<PathBuf>,
    /// Core CSIL/TLS private key. Generated on first core startup when absent.
    pub tls_key: Option<PathBuf>,
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
    /// Collapse disc-like album subfolders (for example `CD1` and `Bonus Disc`) into their
    /// parent folder. Enabled by default.
    pub album_subfolder_flat: bool,
    /// Case-insensitive, fuzzy-matched words and phrases identifying disc subfolders.
    pub album_subfolder_words: Vec<String>,
    /// Fail startup when the configured music directory is absent or empty. Intended for
    /// Docker Compose/local deployments where an empty bind mount usually means a bad mount.
    pub require_music: bool,
    /// DNS-less LinkKeys RP mode. An environment override enables it only
    /// when its value is exactly `true`.
    pub linkkeys_local_rp: bool,
    /// Human-readable name embedded in the local RP's signed descriptor.
    pub linkkeys_local_rp_name: Option<String>,
    /// Admission selectors: either `domain` or `handle@domain`.
    pub linkkeys_trusted_identities: Vec<String>,
}

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|s| !s.is_empty())
}

fn exactly_true(value: Option<&str>) -> bool {
    value == Some("true")
}

const DEFAULT_ALBUM_SUBFOLDER_WORDS: &[&str] = &["cd", "disc", "disk", "bonus disc"];

fn configurable_bool(value: Option<&str>, file_value: Option<bool>, default: bool) -> bool {
    match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("1" | "true" | "yes" | "on") => true,
        Some("0" | "false" | "no" | "off") => false,
        Some(_) => default,
        None => file_value.unwrap_or(default),
    }
}

fn album_subfolder_words(value: Option<String>) -> Vec<String> {
    value
        .map(|words| {
            words
                .split(',')
                .map(str::trim)
                .filter(|word| !word.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_else(|| {
            DEFAULT_ALBUM_SUBFOLDER_WORDS
                .iter()
                .map(|word| (*word).to_string())
                .collect()
        })
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
        let linkkeys_local_rp = match std::env::var("ICHOI_LINKKEYS_LOCAL_RP") {
            Ok(value) => exactly_true(Some(&value)),
            Err(_) => file.linkkeys_local_rp.unwrap_or(false),
        };
        let music_dir =
            pick_opt("ICHOI_MUSIC_DIR", file.music_dir.map(pb_to_string)).map(PathBuf::from);
        let audiobook_dir = pick_opt("ICHOI_AUDIOBOOK_DIR", file.audiobook_dir.map(pb_to_string))
            .map(PathBuf::from);
        let db_dir = pick_opt("ICHOI_DB_DIR", file.db_dir.map(pb_to_string)).map(PathBuf::from);
        let album_subfolder_flat = configurable_bool(
            env("ICHOI_ALBUM_SUBFOLDER_FLAT").as_deref(),
            file.album_subfolder_flat,
            true,
        );
        let album_subfolder_words = album_subfolder_words(pick_opt(
            "ICHOI_ALBUM_SUBFOLDER_WORDS",
            file.album_subfolder_words.map(|words| words.join(",")),
        ));

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
            tls_cert: pick_opt("ICHOI_TLS_CERT", file.tls_cert.map(pb_to_string))
                .map(PathBuf::from),
            tls_key: pick_opt("ICHOI_TLS_KEY", file.tls_key.map(pb_to_string)).map(PathBuf::from),
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
            album_subfolder_flat,
            album_subfolder_words,
            require_music: matches!(
                env("ICHOI_REQUIRE_MUSIC")
                    .or_else(|| file
                        .require_music
                        .map(|v| if v { "1" } else { "0" }.to_string()))
                    .as_deref(),
                Some("1") | Some("true") | Some("yes")
            ),
            linkkeys_local_rp,
            linkkeys_local_rp_name: pick_opt(
                "ICHOI_LINKKEYS_LOCAL_RP_NAME",
                file.linkkeys_local_rp_name,
            ),
            linkkeys_trusted_identities: pick_opt(
                "ICHOI_LINKKEYS_TRUSTED_IDENTITIES",
                file.linkkeys_trusted_identities.map(|v| v.join(",")),
            )
            .map(|s| {
                s.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
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

    /// Directory used for generated core transport identity files.
    pub fn tls_dir(&self) -> PathBuf {
        self.db_dir
            .clone()
            .or_else(|| self.music_dir.clone())
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

fn pb_to_string(p: PathBuf) -> String {
    p.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{album_subfolder_words, configurable_bool, exactly_true, FileConfig};

    #[test]
    fn local_rp_environment_enablement_is_exact() {
        for value in ["1", "yes", "TRUE", "True", "false", "true ", " true"] {
            assert!(!exactly_true(Some(value)), "enabled for {value:?}");
        }
        assert!(exactly_true(Some("true")));
        assert!(!exactly_true(None));
    }

    #[test]
    fn transport_security_fields_parse_from_toml() {
        let parsed: FileConfig = toml::from_str(
            r#"
core_keys = ["sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]
tls_cert = "/state/cert.der"
tls_key = "/state/key.der"
"#,
        )
        .unwrap();
        assert_eq!(parsed.core_keys.unwrap().len(), 1);
        assert_eq!(parsed.tls_cert.unwrap(), PathBuf::from("/state/cert.der"));
        assert_eq!(parsed.tls_key.unwrap(), PathBuf::from("/state/key.der"));
    }

    #[test]
    fn audiobook_directory_parses_from_toml() {
        let parsed: FileConfig = toml::from_str(
            r#"
audiobook_dir = "/media/audiobooks"
"#,
        )
        .unwrap();
        assert_eq!(
            parsed.audiobook_dir.unwrap(),
            PathBuf::from("/media/audiobooks")
        );
    }

    #[test]
    fn album_subfolder_configuration_parses_and_defaults_enabled() {
        let parsed: FileConfig = toml::from_str(
            r#"
album_subfolder_flat = false
album_subfolder_words = ["part", "supplement"]
"#,
        )
        .unwrap();
        assert!(!parsed.album_subfolder_flat.unwrap());
        assert_eq!(
            parsed.album_subfolder_words.unwrap(),
            vec!["part", "supplement"]
        );
        assert!(configurable_bool(None, None, true));
        assert!(!configurable_bool(Some("false"), Some(true), true));
        assert!(configurable_bool(Some("true"), Some(false), true));
        assert_eq!(
            album_subfolder_words(None),
            vec!["cd", "disc", "disk", "bonus disc"]
        );
        assert_eq!(
            album_subfolder_words(Some("part, supplement ".to_string())),
            vec!["part", "supplement"]
        );
    }
}
