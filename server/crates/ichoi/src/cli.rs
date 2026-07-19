//! Command-line interface. `serve` is one subcommand among the operational tools.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

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
    /// Print the core's pinned CSIL/TLS fingerprint, generating its identity if needed.
    CoreFingerprint,
    /// Install this binary as a native satellite service.
    Install(InstallArgs),
    /// Remove a native satellite service installation.
    Uninstall(UninstallArgs),
    /// Ask the native service manager for satellite status.
    Status(StatusArgs),
    /// Run Ichoi using an explicit configuration path (used by user service managers).
    #[command(hide = true)]
    ServeWithConfig { config: PathBuf },
    /// Enter the Windows Service Control Manager dispatcher.
    #[command(hide = true)]
    ServiceRun { config: PathBuf },
    /// Print version and exit.
    Version,
}

#[derive(Args)]
struct InstallArgs {
    /// Install an outbound satellite audio node.
    #[arg(value_name = "MODE", default_value = "satellite", value_parser = ["satellite"])]
    mode: String,
    /// Core socket address, such as 192.168.1.20:4043.
    #[arg(long)]
    core: String,
    /// Pinned sha256 core certificate fingerprint; repeat during key rotation.
    #[arg(long = "core-key", required = true)]
    core_keys: Vec<String>,
    /// Read the node token from this file.
    #[arg(long, conflicts_with = "node_token_stdin")]
    node_token_file: Option<PathBuf>,
    /// Read the node token from stdin.
    #[arg(long, conflicts_with = "node_token_file")]
    node_token_stdin: bool,
    /// Install for the whole machine or the current user. Defaults to system on Linux and user elsewhere.
    #[arg(long, value_enum)]
    scope: Option<crate::install::Scope>,
    /// Override the platform-native configuration path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Override the directory in which the executable is installed.
    #[arg(long)]
    install_dir: Option<PathBuf>,
    /// Do not register the service for future logins/boots.
    #[arg(long)]
    no_enable: bool,
    /// Install and enable without starting it now.
    #[arg(long)]
    no_start: bool,
    /// Replace an existing binary, configuration, and service definition.
    #[arg(long)]
    replace: bool,
    /// Print the complete installation plan without changing the machine.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args)]
struct UninstallArgs {
    /// Scope used when the satellite was installed.
    #[arg(long, value_enum)]
    scope: Option<crate::install::Scope>,
    /// Keep the satellite configuration and node credential.
    #[arg(long)]
    keep_config: bool,
    /// Keep the installed executable.
    #[arg(long)]
    keep_binary: bool,
    /// Override the installed configuration path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Override the installed executable directory.
    #[arg(long)]
    install_dir: Option<PathBuf>,
    /// Print the removal plan without changing the machine.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args)]
struct StatusArgs {
    /// Scope used when the satellite was installed.
    #[arg(long, value_enum)]
    scope: Option<crate::install::Scope>,
}

/// Entry point invoked by `main`.
pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Install(args) => {
            debug_assert_eq!(args.mode, "satellite");
            let node_token = crate::install::read_node_token(
                args.node_token_file.as_deref(),
                args.node_token_stdin,
            )?;
            let options = crate::install::InstallOptions {
                core_addr: args.core,
                core_keys: args.core_keys,
                node_token,
                scope: args.scope,
                config_path: args.config,
                install_dir: args.install_dir,
                start: !args.no_start,
                enable: !args.no_enable,
                replace: args.replace,
                dry_run: args.dry_run,
            };
            let plan = crate::install::plan(&options)?;
            print!("{}", crate::install::describe(&plan));
            if !options.dry_run {
                crate::install::apply(&plan, options.replace)?;
                println!("Ichoi satellite installed");
            }
            Ok(())
        }
        Commands::Uninstall(args) => crate::install::uninstall(
            args.scope,
            args.config,
            args.install_dir,
            args.keep_config,
            args.keep_binary,
            args.dry_run,
        ),
        Commands::Status(args) => crate::install::status(args.scope),
        Commands::ServeWithConfig { config } => crate::install::serve_with_config(&config),
        Commands::ServiceRun { config } => crate::install::run_windows_service(config),
        Commands::Version => {
            println!("ichoi {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        command => run_configured(command),
    }
}

fn run_configured(command: Commands) -> anyhow::Result<()> {
    let config = Config::load()?;
    // Quiet lofty's per-file VBR/tag warnings (noise on messy libraries) unless explicitly
    // asked for; keep the user's level for everything else.
    let filter = format!("{},lofty=error", config.log);
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(filter)).init();

    match command {
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
            let mut conn = pool.get()?;
            let mut configured = 0;
            for (kind, root) in [
                ("music", config.music_dir.as_ref()),
                ("audiobook", config.audiobook_dir.as_ref()),
            ] {
                let Some(root) = root else { continue };
                configured += 1;
                let library_id = format!("lib:{kind}");
                store::upsert_library(
                    &mut conn,
                    &crate::db::models::Library {
                        id: library_id.clone(),
                        kind: kind.to_string(),
                        path: root.to_string_lossy().into_owned(),
                    },
                )?;
                let excluded = if kind == "music" {
                    config.audiobook_dir.as_deref()
                } else {
                    None
                };
                let stats = scan::scan_library(
                    &mut conn,
                    &library_id,
                    root,
                    excluded,
                    config.split_dump_folders,
                    config.album_subfolder_flat,
                    &config.album_subfolder_words,
                )?;
                println!(
                    "scanned {kind}: {} tracks ({} errors)",
                    stats.tracks, stats.errors
                );
            }
            if configured == 0 {
                anyhow::bail!("ICHOI_MUSIC_DIR and ICHOI_AUDIOBOOK_DIR are not set");
            }
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
            println!("audiobooks:  {:?}", config.audiobook_dir);
            println!("album_flat:  {}", config.album_subfolder_flat);
            println!("album_words: {:?}", config.album_subfolder_words);
            println!("db:          {}", config.database_url());
            println!("http_addr:   {}", config.http_addr);
            println!("csil_addr:   {}", config.csil_addr);
            println!("web_dir:     {}", config.web_dir.display());
            println!("transcode:   {}", config.transcode_codec);
            Ok(())
        }
        Commands::CoreFingerprint => {
            let identity = crate::tls::core_identity(&config)?;
            println!("{}", identity.fingerprint);
            Ok(())
        }
        Commands::Install(_)
        | Commands::Uninstall(_)
        | Commands::Status(_)
        | Commands::ServeWithConfig { .. }
        | Commands::ServiceRun { .. }
        | Commands::Version => unreachable!("handled before configuration loading"),
    }
}
