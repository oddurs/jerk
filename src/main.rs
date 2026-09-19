use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
    MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use jerk::app::{App, Tab};
use jerk::theme::Theme;
use jerk::ui::MouseAction;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use serde::Serialize;

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_help();
        return Ok(());
    }
    if args.iter().any(|arg| arg == "-V" || arg == "--version") {
        println!("jerk {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let plain = args.iter().any(|arg| arg == "--plain");
    let json = args.iter().any(|arg| arg == "--json");
    let ai_payload = args.iter().any(|arg| arg == "--ai-payload");
    let root = args
        .iter()
        .find(|arg| !arg.starts_with('-'))
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir().context("cannot determine current directory")?);
    let root = root
        .canonicalize()
        .with_context(|| format!("cannot open {}", root.display()))?;

    if json {
        return print_json(&root);
    }
    if ai_payload {
        return print_ai_payload(&root);
    }
    if plain || !io::stdout().is_terminal() {
        return print_plain(&root);
    }
    run(root)
}

fn print_help() {
    println!(
        "jerk — project pulse from the terminal\n\n\
         USAGE:\n  jerk [DIRECTORY] [--plain | --json | --ai-payload]\n\n\
         Scans a repository, or every immediate Git repository below DIRECTORY.\n\n\
         PRIVACY:\n  --ai-payload prints the exact metrics-only JSON used for AI analysis.\n\n\
         KEYS:\n  ↑/k ↓/j   select project\n  g/G       first/last project\n  ←/h →/l   change view\n  1–6       jump to view\n  a         analyze from the Insight view\n  p         preview the outbound AI payload\n  /         filter projects\n  s         cycle project ordering\n  r         rescan local data\n  R         refresh GitHub and deployment health\n  o         open project in the system file browser\n  ?         keyboard guide\n  q         quit\n"
    );
}

#[derive(Serialize)]
struct Snapshot<'a> {
    schema: u16,
    generated_at_unix: u64,
    root: &'a std::path::Path,
    projects: Vec<jerk::model::Project>,
}

fn print_json(root: &std::path::Path) -> Result<()> {
    let generated_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let snapshot = Snapshot {
        schema: jerk::schema::VERSION,
        generated_at_unix,
        root,
        projects: jerk::scan::scan_all(root),
    };
    println!("{}", serde_json::to_string_pretty(&snapshot)?);
    Ok(())
}

fn print_ai_payload(root: &std::path::Path) -> Result<()> {
    let snapshots = jerk::scan::scan_all(root)
        .iter()
        .map(jerk::ai::AiSnapshot::from_project)
        .collect::<Vec<_>>();
    if snapshots.len() == 1 {
        println!("{}", serde_json::to_string_pretty(&snapshots[0])?);
    } else {
        println!("{}", serde_json::to_string_pretty(&snapshots)?);
    }
    Ok(())
}

fn print_plain(root: &std::path::Path) -> Result<()> {
    let projects = jerk::scan::scan_all(root);
    if projects.is_empty() {
        println!("No Git repositories found below {}", root.display());
        return Ok(());
    }
    println!("SCORE  PROJECT              COMMITS  30D  DOCS  CAIRN  CI  SITE    LANGUAGE");
    for p in projects {
        let cairn = p
            .cairn
            .as_ref()
            .map(|c| format!("{}%", c.completion))
            .unwrap_or_else(|| "—".into());
        println!(
            "{:>3}    {:<20} {:>7} {:>4}  {:>3}/6  {:>5}  {:<3} {:<7} {}",
            p.score.total,
            truncate(&p.name, 20),
            p.git.commits,
            p.git.commits_30d,
            p.docs.count(),
            cairn,
            if p.delivery.ci { "yes" } else { "no" },
            if p.site.source { "source" } else { "—" },
            p.git
                .languages
                .first()
                .map(|(name, _)| name.as_str())
                .unwrap_or("—"),
        );
    }
    Ok(())
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    let mut out = value
        .chars()
        .take(width.saturating_sub(1))
        .collect::<String>();
    out.push('…');
    out
}

fn run(root: PathBuf) -> Result<()> {
    let mut guard = TerminalGuard::enter()?;
    let mut app = App::new(root);
    let theme = Theme::terminal();

    loop {
        app.poll();
        guard
            .terminal
            .draw(|frame| jerk::ui::draw(frame, &app, theme))?;
        if !event::poll(Duration::from_millis(120))? {
            continue;
        }
        let input = event::read()?;
        if let Event::Mouse(mouse) = input {
            if handle_mouse(&mut app, mouse) {
                break;
            }
            continue;
        }
        let Event::Key(key) = input else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if app.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => app.toggle_help(),
                _ => {}
            }
            continue;
        }
        if app.show_ai_payload {
            match key.code {
                KeyCode::Esc | KeyCode::Char('p') | KeyCode::Char('q') => {
                    app.toggle_ai_payload();
                }
                KeyCode::Char('a') => {
                    app.toggle_ai_payload();
                    app.request_analysis();
                }
                _ => {}
            }
            continue;
        }
        if app.searching {
            match key.code {
                KeyCode::Enter => app.finish_search(),
                KeyCode::Esc => app.clear_search(),
                KeyCode::Backspace => app.search_pop(),
                KeyCode::Char(character) => app.search_push(character),
                _ => {}
            }
            continue;
        }
        match key.code {
            KeyCode::Char('q') => break,
            KeyCode::Esc if !app.filter.is_empty() => app.clear_search(),
            KeyCode::Esc => break,
            KeyCode::Down | KeyCode::Char('j') => app.select_next(),
            KeyCode::Up | KeyCode::Char('k') => app.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => app.select_first(),
            KeyCode::Char('G') | KeyCode::End => app.select_last(),
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => app.tab = app.tab.next(),
            KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => app.tab = app.tab.previous(),
            KeyCode::Char('1') => app.select_tab(Tab::Overview),
            KeyCode::Char('2') => app.select_tab(Tab::Git),
            KeyCode::Char('3') => app.select_tab(Tab::Delivery),
            KeyCode::Char('4') => app.select_tab(Tab::Cairn),
            KeyCode::Char('5') => app.select_tab(Tab::Portfolio),
            KeyCode::Char('6') => app.select_tab(Tab::Insight),
            KeyCode::Char('a') if app.tab == Tab::Insight => app.request_analysis(),
            KeyCode::Char('p') if app.tab == Tab::Insight => app.toggle_ai_payload(),
            KeyCode::Char('r') => app.refresh_local(),
            KeyCode::Char('R') => app.refresh_remote(),
            KeyCode::Char('s') => app.cycle_sort(),
            KeyCode::Char('/') => app.start_search(),
            KeyCode::Char('?') => app.toggle_help(),
            KeyCode::Char('o') | KeyCode::Enter => {
                if let Some(path) = app.open_target() {
                    open(path);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn handle_mouse(app: &mut App, mouse: MouseEvent) -> bool {
    match mouse.kind {
        MouseEventKind::ScrollDown => {
            app.select_next();
            return false;
        }
        MouseEventKind::ScrollUp => {
            app.select_previous();
            return false;
        }
        MouseEventKind::Down(MouseButton::Left) => {}
        _ => return false,
    }
    let Ok((width, height)) = crossterm::terminal::size() else {
        return false;
    };
    let Some(action) =
        jerk::ui::mouse_action(app, Rect::new(0, 0, width, height), mouse.column, mouse.row)
    else {
        return false;
    };
    match action {
        MouseAction::SelectTab(tab) => app.select_tab(tab),
        MouseAction::SelectProject(index) => app.select_index(index),
        MouseAction::Analyze => {
            if app.show_ai_payload {
                app.toggle_ai_payload();
            }
            app.select_tab(Tab::Insight);
            app.request_analysis();
        }
        MouseAction::Payload => {
            app.select_tab(Tab::Insight);
            app.toggle_ai_payload();
        }
        MouseAction::Refresh => app.refresh_remote(),
        MouseAction::Sort => app.cycle_sort(),
        MouseAction::Open => {
            if let Some(path) = app.open_target() {
                open(path);
            }
        }
        MouseAction::Help => app.toggle_help(),
        MouseAction::Quit => return true,
        MouseAction::CloseOverlay => {
            if app.show_help {
                app.toggle_help();
            } else if app.show_ai_payload {
                app.toggle_ai_payload();
            }
        }
    }
    false
}

fn open(path: &std::path::Path) {
    let _ = open_command(path).spawn();
}

#[cfg(target_os = "macos")]
fn open_command(path: &std::path::Path) -> Command {
    let mut command = Command::new("open");
    command.arg(path);
    command
}

#[cfg(target_os = "windows")]
fn open_command(path: &std::path::Path) -> Command {
    let mut command = Command::new("explorer.exe");
    command.arg(path);
    command
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_command(path: &std::path::Path) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(path);
    command
}

#[cfg(not(any(unix, target_os = "windows")))]
fn open_command(path: &std::path::Path) -> Command {
    let mut command = Command::new("open");
    command.arg(path);
    command
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
            let _ = execute!(stdout, DisableMouseCapture, LeaveAlternateScreen);
            let _ = disable_raw_mode();
            return Err(error.into());
        }
        let terminal = match Terminal::new(CrosstermBackend::new(stdout)) {
            Ok(terminal) => terminal,
            Err(error) => {
                let mut cleanup = io::stdout();
                let _ = execute!(cleanup, DisableMouseCapture, LeaveAlternateScreen);
                let _ = disable_raw_mode();
                return Err(error.into());
            }
        };
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            DisableMouseCapture,
            LeaveAlternateScreen
        );
        let _ = self.terminal.show_cursor();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_keeps_the_requested_width() {
        assert_eq!(truncate("a rather long project", 8), "a rathe…");
        assert_eq!(truncate("short", 8), "short");
    }
}
