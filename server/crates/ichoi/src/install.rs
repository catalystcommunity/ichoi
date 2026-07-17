//! Native satellite installation planning and application.
//!
//! Planning is deliberately pure enough to test every platform on Linux. Applying a plan is a
//! small filesystem/command boundary used only after all inputs and privileges are validated.

use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Scope {
    System,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Macos,
    Windows,
}

impl Platform {
    pub fn current() -> anyhow::Result<Self> {
        match std::env::consts::OS {
            "linux" => Ok(Self::Linux),
            "macos" => Ok(Self::Macos),
            "windows" => Ok(Self::Windows),
            os => anyhow::bail!("native satellite installation is not supported on {os}"),
        }
    }

    pub fn default_scope(self) -> Scope {
        match self {
            Self::Linux => Scope::System,
            Self::Macos | Self::Windows => Scope::User,
        }
    }
}

#[derive(Debug)]
pub struct InstallOptions {
    pub core_addr: String,
    pub core_keys: Vec<String>,
    pub node_token: String,
    pub scope: Option<Scope>,
    pub config_path: Option<PathBuf>,
    pub install_dir: Option<PathBuf>,
    pub start: bool,
    pub enable: bool,
    pub replace: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct PlannedFile {
    pub path: PathBuf,
    pub contents: FileContents,
    pub private: bool,
    pub executable: bool,
}

#[derive(Debug, Clone)]
pub enum FileContents {
    Bytes(Vec<u8>),
    Copy(PathBuf),
}

#[derive(Debug, Clone)]
pub struct InstallPlan {
    pub platform: Platform,
    pub scope: Scope,
    pub binary_path: PathBuf,
    pub config_path: PathBuf,
    pub files: Vec<PlannedFile>,
    pub commands: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
struct HostPaths {
    home: PathBuf,
    config_home: Option<PathBuf>,
    data_home: Option<PathBuf>,
    program_data: Option<PathBuf>,
    program_files: Option<PathBuf>,
}

impl HostPaths {
    fn current() -> anyhow::Result<Self> {
        let home = env_path("HOME")
            .or_else(|| env_path("USERPROFILE"))
            .ok_or_else(|| anyhow::anyhow!("cannot determine the current user's home directory"))?;
        Ok(Self {
            home,
            config_home: env_path("XDG_CONFIG_HOME").or_else(|| env_path("APPDATA")),
            data_home: env_path("LOCALAPPDATA"),
            program_data: env_path("PROGRAMDATA"),
            program_files: env_path("ProgramFiles"),
        })
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

pub fn read_node_token(path: Option<&Path>, stdin: bool) -> anyhow::Result<String> {
    let token = if let Some(path) = path {
        std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading node token {}: {e}", path.display()))?
    } else if stdin {
        let mut value = String::new();
        std::io::stdin().read_to_string(&mut value)?;
        value
    } else {
        std::env::var("ICHOI_NODE_TOKEN").map_err(|_| {
            anyhow::anyhow!(
                "provide the node token with --node-token-file, --node-token-stdin, or ICHOI_NODE_TOKEN"
            )
        })?
    };
    let token = token.trim().to_string();
    if token.is_empty() {
        anyhow::bail!("node token is empty");
    }
    Ok(token)
}

pub fn plan(options: &InstallOptions) -> anyhow::Result<InstallPlan> {
    let platform = Platform::current()?;
    plan_for(
        platform,
        &HostPaths::current()?,
        options,
        std::env::current_exe()?,
    )
}

fn plan_for(
    platform: Platform,
    host: &HostPaths,
    options: &InstallOptions,
    source_binary: PathBuf,
) -> anyhow::Result<InstallPlan> {
    validate_options(options)?;
    let scope = options.scope.unwrap_or_else(|| platform.default_scope());
    if platform == Platform::Macos && scope == Scope::System {
        anyhow::bail!("macOS satellites use a per-user LaunchAgent; --scope system is unsupported");
    }

    let (default_bin_dir, default_config, service_path) = layout(platform, scope, host)?;
    let bin_dir = options.install_dir.clone().unwrap_or(default_bin_dir);
    let binary_name = if platform == Platform::Windows {
        "ichoi.exe"
    } else {
        "ichoi"
    };
    let binary_path = bin_dir.join(binary_name);
    let config_path = options.config_path.clone().unwrap_or(default_config);
    let config = satellite_config(options);
    let (service_contents, commands) = service_definition(
        platform,
        scope,
        &binary_path,
        &config_path,
        service_path.as_deref(),
        options.enable,
        options.start,
    )?;

    let mut files = vec![
        PlannedFile {
            path: binary_path.clone(),
            contents: FileContents::Copy(source_binary),
            private: false,
            executable: true,
        },
        PlannedFile {
            path: config_path.clone(),
            contents: FileContents::Bytes(config.into_bytes()),
            private: true,
            executable: false,
        },
    ];
    if let (Some(path), Some(contents)) = (service_path, service_contents) {
        files.push(PlannedFile {
            path,
            contents: FileContents::Bytes(contents.into_bytes()),
            private: false,
            executable: false,
        });
    }

    Ok(InstallPlan {
        platform,
        scope,
        binary_path,
        config_path,
        files,
        commands,
    })
}

fn validate_options(options: &InstallOptions) -> anyhow::Result<()> {
    if options.core_addr.trim().is_empty() {
        anyhow::bail!("--core must not be empty");
    }
    crate::tls::client_config(&options.core_keys)?;
    if options.node_token.trim().is_empty() {
        anyhow::bail!("node token must not be empty");
    }
    Ok(())
}

fn layout(
    platform: Platform,
    scope: Scope,
    host: &HostPaths,
) -> anyhow::Result<(PathBuf, PathBuf, Option<PathBuf>)> {
    Ok(match (platform, scope) {
        (Platform::Linux, Scope::System) => (
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/etc/ichoi/ichoi.toml"),
            Some(PathBuf::from("/etc/systemd/system/ichoi-satellite.service")),
        ),
        (Platform::Linux, Scope::User) => {
            let config = host
                .config_home
                .clone()
                .unwrap_or_else(|| host.home.join(".config"));
            (
                host.home.join(".local/bin"),
                config.join("ichoi/ichoi.toml"),
                Some(config.join("systemd/user/ichoi-satellite.service")),
            )
        }
        (Platform::Macos, Scope::User) => {
            let support = host.home.join("Library/Application Support/Ichoi");
            (
                support.clone(),
                support.join("ichoi.toml"),
                Some(
                    host.home
                        .join("Library/LaunchAgents/community.catalyst.ichoi.satellite.plist"),
                ),
            )
        }
        (Platform::Windows, Scope::User) => {
            let data = host
                .data_home
                .clone()
                .unwrap_or_else(|| host.home.join("AppData/Local"));
            let config = host
                .config_home
                .clone()
                .unwrap_or_else(|| host.home.join("AppData/Roaming"));
            (data.join("Ichoi"), config.join("Ichoi/ichoi.toml"), None)
        }
        (Platform::Windows, Scope::System) => (
            host.program_files
                .clone()
                .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
                .join("Ichoi"),
            host.program_data
                .clone()
                .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
                .join("Ichoi/ichoi.toml"),
            None,
        ),
        (Platform::Macos, Scope::System) => unreachable!(),
    })
}

fn satellite_config(options: &InstallOptions) -> String {
    let mut out = String::new();
    out.push_str("role = \"satellite\"\n");
    let _ = writeln!(out, "core_addr = {}", toml_string(options.core_addr.trim()));
    out.push_str("core_keys = [\n");
    for key in &options.core_keys {
        let _ = writeln!(out, "  {},", toml_string(key));
    }
    out.push_str("]\n");
    let _ = writeln!(
        out,
        "node_token = {}",
        toml_string(options.node_token.trim())
    );
    out.push_str("log = \"info\"\n");
    out
}

fn toml_string(value: &str) -> String {
    // JSON string escaping is also valid TOML basic-string escaping.
    serde_json::to_string(value).expect("serializing a string cannot fail")
}

fn quoted(path: &Path) -> String {
    format!("\"{}\"", path.display())
}

fn service_definition(
    platform: Platform,
    scope: Scope,
    binary: &Path,
    config: &Path,
    service_path: Option<&Path>,
    enable: bool,
    start: bool,
) -> anyhow::Result<(Option<String>, Vec<Vec<String>>)> {
    let mut commands = Vec::new();
    let contents = match platform {
        Platform::Linux => {
            let unit = format!(
                "[Unit]\nDescription=Ichoi satellite audio node\nWants=network-online.target\nAfter=network-online.target sound.target\n\n[Service]\nType=simple\nExecStart={} serve\nEnvironment=ICHOI_CONFIG={}\nRestart=always\nRestartSec=3\n\n[Install]\nWantedBy=multi-user.target\n",
                binary.display(),
                config.display()
            );
            let mut systemctl = vec!["systemctl".to_string()];
            if scope == Scope::User {
                systemctl.push("--user".to_string());
            }
            let mut reload = systemctl.clone();
            reload.push("daemon-reload".into());
            commands.push(reload);
            if enable {
                let mut command = systemctl.clone();
                command.extend(["enable".into(), "ichoi-satellite.service".into()]);
                commands.push(command);
            }
            if start {
                let mut command = systemctl;
                command.extend(["restart".into(), "ichoi-satellite.service".into()]);
                commands.push(command);
            }
            Some(unit)
        }
        Platform::Macos => {
            let plist = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>Label</key><string>community.catalyst.ichoi.satellite</string>\n<key>ProgramArguments</key><array><string>{}</string><string>serve</string></array>\n<key>EnvironmentVariables</key><dict><key>ICHOI_CONFIG</key><string>{}</string></dict>\n<key>RunAtLoad</key><true/>\n<key>KeepAlive</key><true/>\n<key>ProcessType</key><string>Interactive</string>\n</dict></plist>\n",
                xml_escape(&binary.display().to_string()),
                xml_escape(&config.display().to_string())
            );
            if start {
                let uid = command_output("id", &["-u"]).unwrap_or_else(|| "$(id -u)".into());
                commands.push(vec![
                    "launchctl".into(),
                    "bootstrap".into(),
                    format!("gui/{uid}"),
                    service_path
                        .ok_or_else(|| anyhow::anyhow!("missing LaunchAgent path"))?
                        .display()
                        .to_string(),
                ]);
            }
            Some(plist)
        }
        Platform::Windows if scope == Scope::User => {
            commands.push(windows_private_config_command(config, false));
            if enable || start {
                commands.push(vec![
                    "schtasks.exe".into(),
                    "/Create".into(),
                    "/F".into(),
                    "/SC".into(),
                    "ONLOGON".into(),
                    "/TN".into(),
                    "Ichoi Satellite".into(),
                    "/TR".into(),
                    format!("{} serve-with-config {}", quoted(binary), quoted(config)),
                ]);
                if start {
                    commands.push(vec![
                        "schtasks.exe".into(),
                        "/Run".into(),
                        "/TN".into(),
                        "Ichoi Satellite".into(),
                    ]);
                }
            }
            None
        }
        Platform::Windows => {
            commands.push(windows_private_config_command(config, true));
            commands.push(vec![
                "sc.exe".into(),
                "create".into(),
                "IchoiSatellite".into(),
                "start=".into(),
                if enable {
                    "auto".into()
                } else {
                    "demand".into()
                },
                "DisplayName=".into(),
                "Ichoi Satellite".into(),
                "binPath=".into(),
                format!("{} service-run {}", quoted(binary), quoted(config)),
            ]);
            if start {
                commands.push(vec![
                    "sc.exe".into(),
                    "start".into(),
                    "IchoiSatellite".into(),
                ]);
            }
            None
        }
    };
    Ok((contents, commands))
}

fn windows_private_config_command(config: &Path, system: bool) -> Vec<String> {
    let mut command = vec![
        "icacls.exe".into(),
        config.display().to_string(),
        "/inheritance:r".into(),
        "/grant:r".into(),
    ];
    if system {
        command.extend(["SYSTEM:F".into(), "*S-1-5-32-544:F".into()]);
    } else {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "CURRENT_USER".into());
        command.push(format!("{user}:F"));
    }
    command
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn describe(plan: &InstallPlan) -> String {
    let mut out = format!(
        "install {:?} satellite ({:?} scope)\n",
        plan.platform, plan.scope
    );
    for file in &plan.files {
        let kind = if file.private {
            "private config"
        } else if file.executable {
            "executable"
        } else {
            "service definition"
        };
        let _ = writeln!(out, "  write {kind}: {}", file.path.display());
    }
    for command in &plan.commands {
        let _ = writeln!(out, "  run: {}", command.join(" "));
    }
    out
}

pub fn apply(plan: &InstallPlan, replace: bool) -> anyhow::Result<()> {
    ensure_privileges(plan.platform, plan.scope)?;
    for file in &plan.files {
        write_planned_file(file, replace)?;
    }
    for command in &plan.commands {
        run_command(command)?;
    }
    Ok(())
}

pub fn uninstall(
    requested_scope: Option<Scope>,
    config_override: Option<PathBuf>,
    install_dir_override: Option<PathBuf>,
    keep_config: bool,
    keep_binary: bool,
    dry_run: bool,
) -> anyhow::Result<()> {
    let platform = Platform::current()?;
    let scope = requested_scope.unwrap_or_else(|| platform.default_scope());
    if platform == Platform::Macos && scope == Scope::System {
        anyhow::bail!("macOS satellites do not support system scope");
    }
    let host = HostPaths::current()?;
    let (default_bin_dir, default_config, service_path) = layout(platform, scope, &host)?;
    let binary =
        install_dir_override
            .unwrap_or(default_bin_dir)
            .join(if platform == Platform::Windows {
                "ichoi.exe"
            } else {
                "ichoi"
            });
    let config = config_override.unwrap_or(default_config);
    let commands = stop_commands(platform, scope, service_path.as_deref())?;
    println!("uninstall {platform:?} satellite ({scope:?} scope)");
    for command in &commands {
        println!("  run: {}", command.join(" "));
    }
    if let Some(path) = &service_path {
        println!("  remove service definition: {}", path.display());
    }
    if !keep_config {
        println!("  remove private config: {}", config.display());
    }
    if !keep_binary {
        println!("  remove executable: {}", binary.display());
    }
    if dry_run {
        return Ok(());
    }
    ensure_privileges(platform, scope)?;
    for command in &commands {
        run_command_best_effort(command);
    }
    if let Some(path) = service_path {
        remove_if_present(&path)?;
    }
    if !keep_config {
        remove_if_present(&config)?;
    }
    if !keep_binary {
        remove_if_present(&binary)?;
    }
    if platform == Platform::Linux {
        let mut command = vec!["systemctl".to_string()];
        if scope == Scope::User {
            command.push("--user".into());
        }
        command.push("daemon-reload".into());
        run_command_best_effort(&command);
    }
    println!("Ichoi satellite uninstalled");
    Ok(())
}

pub fn status(requested_scope: Option<Scope>) -> anyhow::Result<()> {
    let platform = Platform::current()?;
    let scope = requested_scope.unwrap_or_else(|| platform.default_scope());
    if platform == Platform::Macos && scope == Scope::System {
        anyhow::bail!("macOS satellites do not support system scope");
    }
    let host = HostPaths::current()?;
    let (_, _, service_path) = layout(platform, scope, &host)?;
    let command = status_command(platform, scope, service_path.as_deref())?;
    run_command(&command)
}

fn stop_commands(
    platform: Platform,
    scope: Scope,
    service_path: Option<&Path>,
) -> anyhow::Result<Vec<Vec<String>>> {
    Ok(match (platform, scope) {
        (Platform::Linux, _) => {
            let mut command = vec!["systemctl".to_string()];
            if scope == Scope::User {
                command.push("--user".into());
            }
            command.extend([
                "disable".into(),
                "--now".into(),
                "ichoi-satellite.service".into(),
            ]);
            vec![command]
        }
        (Platform::Macos, Scope::User) => vec![vec![
            "launchctl".into(),
            "bootout".into(),
            format!(
                "gui/{}",
                command_output("id", &["-u"]).unwrap_or_else(|| "$(id -u)".into())
            ),
            service_path
                .ok_or_else(|| anyhow::anyhow!("missing LaunchAgent path"))?
                .display()
                .to_string(),
        ]],
        (Platform::Windows, Scope::User) => vec![vec![
            "schtasks.exe".into(),
            "/Delete".into(),
            "/F".into(),
            "/TN".into(),
            "Ichoi Satellite".into(),
        ]],
        (Platform::Windows, Scope::System) => vec![
            vec!["sc.exe".into(), "stop".into(), "IchoiSatellite".into()],
            vec!["sc.exe".into(), "delete".into(), "IchoiSatellite".into()],
        ],
        (Platform::Macos, Scope::System) => unreachable!(),
    })
}

fn status_command(
    platform: Platform,
    scope: Scope,
    _service_path: Option<&Path>,
) -> anyhow::Result<Vec<String>> {
    Ok(match (platform, scope) {
        (Platform::Linux, _) => {
            let mut command = vec!["systemctl".to_string()];
            if scope == Scope::User {
                command.push("--user".into());
            }
            command.extend(["status".into(), "ichoi-satellite.service".into()]);
            command
        }
        (Platform::Macos, Scope::User) => vec![
            "launchctl".into(),
            "print".into(),
            format!(
                "gui/{}/community.catalyst.ichoi.satellite",
                command_output("id", &["-u"]).unwrap_or_else(|| "$(id -u)".into())
            ),
        ],
        (Platform::Windows, Scope::User) => vec![
            "schtasks.exe".into(),
            "/Query".into(),
            "/TN".into(),
            "Ichoi Satellite".into(),
            "/V".into(),
        ],
        (Platform::Windows, Scope::System) => {
            vec!["sc.exe".into(), "query".into(), "IchoiSatellite".into()]
        }
        (Platform::Macos, Scope::System) => unreachable!(),
    })
}

fn remove_if_present(path: &Path) -> anyhow::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn run_command_best_effort(command: &[String]) {
    if let Err(error) = run_command(command) {
        eprintln!("warning: {error}");
    }
}

fn ensure_privileges(platform: Platform, scope: Scope) -> anyhow::Result<()> {
    if scope == Scope::User {
        return Ok(());
    }
    match platform {
        Platform::Linux | Platform::Macos => {
            if command_output("id", &["-u"]).as_deref() != Some("0") {
                anyhow::bail!("system installation requires root; rerun this command with sudo");
            }
        }
        Platform::Windows => {
            let ok = Command::new("net.exe")
                .arg("session")
                .output()
                .is_ok_and(|o| o.status.success());
            if !ok {
                anyhow::bail!("system installation requires an Administrator terminal");
            }
        }
    }
    Ok(())
}

fn write_planned_file(file: &PlannedFile, replace: bool) -> anyhow::Result<()> {
    if file.path.exists() && !replace {
        anyhow::bail!(
            "{} already exists; use --replace to overwrite the existing installation",
            file.path.display()
        );
    }
    let parent = file
        .path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("{} has no parent directory", file.path.display()))?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.ichoi-install-{}",
        file.path
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("file"),
        std::process::id()
    ));
    match &file.contents {
        FileContents::Bytes(bytes) => std::fs::write(&temporary, bytes)?,
        FileContents::Copy(source) => {
            std::fs::copy(source, &temporary).map_err(|e| {
                anyhow::anyhow!(
                    "copying executable {} to {}: {e}",
                    source.display(),
                    temporary.display()
                )
            })?;
        }
    }
    set_permissions(&temporary, file.private, file.executable)?;
    if replace && file.path.exists() {
        std::fs::remove_file(&file.path)?;
    }
    std::fs::rename(&temporary, &file.path)?;
    Ok(())
}

#[cfg(unix)]
fn set_permissions(path: &Path, private: bool, executable: bool) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = if private {
        0o600
    } else if executable {
        0o755
    } else {
        0o644
    };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_permissions(_path: &Path, _private: bool, _executable: bool) -> anyhow::Result<()> {
    Ok(())
}

fn run_command(command: &[String]) -> anyhow::Result<()> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| anyhow::anyhow!("empty installation command"))?;
    let status = Command::new(program).args(args).status()?;
    if !status.success() {
        anyhow::bail!(
            "installation command failed ({status}): {}",
            command.join(" ")
        );
    }
    Ok(())
}

pub fn serve_with_config(path: &Path) -> anyhow::Result<()> {
    std::env::set_var("ICHOI_CONFIG", path);
    let config = crate::config::Config::load()?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(crate::app::serve(config))
}

#[cfg(target_os = "windows")]
pub fn run_windows_service(config_path: PathBuf) -> anyhow::Result<()> {
    windows_daemon::run(config_path)
}

#[cfg(target_os = "windows")]
mod windows_daemon {
    use super::*;
    use std::sync::{mpsc, OnceLock};
    use std::time::Duration;
    use windows_service::{
        define_windows_service,
        service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
            ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
    };

    static CONFIG_PATH: OnceLock<PathBuf> = OnceLock::new();
    define_windows_service!(ffi_service_main, service_main);

    pub fn run(config_path: PathBuf) -> anyhow::Result<()> {
        CONFIG_PATH.set(config_path).map_err(|_| {
            anyhow::anyhow!("Windows service configuration was already initialized")
        })?;
        service_dispatcher::start("IchoiSatellite", ffi_service_main)?;
        Ok(())
    }

    fn service_main(_arguments: Vec<std::ffi::OsString>) {
        let _ = run_inner();
    }

    fn run_inner() -> anyhow::Result<()> {
        let config_path = CONFIG_PATH
            .get()
            .ok_or_else(|| anyhow::anyhow!("Windows service configuration is missing"))?
            .clone();
        let (stop_tx, stop_rx) = mpsc::channel();
        let handler = move |control| match control {
            ServiceControl::Stop => {
                let _ = stop_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        };
        let status = service_control_handler::register("IchoiSatellite", handler)?;
        let report = |state, accepted| ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: accepted,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        };
        status.set_service_status(report(ServiceState::Running, ServiceControlAccept::STOP))?;
        std::env::set_var("ICHOI_CONFIG", config_path);
        let config = crate::config::Config::load()?;
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async move {
            tokio::select! {
                _ = crate::app::serve(config) => {},
                _ = tokio::task::spawn_blocking(move || stop_rx.recv()) => {},
            }
        });
        status.set_service_status(report(ServiceState::Stopped, ServiceControlAccept::empty()))?;
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
pub fn run_windows_service(_config_path: PathBuf) -> anyhow::Result<()> {
    anyhow::bail!("service-run is available only on Windows")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> HostPaths {
        HostPaths {
            home: PathBuf::from("/home/test"),
            config_home: None,
            data_home: Some(PathBuf::from(r"C:\Users\test\AppData\Local")),
            program_data: Some(PathBuf::from(r"C:\ProgramData")),
            program_files: Some(PathBuf::from(r"C:\Program Files")),
        }
    }

    fn options(scope: Scope) -> InstallOptions {
        InstallOptions {
            core_addr: "192.0.2.10:4043".into(),
            core_keys: vec![format!("sha256:{}", "11".repeat(32))],
            node_token: "secret".into(),
            scope: Some(scope),
            config_path: None,
            install_dir: None,
            start: true,
            enable: true,
            replace: false,
            dry_run: true,
        }
    }

    #[test]
    fn linux_system_plan_uses_etc_and_systemd() {
        let plan = plan_for(
            Platform::Linux,
            &host(),
            &options(Scope::System),
            PathBuf::from("/tmp/ichoi"),
        )
        .unwrap();
        assert_eq!(plan.config_path, PathBuf::from("/etc/ichoi/ichoi.toml"));
        assert!(plan
            .files
            .iter()
            .any(|f| f.path == Path::new("/etc/systemd/system/ichoi-satellite.service")));
        assert!(plan
            .commands
            .iter()
            .any(|c| c == &["systemctl", "enable", "ichoi-satellite.service"]));
    }

    #[test]
    fn macos_is_a_user_launch_agent() {
        let plan = plan_for(
            Platform::Macos,
            &host(),
            &options(Scope::User),
            PathBuf::from("/tmp/ichoi"),
        )
        .unwrap();
        assert!(plan
            .config_path
            .to_string_lossy()
            .contains("Library/Application Support/Ichoi"));
        assert!(plan
            .files
            .iter()
            .any(|f| f.path.to_string_lossy().contains("Library/LaunchAgents")));
        assert!(plan_for(
            Platform::Macos,
            &host(),
            &options(Scope::System),
            PathBuf::from("/tmp/ichoi")
        )
        .is_err());
    }

    #[test]
    fn windows_supports_user_task_and_system_service() {
        let user = plan_for(
            Platform::Windows,
            &host(),
            &options(Scope::User),
            PathBuf::from("ichoi.exe"),
        )
        .unwrap();
        assert!(user.commands.iter().flatten().any(|v| v == "ONLOGON"));
        let system = plan_for(
            Platform::Windows,
            &host(),
            &options(Scope::System),
            PathBuf::from("ichoi.exe"),
        )
        .unwrap();
        assert!(system
            .commands
            .iter()
            .flatten()
            .any(|v| v == "IchoiSatellite"));
    }

    #[test]
    fn config_quotes_values_and_keeps_token_private() {
        let plan = plan_for(
            Platform::Linux,
            &host(),
            &options(Scope::User),
            PathBuf::from("/tmp/ichoi"),
        )
        .unwrap();
        let config = plan
            .files
            .iter()
            .find(|f| f.path == plan.config_path)
            .unwrap();
        assert!(config.private);
        let FileContents::Bytes(bytes) = &config.contents else {
            panic!()
        };
        let text = String::from_utf8_lossy(bytes);
        assert!(text.contains("node_token = \"secret\""));
        assert!(toml::from_str::<toml::Value>(&text).is_ok());
        assert!(!describe(&plan).contains("secret"));
    }

    #[test]
    fn lifecycle_commands_match_each_platform_scope() {
        let linux = status_command(Platform::Linux, Scope::User, None).unwrap();
        assert_eq!(
            linux,
            ["systemctl", "--user", "status", "ichoi-satellite.service"]
        );
        let windows = stop_commands(Platform::Windows, Scope::System, None).unwrap();
        assert!(windows.iter().flatten().any(|part| part == "delete"));
        let mac_path = Path::new("/Users/test/Library/LaunchAgents/ichoi.plist");
        let mac = stop_commands(Platform::Macos, Scope::User, Some(mac_path)).unwrap();
        assert!(mac.iter().flatten().any(|part| part == "bootout"));
    }
}
