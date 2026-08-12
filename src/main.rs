use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use std::{error::Error, io, process::Stdio, time::Duration};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

#[cfg(target_os = "windows")]
fn build_command(program: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.arg("/C").arg(program);
    for arg in args {
        cmd.arg(arg);
    }
    cmd
}

#[cfg(not(target_os = "windows"))]
fn build_command(program: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new(program);
    for arg in args {
        cmd.arg(arg);
    }
    cmd
}

#[derive(Clone)]
struct Package {
    name: String,
    version: String,
    latest_version: Option<String>,
    manager: &'static str,
    selected: bool,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum ManagerType {
    Winget,
    Npm,
    Pnpm,
    Pip,
    Uv,
    Cargo,
    Choco,
    Scoop,
    Gem,
    Dotnet,
    Brew,
    Apt,
    Pacman,
}

impl ManagerType {
    fn name(&self) -> &'static str {
        match self {
            Self::Winget => "winget",
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Pip => "pip",
            Self::Uv => "uv",
            Self::Cargo => "cargo",
            Self::Choco => "choco",
            Self::Scoop => "scoop",
            Self::Gem => "gem",
            Self::Dotnet => "dotnet",
            Self::Brew => "brew",
            Self::Apt => "apt",
            Self::Pacman => "pacman",
        }
    }

    fn requires_sudo(&self) -> bool {
        matches!(self, Self::Apt | Self::Pacman)
    }

    async fn list_packages(&self) -> Vec<Package> {
        match self {
            Self::Npm => list_npm().await,
            Self::Pnpm => list_pnpm().await,
            Self::Pip => list_pip().await,
            Self::Uv => list_uv().await,
            Self::Cargo => list_cargo().await,
            Self::Gem => list_gem().await,
            Self::Dotnet => list_dotnet().await,
            #[cfg(target_os = "windows")]
            Self::Winget => list_winget().await,
            #[cfg(target_os = "windows")]
            Self::Choco => list_choco().await,
            #[cfg(target_os = "windows")]
            Self::Scoop => list_scoop().await,
            #[cfg(not(target_os = "windows"))]
            Self::Brew => list_brew().await,
            #[cfg(not(target_os = "windows"))]
            Self::Apt => list_apt().await,
            #[cfg(not(target_os = "windows"))]
            Self::Pacman => list_pacman().await,
            _ => vec![],
        }
    }

    async fn list_outdated(&self) -> Vec<(String, String)> {
        match self {
            Self::Npm => outdated_npm().await,
            Self::Pnpm => outdated_pnpm().await,
            Self::Pip => outdated_pip().await,
            Self::Gem => outdated_gem().await,
            #[cfg(target_os = "windows")]
            Self::Winget => outdated_winget().await,
            #[cfg(target_os = "windows")]
            Self::Choco => outdated_choco().await,
            #[cfg(target_os = "windows")]
            Self::Scoop => outdated_scoop().await,
            #[cfg(not(target_os = "windows"))]
            Self::Brew => outdated_brew().await,
            #[cfg(not(target_os = "windows"))]
            Self::Apt => outdated_apt().await,
            #[cfg(not(target_os = "windows"))]
            Self::Pacman => outdated_pacman().await,
            _ => vec![],
        }
    }

    fn update_command(&self, pkg: &str) -> Command {
        match self {
            Self::Npm => build_command("npm", &["update", "-g", pkg]),
            Self::Pnpm => build_command("pnpm", &["update", "-g", pkg]),
            Self::Pip => build_command("pip", &["install", "--upgrade", pkg]),
            Self::Uv => build_command("uv", &["tool", "upgrade", pkg]),
            Self::Cargo => build_command("cargo", &["install", pkg]),
            Self::Gem => build_command("gem", &["update", pkg]),
            Self::Dotnet => build_command("dotnet", &["tool", "update", "-g", pkg]),
            Self::Winget => build_command("winget", &["upgrade", pkg]),
            Self::Choco => build_command("choco", &["upgrade", pkg, "-y"]),
            Self::Scoop => build_command("scoop", &["update", pkg]),
            Self::Brew => build_command("brew", &["upgrade", pkg]),
            Self::Apt => build_command(
                "sudo",
                &["-n", "apt-get", "--only-upgrade", "install", "-y", pkg],
            ),
            Self::Pacman => build_command("sudo", &["-n", "pacman", "-S", "--noconfirm", pkg]),
        }
    }

    fn delete_command(&self, pkg: &str) -> Command {
        match self {
            Self::Npm => build_command("npm", &["uninstall", "-g", pkg]),
            Self::Pnpm => build_command("pnpm", &["remove", "-g", pkg]),
            Self::Pip => build_command("pip", &["uninstall", "-y", pkg]),
            Self::Uv => build_command("uv", &["tool", "uninstall", pkg]),
            Self::Cargo => build_command("cargo", &["uninstall", pkg]),
            Self::Gem => build_command("gem", &["uninstall", pkg]),
            Self::Dotnet => build_command("dotnet", &["tool", "uninstall", "-g", pkg]),
            Self::Winget => build_command("winget", &["uninstall", pkg]),
            Self::Choco => build_command("choco", &["uninstall", pkg, "-y"]),
            Self::Scoop => build_command("scoop", &["uninstall", pkg]),
            Self::Brew => build_command("brew", &["uninstall", pkg]),
            Self::Apt => build_command("sudo", &["-n", "apt-get", "remove", "-y", pkg]),
            Self::Pacman => build_command("sudo", &["-n", "pacman", "-R", "--noconfirm", pkg]),
        }
    }
}

fn get_managers() -> Vec<ManagerType> {
    let mut m = vec![
        ManagerType::Npm,
        ManagerType::Pnpm,
        ManagerType::Pip,
        ManagerType::Uv,
        ManagerType::Cargo,
        ManagerType::Gem,
        ManagerType::Dotnet,
    ];

    #[cfg(target_os = "windows")]
    {
        m.push(ManagerType::Winget);
        m.push(ManagerType::Choco);
        m.push(ManagerType::Scoop);
    }

    #[cfg(not(target_os = "windows"))]
    {
        m.push(ManagerType::Brew);
        m.push(ManagerType::Apt);
        m.push(ManagerType::Pacman);
    }
    m
}

// =============================================================================
// Async Parsers
// =============================================================================

async fn outdated_gem() -> Vec<(String, String)> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("gem", &["outdated"]).output().await {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if let Some(idx) = line.find(" (") {
                let name = &line[..idx];
                if let Some(less_idx) = line.find("< ") {
                    let latest_part = &line[less_idx + 2..line.len() - 1];
                    let latest = latest_part.split(", ").next().unwrap_or(latest_part);
                    pkgs.push((name.to_string(), latest.to_string()));
                }
            }
        }
    }
    pkgs
}

#[cfg(not(target_os = "windows"))]
async fn outdated_brew() -> Vec<(String, String)> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("brew", &["outdated"]).output().await {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if !parts.is_empty() {
                if let Some(idx) = parts.iter().position(|&x| x == "<") {
                    if idx + 1 < parts.len() {
                        pkgs.push((parts[0].to_string(), parts[idx + 1].to_string()));
                    }
                }
            }
        }
    }
    pkgs
}

#[cfg(not(target_os = "windows"))]
async fn outdated_apt() -> Vec<(String, String)> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("apt", &["list", "--upgradable"]).output().await {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines().skip(1) {
            if let Some(idx) = line.find('/') {
                let name = &line[..idx];
                if let Some(space_idx) = line.find(' ') {
                    let version_part = line[space_idx..].trim();
                    let latest = version_part.split_whitespace().next().unwrap_or("unknown");
                    pkgs.push((name.to_string(), latest.to_string()));
                }
            }
        }
    }
    pkgs
}

#[cfg(not(target_os = "windows"))]
async fn outdated_pacman() -> Vec<(String, String)> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("pacman", &["-Qu"]).output().await {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 && parts[2] == "->" {
                pkgs.push((parts[0].to_string(), parts[3].to_string()));
            }
        }
    }
    pkgs
}

async fn outdated_npm() -> Vec<(String, String)> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("npm", &["outdated", "-g", "--json"])
        .output()
        .await
    {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(obj) = json.as_object() {
                for (k, v) in obj {
                    if let Some(latest) = v["latest"].as_str() {
                        pkgs.push((k.clone(), latest.to_string()));
                    }
                }
            }
        }
    }
    pkgs
}

async fn outdated_pnpm() -> Vec<(String, String)> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("pnpm", &["outdated", "-g", "--json"])
        .output()
        .await
    {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(obj) = json.as_object() {
                for (k, v) in obj {
                    if let Some(latest) = v["latest"].as_str() {
                        pkgs.push((k.clone(), latest.to_string()));
                    }
                }
            }
        }
    }
    pkgs
}

async fn outdated_pip() -> Vec<(String, String)> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("pip", &["list", "--outdated", "--format=json"])
        .output()
        .await
    {
        if let Ok(json) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
            for v in json {
                if let (Some(n), Some(latest)) = (v["name"].as_str(), v["latest_version"].as_str())
                {
                    pkgs.push((n.to_string(), latest.to_string()));
                }
            }
        }
    }
    pkgs
}

#[cfg(target_os = "windows")]
async fn outdated_winget() -> Vec<(String, String)> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("winget", &["upgrade", "--accept-source-agreements"]).output().await {
        let text = String::from_utf8_lossy(&out.stdout);
        let mut lines = text.lines().skip_while(|l| !l.starts_with("Name"));
        if let Some(header) = lines.next() {
            let id_idx = header.find("Id").unwrap_or(0);
            let avail_idx = header.find("Available").unwrap_or(0);
            for line in lines.skip(1) {
                if line.len() > avail_idx {
                    let id = line[id_idx..]
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_string();
                    let available = line[avail_idx..]
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_string();
                    if !id.is_empty() && !available.is_empty() && !id.contains(" ") && id != "Id" {
                        pkgs.push((id, available));
                    }
                }
            }
        }
    }
    pkgs
}

#[cfg(target_os = "windows")]
async fn outdated_choco() -> Vec<(String, String)> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("choco", &["outdated"]).output().await {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 3 {
                pkgs.push((parts[0].to_string(), parts[2].to_string()));
            }
        }
    }
    pkgs
}

#[cfg(target_os = "windows")]
async fn outdated_scoop() -> Vec<(String, String)> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("scoop", &["status"]).output().await {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines().skip_while(|l| !l.starts_with("----")).skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                pkgs.push((parts[0].to_string(), parts[2].to_string()));
            }
        }
    }
    pkgs
}

#[cfg(target_os = "windows")]
async fn list_winget() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("winget", &["list", "--accept-source-agreements"]).output().await {
        let text = String::from_utf8_lossy(&out.stdout);
        let mut lines = text.lines().skip_while(|l| !l.starts_with("Name"));
        if let Some(header) = lines.next() {
            let id_idx = header.find("Id").unwrap_or(0);
            let ver_idx = header.find("Version").unwrap_or(0);
            for line in lines.skip(1) {
                if line.len() > ver_idx {
                    let id = line[id_idx..ver_idx].trim().to_string();
                    let ver = line[ver_idx..]
                        .split_whitespace()
                        .next()
                        .unwrap_or("unknown")
                        .to_string();
                    if !id.is_empty() && !id.contains(" ") {
                        pkgs.push(Package {
                            name: id,
                            version: ver,
                            latest_version: None,
                            manager: "winget",
                            selected: false,
                        });
                    }
                }
            }
        }
    }
    pkgs
}

async fn list_npm() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("npm", &["list", "-g", "--depth=0", "--json"])
        .output()
        .await
    {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(deps) = json["dependencies"].as_object() {
                for (k, v) in deps {
                    let ver = v["version"].as_str().unwrap_or("unknown");
                    pkgs.push(Package {
                        name: k.clone(),
                        version: ver.to_string(),
                        latest_version: None,
                        manager: "npm",
                        selected: false,
                    });
                }
            }
        }
    }
    pkgs
}

async fn list_pnpm() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("pnpm", &["ls", "-g", "--depth=0", "--json"])
        .output()
        .await
    {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(arr) = json.as_array() {
                if let Some(first) = arr.first() {
                    if let Some(deps) = first["dependencies"].as_object() {
                        for (k, v) in deps {
                            let ver = v["version"].as_str().unwrap_or("unknown");
                            pkgs.push(Package {
                                name: k.clone(),
                                version: ver.to_string(),
                                latest_version: None,
                                manager: "pnpm",
                                selected: false,
                            });
                        }
                    }
                }
            }
        }
    }
    pkgs
}

async fn list_pip() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("pip", &["list", "--format=json"])
        .output()
        .await
    {
        if let Ok(json) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
            for v in json {
                if let (Some(n), Some(ver)) = (v["name"].as_str(), v["version"].as_str()) {
                    pkgs.push(Package {
                        name: n.to_string(),
                        version: ver.to_string(),
                        latest_version: None,
                        manager: "pip",
                        selected: false,
                    });
                }
            }
        }
    }
    pkgs
}

async fn list_uv() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("uv", &["tool", "list"]).output().await {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                pkgs.push(Package {
                    name: parts[0].to_string(),
                    version: parts[1].trim_start_matches('v').to_string(),
                    latest_version: None,
                    manager: "uv",
                    selected: false,
                });
            }
        }
    }
    pkgs
}

async fn list_cargo() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("cargo", &["install", "--list"])
        .output()
        .await
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.ends_with(':') && !line.starts_with(' ') {
                let parts: Vec<&str> = line.trim_end_matches(':').split_whitespace().collect();
                if parts.len() >= 2 {
                    pkgs.push(Package {
                        name: parts[0].to_string(),
                        version: parts[1].trim_start_matches('v').to_string(),
                        latest_version: None,
                        manager: "cargo",
                        selected: false,
                    });
                }
            }
        }
    }
    pkgs
}

#[cfg(target_os = "windows")]
async fn list_choco() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("choco", &["list", "-lo"]).output().await {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2
                && !line.contains("packages installed")
                && !line.starts_with("Chocolatey")
            {
                pkgs.push(Package {
                    name: parts[0].to_string(),
                    version: parts[1].to_string(),
                    latest_version: None,
                    manager: "choco",
                    selected: false,
                });
            }
        }
    }
    pkgs
}

#[cfg(target_os = "windows")]
async fn list_scoop() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("scoop", &["list"]).output().await {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                pkgs.push(Package {
                    name: parts[0].to_string(),
                    version: parts[1].to_string(),
                    latest_version: None,
                    manager: "scoop",
                    selected: false,
                });
            }
        }
    }
    pkgs
}

async fn list_dotnet() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("dotnet", &["tool", "list", "-g"])
        .output()
        .await
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                pkgs.push(Package {
                    name: parts[0].to_string(),
                    version: parts[1].to_string(),
                    latest_version: None,
                    manager: "dotnet",
                    selected: false,
                });
            }
        }
    }
    pkgs
}

async fn list_gem() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("gem", &["list", "--local"]).output().await {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if let Some(idx) = line.find(" (") {
                let name = &line[..idx];
                let ver_part = &line[idx + 2..line.len() - 1];
                let version = ver_part.split(", ").next().unwrap_or(ver_part);
                pkgs.push(Package {
                    name: name.to_string(),
                    version: version.to_string(),
                    latest_version: None,
                    manager: "gem",
                    selected: false,
                });
            }
        }
    }
    pkgs
}

#[cfg(not(target_os = "windows"))]
async fn list_brew() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("brew", &["list", "--versions"])
        .output()
        .await
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                pkgs.push(Package {
                    name: parts[0].to_string(),
                    version: parts[1].to_string(),
                    latest_version: None,
                    manager: "brew",
                    selected: false,
                });
            }
        }
    }
    pkgs
}

#[cfg(not(target_os = "windows"))]
async fn list_apt() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("apt", &["list", "--installed"])
        .output()
        .await
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines().skip(1) {
            if let Some(idx) = line.find('/') {
                let name = &line[..idx];
                if let Some(space_idx) = line.find(' ') {
                    let version_part = &line[space_idx..];
                    let version = version_part.split_whitespace().next().unwrap_or("unknown");
                    pkgs.push(Package {
                        name: name.to_string(),
                        version: version.to_string(),
                        latest_version: None,
                        manager: "apt",
                        selected: false,
                    });
                }
            }
        }
    }
    pkgs
}

#[cfg(not(target_os = "windows"))]
async fn list_pacman() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = build_command("pacman", &["-Qe"]).output().await {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                pkgs.push(Package {
                    name: parts[0].to_string(),
                    version: parts[1].to_string(),
                    latest_version: None,
                    manager: "pacman",
                    selected: false,
                });
            }
        }
    }
    pkgs
}

// =============================================================================
// App State & Event Loop
// =============================================================================

enum AppEvent {
    ManagerLoaded(&'static str, Vec<Package>),
    OutdatedLoaded(&'static str, Vec<(String, String)>),
    LogLine(String),
    OperationFinished(bool, Vec<Package>),
}

#[derive(PartialEq)]
enum ViewState {
    Managers,
    Packages(usize),
    Logs,
}

struct ManagerState {
    name: &'static str,
    loading: bool,
    packages: Vec<Package>,
}

struct App {
    managers: Vec<ManagerState>,
    managers_state: ListState,
    packages_state: ListState,
    view_state: ViewState,
    logs: Vec<String>,
    logs_scroll: u16,
    rx: mpsc::UnboundedReceiver<AppEvent>,
}

impl App {
    fn new(rx: mpsc::UnboundedReceiver<AppEvent>) -> App {
        let mut managers = Vec::new();
        for m in get_managers() {
            managers.push(ManagerState {
                name: m.name(),
                loading: true,
                packages: vec![],
            });
        }
        let mut app = App {
            managers,
            managers_state: ListState::default(),
            packages_state: ListState::default(),
            view_state: ViewState::Managers,
            logs: vec![],
            logs_scroll: 0,
            rx,
        };
        app.managers_state.select(Some(0));
        app
    }

    fn loading_count(&self) -> usize {
        self.managers.iter().filter(|m| m.loading).count()
    }
}

async fn run_operations(
    tx: mpsc::UnboundedSender<AppEvent>,
    packages: Vec<Package>,
    is_update: bool,
) {
    let managers = get_managers();
    for pkg in &packages {
        let _ = tx.send(AppEvent::LogLine(format!(
            "========================================"
        )));
        let action = if is_update {
            "Updating"
        } else {
            "Uninstalling"
        };
        let _ = tx.send(AppEvent::LogLine(format!(
            "{} {} via {}...",
            action, pkg.name, pkg.manager
        )));

        if let Some(m_type) = managers.iter().find(|m| m.name() == pkg.manager) {
            if m_type.requires_sudo() {
                let _ = tx.send(AppEvent::LogLine(format!(
                    "WARNING: {} requires sudo privileges. Using 'sudo -n' (will fail if password prompt is needed).",
                    pkg.manager
                )));
            }

            let mut cmd = if is_update {
                m_type.update_command(&pkg.name)
            } else {
                m_type.delete_command(&pkg.name)
            };

            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

            match cmd.spawn() {
                Ok(mut child) => {
                    let stdout = child.stdout.take();
                    let stderr = child.stderr.take();
                    let tx_out = tx.clone();
                    let tx_err = tx.clone();

                    let j1 = tokio::spawn(async move {
                        if let Some(out) = stdout {
                            let mut reader = BufReader::new(out).lines();
                            while let Ok(Some(line)) = reader.next_line().await {
                                let _ = tx_out.send(AppEvent::LogLine(line));
                            }
                        }
                    });

                    let j2 = tokio::spawn(async move {
                        if let Some(err) = stderr {
                            let mut reader = BufReader::new(err).lines();
                            while let Ok(Some(line)) = reader.next_line().await {
                                let _ = tx_err.send(AppEvent::LogLine(format!("ERROR: {}", line)));
                            }
                        }
                    });

                    let _ = tokio::join!(j1, j2);
                    let _ = child.wait().await;
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::LogLine(format!("Failed to start command: {}", e)));
                }
            }
        }
    }
    let _ = tx.send(AppEvent::LogLine(format!(
        "========================================"
    )));
    let _ = tx.send(AppEvent::LogLine(format!(
        "Operations complete. Press 'Esc' to return to dashboard."
    )));
    let _ = tx.send(AppEvent::OperationFinished(is_update, packages));
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (tx, rx) = mpsc::unbounded_channel();

    for manager in get_managers() {
        let tx_clone = tx.clone();
        let tx_outdated = tx.clone();
        tokio::spawn(async move {
            let mut pkgs = manager.list_packages().await;
            pkgs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            let _ = tx_clone.send(AppEvent::ManagerLoaded(manager.name(), pkgs));

            let outdated = manager.list_outdated().await;
            let _ = tx_outdated.send(AppEvent::OutdatedLoaded(manager.name(), outdated));
        });
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(rx);

    loop {
        while let Ok(event) = app.rx.try_recv() {
            match event {
                AppEvent::ManagerLoaded(name, pkgs) => {
                    if let Some(mgr) = app.managers.iter_mut().find(|m| m.name == name) {
                        mgr.packages = pkgs;
                        mgr.loading = false;
                    }
                }
                AppEvent::OutdatedLoaded(name, outdated) => {
                    if let Some(mgr) = app.managers.iter_mut().find(|m| m.name == name) {
                        for (pkg_name, latest_ver) in outdated {
                            if let Some(p) = mgr.packages.iter_mut().find(|p| p.name.trim().eq_ignore_ascii_case(pkg_name.trim())) {
                                p.latest_version = Some(latest_ver.trim().to_string());
                            }
                        }
                    }
                }
                AppEvent::LogLine(line) => {
                    app.logs.push(line);
                    if app.logs.len() > 10 {
                        app.logs_scroll = (app.logs.len() - 10) as u16;
                    }
                }
                AppEvent::OperationFinished(is_update, pkgs) => {
                    for p in pkgs {
                        if let Some(mgr) = app.managers.iter_mut().find(|m| m.name == p.manager) {
                            if is_update {
                                if let Some(pkg) = mgr.packages.iter_mut().find(|x| x.name == p.name) {
                                    if let Some(latest) = &pkg.latest_version {
                                        pkg.version = latest.clone();
                                    }
                                    pkg.latest_version = None;
                                    pkg.selected = false;
                                }
                            } else {
                                mgr.packages.retain(|x| x.name != p.name);
                            }
                        }
                    }
                }
            }
        }

        terminal.draw(|f| ui(f, &mut app))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => {
                            if app.view_state != ViewState::Logs {
                                break;
                            }
                        }
                        KeyCode::Esc | KeyCode::Backspace => {
                            if app.view_state == ViewState::Logs {
                                app.view_state = ViewState::Managers;
                                app.logs.clear();
                                app.logs_scroll = 0;
                            } else if let ViewState::Packages(idx) = app.view_state {
                                app.view_state = ViewState::Managers;
                                for pkg in &mut app.managers[idx].packages {
                                    pkg.selected = false;
                                }
                            } else {
                                break;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => match app.view_state {
                            ViewState::Managers => {
                                if let Some(i) = app.managers_state.selected() {
                                    app.managers_state.select(Some(
                                        if i >= app.managers.len() - 1 {
                                            0
                                        } else {
                                            i + 1
                                        },
                                    ));
                                } else {
                                    app.managers_state.select(Some(0));
                                }
                            }
                            ViewState::Packages(idx) => {
                                let count = app.managers[idx].packages.len();
                                if count > 0 {
                                    if let Some(i) = app.packages_state.selected() {
                                        app.packages_state.select(Some(if i >= count - 1 {
                                            0
                                        } else {
                                            i + 1
                                        }));
                                    } else {
                                        app.packages_state.select(Some(0));
                                    }
                                }
                            }
                            ViewState::Logs => {
                                app.logs_scroll = app.logs_scroll.saturating_add(1);
                            }
                        },
                        KeyCode::Up | KeyCode::Char('k') => match app.view_state {
                            ViewState::Managers => {
                                if let Some(i) = app.managers_state.selected() {
                                    app.managers_state.select(Some(if i == 0 {
                                        app.managers.len() - 1
                                    } else {
                                        i - 1
                                    }));
                                } else {
                                    app.managers_state.select(Some(0));
                                }
                            }
                            ViewState::Packages(idx) => {
                                let count = app.managers[idx].packages.len();
                                if count > 0 {
                                    if let Some(i) = app.packages_state.selected() {
                                        app.packages_state.select(Some(if i == 0 {
                                            count - 1
                                        } else {
                                            i - 1
                                        }));
                                    } else {
                                        app.packages_state.select(Some(0));
                                    }
                                }
                            }
                            ViewState::Logs => {
                                app.logs_scroll = app.logs_scroll.saturating_sub(1);
                            }
                        },
                        KeyCode::Char(' ') => {
                            if let ViewState::Packages(idx) = app.view_state {
                                if let Some(i) = app.packages_state.selected() {
                                    if i < app.managers[idx].packages.len() {
                                        app.managers[idx].packages[i].selected =
                                            !app.managers[idx].packages[i].selected;
                                    }
                                }
                            }
                        }
                        KeyCode::Enter => {
                            if app.view_state == ViewState::Managers {
                                if let Some(i) = app.managers_state.selected() {
                                    if !app.managers[i].loading {
                                        app.view_state = ViewState::Packages(i);
                                        app.packages_state.select(Some(0));
                                    }
                                }
                            }
                        }
                        KeyCode::Char('u') | KeyCode::Char('d') => {
                            if let ViewState::Packages(idx) = app.view_state {
                                let selected: Vec<_> = app.managers[idx]
                                    .packages
                                    .iter()
                                    .filter(|p| p.selected)
                                    .cloned()
                                    .collect();
                                if !selected.is_empty() {
                                    app.view_state = ViewState::Logs;
                                    let tx_clone = tx.clone();
                                    let is_update = key.code == KeyCode::Char('u');
                                    tokio::spawn(async move {
                                        run_operations(tx_clone, selected, is_update).await;
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(f.area());

    let loading_count = app.loading_count();
    let title_text = if loading_count > 0 {
        format!("Satellite - Loading {} managers...", loading_count)
    } else {
        "Satellite - Universal Package Manager".to_string()
    };
    let title = Paragraph::new(title_text)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL).title("Status"));
    f.render_widget(title, chunks[0]);

    match app.view_state {
        ViewState::Managers => {
            let items: Vec<ListItem> = app
                .managers
                .iter()
                .map(|m| {
                    let (count_str, loading_style) = if m.loading {
                        ("...".to_string(), Style::default().fg(Color::DarkGray))
                    } else if m.packages.is_empty() {
                        (
                            "[Not Installed]".to_string(),
                            Style::default().fg(Color::Red),
                        )
                    } else {
                        (
                            format!("{} packages", m.packages.len()),
                            Style::default().fg(Color::Green),
                        )
                    };

                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{:<15}", m.name),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(count_str, loading_style),
                    ]))
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Select Package Manager"),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");
            f.render_stateful_widget(list, chunks[1], &mut app.managers_state);

            let help = Paragraph::new("q: Quit | j/k or Up/Down: Navigate | Enter: View Packages")
                .style(Style::default().fg(Color::Gray))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(help, chunks[2]);
        }
        ViewState::Packages(idx) => {
            let items: Vec<ListItem> = app.managers[idx]
                .packages
                .iter()
                .map(|p| {
                    let prefix = if p.selected { "[X]" } else { "[ ]" };
                    let style = if p.selected {
                        Style::default().fg(Color::LightGreen)
                    } else {
                        Style::default()
                    };
                    let version_display = p.version.clone();
                    let update_span = if p.latest_version.is_some() {
                        Span::styled(" [Update Available]", Style::default().fg(Color::Yellow))
                    } else {
                        Span::raw("")
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{} {:<30} ", prefix, p.name), style),
                        Span::styled(version_display, Style::default().fg(Color::DarkGray)),
                        update_span,
                    ]))
                })
                .collect();

            let title = format!("{} Packages", app.managers[idx].name);
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(title))
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");
            f.render_stateful_widget(list, chunks[1], &mut app.packages_state);

            let help = Paragraph::new("Space: Select | u: Update | d: Delete | Esc: Back")
                .style(Style::default().fg(Color::Gray))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(help, chunks[2]);
        }
        ViewState::Logs => {
            let text: Vec<Line> = app.logs.iter().map(|l| Line::from(l.clone())).collect();
            let p = Paragraph::new(text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Operation Logs"),
                )
                .wrap(Wrap { trim: false })
                .scroll((app.logs_scroll, 0));
            f.render_widget(p, chunks[1]);

            let help = Paragraph::new("j/k: Scroll | Esc: Back to Dashboard")
                .style(Style::default().fg(Color::Gray))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(help, chunks[2]);
        }
    }
}
