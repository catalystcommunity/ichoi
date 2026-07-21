//! Server bootstrap: build the pool, migrate, run transforms, kick a scan, and serve the
//! HTTP and CSIL surfaces concurrently.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::config::{Config, Role};
use crate::db::{self, models, store};
use crate::handlers::App;
use crate::{audio, scan, server};

pub fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "ichoi-core".to_string())
}

/// Run migrations + boot transforms against a fresh pool. Shared by `serve` and `migrate`.
pub fn prepare_db(config: &Config) -> anyhow::Result<db::SqlitePool> {
    let pool = db::establish_pool(&config.database_url())?;
    let mut conn = pool.get()?;
    db::run_migrations(&mut conn)?;
    db::run_transforms(&mut conn, &hostname())?;
    drop(conn);
    crate::auth::local_rp::initialize_database(&pool, config)?;
    let mut conn = pool.get()?;
    let outputs = audio::enumerate();
    store::sync_core_outputs(&mut conn, &hostname(), &outputs)?;
    Ok(pool)
}

fn ensure_library(
    pool: &db::SqlitePool,
    kind: &str,
    path: &std::path::Path,
) -> anyhow::Result<String> {
    let id = format!("lib:{kind}");
    let mut conn = pool.get()?;
    store::upsert_library(
        &mut conn,
        &models::Library {
            id: id.clone(),
            kind: kind.to_string(),
            path: path.to_string_lossy().into_owned(),
        },
    )?;
    Ok(id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanStart {
    Started,
    AlreadyRunning,
    NoLibraries,
}

struct ScanRunningGuard(Arc<std::sync::atomic::AtomicBool>);

impl Drop for ScanRunningGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl App {
    /// Start the same full, reconciling scan used at startup. Only one scan may run at once.
    pub fn start_library_scan(&self, fetch_art: bool) -> anyhow::Result<ScanStart> {
        let mut configured = Vec::new();
        for (kind, path) in [
            ("music", self.config.music_dir.clone()),
            ("audiobook", self.config.audiobook_dir.clone()),
        ] {
            if let Some(path) = path {
                if path.is_dir() {
                    let id = ensure_library(&self.pool, kind, &path)?;
                    configured.push((id, kind.to_string(), path));
                } else {
                    log::warn!(
                        "{kind} dir {} does not exist; skipping scan",
                        path.display()
                    );
                }
            }
        }
        if configured.is_empty() {
            return Ok(ScanStart::NoLibraries);
        }
        if self
            .scan_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(ScanStart::AlreadyRunning);
        }

        let pool = self.pool.clone();
        let config = self.config.clone();
        let running = self.scan_running.clone();
        std::thread::spawn(move || {
            let _guard = ScanRunningGuard(running);
            let mut conn = match pool.get() {
                Ok(conn) => conn,
                Err(error) => {
                    log::error!("scan: no db connection: {error}");
                    return;
                }
            };
            let audiobook_root = configured
                .iter()
                .find(|(_, kind, _)| kind == "audiobook")
                .map(|(_, _, path)| path.clone());
            for (library_id, kind, root) in configured {
                let excluded = (kind == "music")
                    .then_some(audiobook_root.as_deref())
                    .flatten();
                match scan::scan_library(
                    &mut conn,
                    &library_id,
                    &root,
                    excluded,
                    config.split_dump_folders,
                    config.album_subfolder_flat,
                    &config.album_subfolder_words,
                ) {
                    Ok(stats) => log::info!(
                        "{kind} scan complete: {} tracks, {} errors",
                        stats.tracks,
                        stats.errors
                    ),
                    Err(error) => log::error!("{kind} scan failed: {error}"),
                }
            }
            if fetch_art && config.fetch_art {
                log::info!("cover art: fetching missing art in background");
                match crate::art::fetch_missing(&mut conn, 1_000_000) {
                    Ok(stats) => log::info!(
                        "cover art: {} fetched, {} skipped, {} not found",
                        stats.fetched,
                        stats.skipped,
                        stats.failed
                    ),
                    Err(error) => log::warn!("cover art: {error}"),
                }
            }
        });
        Ok(ScanStart::Started)
    }

    pub fn library_scan_running(&self) -> bool {
        self.scan_running.load(Ordering::Acquire)
    }
}

/// The `serve` command.
pub async fn serve(config: Config) -> anyhow::Result<()> {
    validate_runtime_config(&config)?;
    if config.role == Role::Satellite {
        return crate::satellite::run(config).await;
    }
    let config = Arc::new(config);
    let pool = prepare_db(&config)?;

    let app = App::new(pool.clone(), config.clone());

    // Catalog rows for a disabled library remain dormant so re-enabling it preserves progress.
    let _ = app.start_library_scan(true)?;

    let http_router = server::http::router(app.clone(), config.web_dir.clone());
    let http_listener = tokio::net::TcpListener::bind(&config.http_addr).await?;
    log::info!(
        "HTTP on {} (serving {})",
        config.http_addr,
        config.web_dir.display()
    );

    let tcp = server::tcp::serve_tcp(app.clone(), config.csil_addr.clone());

    tokio::select! {
        r = axum::serve(http_listener, http_router) => r?,
        r = tcp => r?,
        _ = tokio::signal::ctrl_c() => log::info!("shutting down"),
    }
    Ok(())
}

fn validate_runtime_config(config: &Config) -> anyhow::Result<()> {
    if config.role == Role::Satellite {
        if config.core_addr.as_deref().unwrap_or_default().is_empty() {
            anyhow::bail!("satellite role requires ICHOI_CORE_ADDR");
        }
        if config.node_token.as_deref().unwrap_or_default().is_empty() {
            anyhow::bail!("satellite role requires ICHOI_NODE_TOKEN");
        }
        crate::tls::client_config(&config.core_keys)?;
    }

    if config.require_music {
        let music = config
            .music_dir
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("ICHOI_REQUIRE_MUSIC=1 requires ICHOI_MUSIC_DIR"))?;
        if !music.is_dir() {
            anyhow::bail!(
                "music dir {} does not exist or is not a directory",
                music.display()
            );
        }
        let mut entries = std::fs::read_dir(music)
            .map_err(|e| anyhow::anyhow!("reading music dir {}: {e}", music.display()))?;
        if entries.next().is_none() {
            anyhow::bail!(
                "music dir {} is empty; mount your music library or unset ICHOI_REQUIRE_MUSIC",
                music.display()
            );
        }
    }

    if config.linkkeys_local_rp {
        if config
            .linkkeys_local_rp_name
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            anyhow::bail!("ICHOI_LINKKEYS_LOCAL_RP=true requires ICHOI_LINKKEYS_LOCAL_RP_NAME");
        }
        if config.linkkeys_trusted_identities.is_empty() {
            anyhow::bail!(
                "ICHOI_LINKKEYS_LOCAL_RP=true requires ICHOI_LINKKEYS_TRUSTED_IDENTITIES"
            );
        }
        for selector in &config.linkkeys_trusted_identities {
            crate::auth::local_rp::parse_selector(selector)?;
        }
    }

    Ok(())
}
