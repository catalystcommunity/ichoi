//! Command-line interface. `serve` is one subcommand among the operational tools.

use clap::{Parser, Subcommand};

use crate::app;
use crate::config::Config;
use crate::db::store;
use crate::scan;

#[derive(Parser)]
#[command(
    name = "ichoi",
    about = "Music player, library manager, and distributed jukebox."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the server (HTTP + CSIL surfaces).
    Serve,
    /// Run schema migrations and exit.
    Migrate,
    /// Scan the configured music library and exit.
    Scan,
    /// Fetch missing album cover art from MusicBrainz / Cover Art Archive and save it into
    /// album folders (never modifies music files). Network + rate-limited (~1 album/sec).
    FetchArt {
        /// Maximum albums to look up in this run.
        #[arg(long, default_value_t = 200)]
        limit: usize,
        /// Re-query albums previously looked up (clears the art-checked cache first).
        #[arg(long)]
        retry: bool,
    },
    /// Print the resolved configuration and exit.
    Config,
    /// Print version and exit.
    Version,
}

/// Entry point invoked by `main`.
pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;
    // Quiet lofty's per-file VBR/tag warnings (noise on messy libraries) unless explicitly
    // asked for; keep the user's level for everything else.
    let filter = format!("{},lofty=error", config.log);
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(filter)).init();

    match cli.command {
        Commands::Serve => {
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(app::serve(config))
        }
        Commands::Migrate => {
            app::prepare_db(&config)?;
            println!("migrations applied");
            Ok(())
        }
        Commands::Scan => {
            let pool = app::prepare_db(&config)?;
            let music = config
                .music_dir
                .clone()
                .ok_or_else(|| anyhow::anyhow!("ICHOI_MUSIC_DIR not set"))?;
            let mut conn = pool.get()?;
            store::upsert_library(
                &mut conn,
                &crate::db::models::Library {
                    id: "lib:music".to_string(),
                    kind: "music".to_string(),
                    path: music.to_string_lossy().into_owned(),
                },
            )?;
            let stats =
                scan::scan_library(&mut conn, "lib:music", &music, config.split_dump_folders)?;
            println!("scanned {} tracks ({} errors)", stats.tracks, stats.errors);
            Ok(())
        }
        Commands::FetchArt { limit, retry } => {
            let pool = app::prepare_db(&config)?;
            let mut conn = pool.get()?;
            if retry {
                crate::db::store::reset_art_checked(&mut conn)?;
            }
            let stats = crate::art::fetch_missing(&mut conn, limit)?;
            println!(
                "cover art: {} fetched, {} skipped, {} not found",
                stats.fetched, stats.skipped, stats.failed
            );
            Ok(())
        }
        Commands::Config => {
            println!("role:        {:?}", config.role);
            println!("music_dir:   {:?}", config.music_dir);
            println!("db:          {}", config.database_url());
            println!("http_addr:   {}", config.http_addr);
            println!("csil_addr:   {}", config.csil_addr);
            println!("web_dir:     {}", config.web_dir.display());
            println!("transcode:   {}", config.transcode_codec);
            Ok(())
        }
        Commands::Version => {
            println!("ichoi {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}
