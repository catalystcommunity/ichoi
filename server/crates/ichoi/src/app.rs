//! Server bootstrap: build the pool, migrate, run transforms, kick a scan, and serve the
//! HTTP and CSIL surfaces concurrently.

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

fn ensure_music_library(pool: &db::SqlitePool, path: &std::path::Path) -> anyhow::Result<String> {
    let id = "lib:music".to_string();
    let mut conn = pool.get()?;
    store::upsert_library(
        &mut conn,
        &models::Library {
            id: id.clone(),
            kind: "music".to_string(),
            path: path.to_string_lossy().into_owned(),
        },
    )?;
    Ok(id)
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

    // Kick off a background scan of the music library if configured.
    if let Some(music) = config.music_dir.clone() {
        if music.is_dir() {
            let library_id = ensure_music_library(&pool, &music)?;
            let pool2 = pool.clone();
            let fetch_art = config.fetch_art;
            let split_dumps = config.split_dump_folders;
            tokio::task::spawn_blocking(move || {
                let mut conn = match pool2.get() {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!("scan: no db connection: {e}");
                        return;
                    }
                };
                match scan::scan_library(&mut conn, &library_id, &music, split_dumps) {
                    Ok(stats) => log::info!(
                        "scan complete: {} tracks, {} errors",
                        stats.tracks,
                        stats.errors
                    ),
                    Err(e) => log::error!("scan failed: {e}"),
                }
                // Cover-art fill-in trickles in afterward (rate-limited network); it only
                // touches albums not yet checked, so it is cheap on later startups.
                if fetch_art {
                    log::info!("cover art: fetching missing art in background");
                    match crate::art::fetch_missing(&mut conn, 1_000_000) {
                        Ok(s) => log::info!(
                            "cover art: {} fetched, {} skipped, {} not found",
                            s.fetched,
                            s.skipped,
                            s.failed
                        ),
                        Err(e) => log::warn!("cover art: {e}"),
                    }
                }
            });
        } else {
            log::warn!(
                "music dir {} does not exist; skipping scan",
                music.display()
            );
        }
    }

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
