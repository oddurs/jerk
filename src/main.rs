use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use jerk::app::{App, Tab};
use jerk::theme::Theme;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
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
    if plain || !io::stdout().is_terminal() {
        return print_plain(&root);
    }
    run(root)
}

fn print_help() {
    println!(
        "jerk — project pulse from the terminal\n\n\
         USAGE:\n  jerk [DIRECTORY] [--plain | --json]\n\n\
         Scans a repository, or every immediate Git repository below DIRECTORY.\n\n\
         KEYS:\n  ↑/k ↓/j   select project\n  g/G       first/last project\n  ←/h →/l   change view\n  1–5       jump to view\n  /         filter projects\n  s         cycle project ordering\n  r         rescan local data\n  R         refresh GitHub and deployment health\n  o         open project in the system file browser\n  ?         keyboard guide\n  q         quit\n"
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
        let Event::Key(key) = event::read()? else {
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
        if let Err(error) = execute!(stdout, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
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
