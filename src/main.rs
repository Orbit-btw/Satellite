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
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::{error::Error, io, process::Command, sync::mpsc, thread, time::Duration};

#[derive(Clone)]
struct Package {
    name: String,
    version: String,
    manager: &'static str,
    selected: bool,
}

struct ManagerDef {
    name: &'static str,
    list_fn: fn() -> Vec<Package>,
    update_fn: fn(&str) -> Command,
    delete_fn: fn(&str) -> Command,
}

fn get_managers() -> Vec<ManagerDef> {
    vec![
        ManagerDef {
            name: "winget",
            list_fn: list_winget,
            update_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "winget", "upgrade", pkg]);
                c
            },
            delete_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "winget", "uninstall", pkg]);
                c
            },
        },
        ManagerDef {
            name: "npm",
            list_fn: list_npm,
            update_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "npm", "update", "-g", pkg]);
                c
            },
            delete_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "npm", "uninstall", "-g", pkg]);
                c
            },
        },
        ManagerDef {
            name: "pnpm",
            list_fn: list_pnpm,
            update_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "pnpm", "update", "-g", pkg]);
                c
            },
            delete_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "pnpm", "remove", "-g", pkg]);
                c
            },
        },
        ManagerDef {
            name: "pip",
            list_fn: list_pip,
            update_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "pip", "install", "--upgrade", pkg]);
                c
            },
            delete_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "pip", "uninstall", "-y", pkg]);
                c
            },
        },
        ManagerDef {
            name: "uv",
            list_fn: list_uv,
            update_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "uv", "tool", "upgrade", pkg]);
                c
            },
            delete_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "uv", "tool", "uninstall", pkg]);
                c
            },
        },
        ManagerDef {
            name: "cargo",
            list_fn: list_cargo,
            update_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "cargo", "install", pkg]);
                c
            },
            delete_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "cargo", "uninstall", pkg]);
                c
            },
        },
        ManagerDef {
            name: "choco",
            list_fn: list_choco,
            update_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "choco", "upgrade", pkg, "-y"]);
                c
            },
            delete_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "choco", "uninstall", pkg, "-y"]);
                c
            },
        },
        ManagerDef {
            name: "scoop",
            list_fn: list_scoop,
            update_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "scoop", "update", pkg]);
                c
            },
            delete_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "scoop", "uninstall", pkg]);
                c
            },
        },
        ManagerDef {
            name: "gem",
            list_fn: list_gem,
            update_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "gem", "update", pkg]);
                c
            },
            delete_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "gem", "uninstall", pkg]);
                c
            },
        },
        ManagerDef {
            name: "dotnet",
            list_fn: list_dotnet,
            update_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "dotnet", "tool", "update", "-g", pkg]);
                c
            },
            delete_fn: |pkg| {
                let mut c = Command::new("cmd");
                c.args(["/C", "dotnet", "tool", "uninstall", "-g", pkg]);
                c
            },
        },
    ]
}

// -----------------------------------------------------------------------------
// Parsers
// -----------------------------------------------------------------------------

fn list_winget() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = Command::new("cmd").args(["/C", "winget", "list"]).output() {
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

fn list_npm() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = Command::new("cmd")
        .args(["/C", "npm", "list", "-g", "--depth=0", "--json"])
        .output()
    {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(deps) = json["dependencies"].as_object() {
                for (k, v) in deps {
                    let ver = v["version"].as_str().unwrap_or("unknown");
                    pkgs.push(Package {
                        name: k.clone(),
                        version: ver.to_string(),
                        manager: "npm",
                        selected: false,
                    });
                }
            }
        }
    }
    pkgs
}

fn list_pnpm() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = Command::new("cmd")
        .args(["/C", "pnpm", "ls", "-g", "--depth=0", "--json"])
        .output()
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

fn list_pip() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = Command::new("cmd")
        .args(["/C", "pip", "list", "--format=json"])
        .output()
    {
        if let Ok(json) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
            for v in json {
                if let (Some(n), Some(ver)) = (v["name"].as_str(), v["version"].as_str()) {
                    pkgs.push(Package {
                        name: n.to_string(),
                        version: ver.to_string(),
                        manager: "pip",
                        selected: false,
                    });
                }
            }
        }
    }
    pkgs
}

fn list_uv() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = Command::new("cmd")
        .args(["/C", "uv", "tool", "list"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[0];
                let version = parts[1].trim_start_matches('v');
                pkgs.push(Package {
                    name: name.to_string(),
                    version: version.to_string(),
                    manager: "uv",
                    selected: false,
                });
            }
        }
    }
    pkgs
}

fn list_cargo() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = Command::new("cmd")
        .args(["/C", "cargo", "install", "--list"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.ends_with(':') && !line.starts_with(' ') {
                let parts: Vec<&str> = line.trim_end_matches(':').split_whitespace().collect();
                if parts.len() >= 2 {
                    pkgs.push(Package {
                        name: parts[0].to_string(),
                        version: parts[1].trim_start_matches('v').to_string(),
                        manager: "cargo",
                        selected: false,
                    });
                }
            }
        }
    }
    pkgs
}

fn list_choco() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = Command::new("cmd")
        .args(["/C", "choco", "list", "-lo"])
        .output()
    {
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
                    manager: "choco",
                    selected: false,
                });
            }
        }
    }
    pkgs
}

fn list_scoop() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = Command::new("cmd").args(["/C", "scoop", "list"]).output() {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                pkgs.push(Package {
                    name: parts[0].to_string(),
                    version: parts[1].to_string(),
                    manager: "scoop",
                    selected: false,
                });
            }
        }
    }
    pkgs
}

fn list_dotnet() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = Command::new("cmd")
        .args(["/C", "dotnet", "tool", "list", "-g"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                pkgs.push(Package {
                    name: parts[0].to_string(),
                    version: parts[1].to_string(),
                    manager: "dotnet",
                    selected: false,
                });
            }
        }
    }
    pkgs
}

fn list_gem() -> Vec<Package> {
    let mut pkgs = vec![];
    if let Ok(out) = Command::new("cmd")
        .args(["/C", "gem", "list", "--local"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if let Some(idx) = line.find(" (") {
                let name = &line[..idx];
                let ver_part = &line[idx + 2..line.len() - 1];
                let version = ver_part.split(", ").next().unwrap_or(ver_part);
                pkgs.push(Package {
                    name: name.to_string(),
                    version: version.to_string(),
                    manager: "gem",
                    selected: false,
                });
            }
        }
    }
    pkgs
}

// -----------------------------------------------------------------------------
// App State
// -----------------------------------------------------------------------------

enum AppEvent {
    ManagerLoaded(&'static str, Vec<Package>),
}

#[derive(PartialEq)]
enum ViewState {
    Managers,
    Packages(usize), // index of the manager in `app.managers`
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
    rx: mpsc::Receiver<AppEvent>,
}

impl App {
    fn new(rx: mpsc::Receiver<AppEvent>) -> App {
        let mut managers = Vec::new();
        for m in get_managers() {
            managers.push(ManagerState {
                name: m.name,
                loading: true,
                packages: vec![],
            });
        }

        let mut app = App {
            managers,
            managers_state: ListState::default(),
            packages_state: ListState::default(),
            view_state: ViewState::Managers,
            rx,
        };
        app.managers_state.select(Some(0));
        app
    }

    fn loading_count(&self) -> usize {
        self.managers.iter().filter(|m| m.loading).count()
    }

    fn next(&mut self) {
        match self.view_state {
            ViewState::Managers => {
                let i = match self.managers_state.selected() {
                    Some(i) => {
                        if i >= self.managers.len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.managers_state.select(Some(i));
            }
            ViewState::Packages(idx) => {
                let count = self.managers[idx].packages.len();
                if count == 0 {
                    return;
                }
                let i = match self.packages_state.selected() {
                    Some(i) => {
                        if i >= count - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.packages_state.select(Some(i));
            }
        }
    }

    fn previous(&mut self) {
        match self.view_state {
            ViewState::Managers => {
                let i = match self.managers_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.managers.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.managers_state.select(Some(i));
            }
            ViewState::Packages(idx) => {
                let count = self.managers[idx].packages.len();
                if count == 0 {
                    return;
                }
                let i = match self.packages_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            count - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.packages_state.select(Some(i));
            }
        }
    }

    fn toggle_selection(&mut self) {
        if let ViewState::Packages(idx) = self.view_state {
            if let Some(i) = self.packages_state.selected() {
                if i < self.managers[idx].packages.len() {
                    self.managers[idx].packages[i].selected =
                        !self.managers[idx].packages[i].selected;
                }
            }
        }
    }
}

enum RunAction {
    Update(Vec<Package>),
    Delete(Vec<Package>),
    Quit,
}

fn main() -> Result<(), Box<dyn Error>> {
    let (tx, rx) = mpsc::channel();

    // Spawn threads for all managers
    for manager in get_managers() {
        let tx_clone = tx.clone();
        thread::spawn(move || {
            let mut pkgs = (manager.list_fn)();
            pkgs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            let _ = tx_clone.send(AppEvent::ManagerLoaded(manager.name, pkgs));
        });
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(rx);
    let action = run_app(&mut terminal, &mut app)?;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    let defs = get_managers();

    match action {
        RunAction::Update(selected) => {
            println!("Starting updates for {} packages...", selected.len());
            for pkg in selected {
                println!("========================================");
                println!("Updating {} via {}...", pkg.name, pkg.manager);
                if let Some(def) = defs.iter().find(|d| d.name == pkg.manager) {
                    let child = (def.update_fn)(&pkg.name).spawn();
                    if let Ok(mut c) = child {
                        let _ = c.wait();
                    } else {
                        println!("Failed to start update for {}", pkg.name);
                    }
                }
            }
            println!("========================================");
            println!("All updates finished!");
        }
        RunAction::Delete(selected) => {
            println!("Starting deletion for {} packages...", selected.len());
            for pkg in selected {
                println!("========================================");
                println!("Uninstalling {} via {}...", pkg.name, pkg.manager);
                if let Some(def) = defs.iter().find(|d| d.name == pkg.manager) {
                    let child = (def.delete_fn)(&pkg.name).spawn();
                    if let Ok(mut c) = child {
                        let _ = c.wait();
                    } else {
                        println!("Failed to start uninstall for {}", pkg.name);
                    }
                }
            }
            println!("========================================");
            println!("All deletions finished!");
        }
        RunAction::Quit => {
            println!("Satellite CLI exiting.");
        }
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<RunAction> {
    loop {
        while let Ok(event) = app.rx.try_recv() {
            match event {
                AppEvent::ManagerLoaded(name, pkgs) => {
                    if let Some(mgr) = app.managers.iter_mut().find(|m| m.name == name) {
                        mgr.packages = pkgs;
                        mgr.loading = false;
                    }
                }
            }
        }

        terminal.draw(|f| ui(f, app))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => return Ok(RunAction::Quit),
                        KeyCode::Esc | KeyCode::Backspace => {
                            if let ViewState::Packages(idx) = app.view_state {
                                app.view_state = ViewState::Managers;
                                for pkg in &mut app.managers[idx].packages {
                                    pkg.selected = false;
                                }
                            } else {
                                return Ok(RunAction::Quit);
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => app.next(),
                        KeyCode::Up | KeyCode::Char('k') => app.previous(),
                        KeyCode::Char(' ') => app.toggle_selection(),
                        KeyCode::Enter => {
                            if app.loading_count() > 0 && app.view_state == ViewState::Managers {
                                if let Some(i) = app.managers_state.selected() {
                                    if !app.managers[i].loading {
                                        app.view_state = ViewState::Packages(i);
                                        app.packages_state.select(Some(0));
                                    }
                                }
                            } else if app.view_state == ViewState::Managers {
                                if let Some(i) = app.managers_state.selected() {
                                    app.view_state = ViewState::Packages(i);
                                    app.packages_state.select(Some(0));
                                }
                            }
                        }
                        KeyCode::Char('u') => {
                            if let ViewState::Packages(idx) = app.view_state {
                                let selected: Vec<_> = app.managers[idx]
                                    .packages
                                    .iter()
                                    .filter(|p| p.selected)
                                    .cloned()
                                    .collect();
                                if !selected.is_empty() {
                                    return Ok(RunAction::Update(selected));
                                }
                            }
                        }
                        KeyCode::Char('d') => {
                            if let ViewState::Packages(idx) = app.view_state {
                                let selected: Vec<_> = app.managers[idx]
                                    .packages
                                    .iter()
                                    .filter(|p| p.selected)
                                    .cloned()
                                    .collect();
                                if !selected.is_empty() {
                                    return Ok(RunAction::Delete(selected));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
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
                    let count_str = if m.loading {
                        "...".to_string()
                    } else {
                        m.packages.len().to_string()
                    };
                    let loading_style = if m.loading {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::Green)
                    };

                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{:<15}", m.name),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(format!("{} packages", count_str), loading_style),
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
            let mgr = &app.managers[idx];
            let items: Vec<ListItem> = mgr
                .packages
                .iter()
                .map(|p| {
                    let checkbox = if p.selected { "[x]" } else { "[ ]" };
                    let name_style = if p.selected {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default()
                    };

                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{} ", checkbox),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            format!("{:<30}", p.name),
                            name_style.add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(format!("v{}", p.version)),
                    ]))
                })
                .collect();

            let block_title = format!("{} Packages", mgr.name);
            if items.is_empty() {
                let empty = Paragraph::new(if mgr.loading {
                    "Loading..."
                } else {
                    "No packages found for this manager."
                })
                .block(Block::default().borders(Borders::ALL).title(block_title));
                f.render_widget(empty, chunks[1]);
            } else {
                let list = List::new(items)
                    .block(Block::default().borders(Borders::ALL).title(block_title))
                    .highlight_style(
                        Style::default()
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol(">> ");
                f.render_stateful_widget(list, chunks[1], &mut app.packages_state);
            }

            let help = Paragraph::new(
                "Esc/Backspace: Back | Space: Select | u: Update Selected | d: Delete Selected",
            )
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().borders(Borders::ALL));
            f.render_widget(help, chunks[2]);
        }
    }
}
