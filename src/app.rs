use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use crate::ai::{self, AiReport, AiSnapshot, AnalysisProvider};
use crate::model::Project;
use crate::remote::{self, GithubPortfolio, RemoteUpdate};
use crate::scan;
use crate::settings::DashboardSettings;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Overview,
    Git,
    Delivery,
    Cairn,
    Portfolio,
    Insight,
}

impl Tab {
    pub const ALL: [Self; 6] = [
        Self::Overview,
        Self::Git,
        Self::Delivery,
        Self::Cairn,
        Self::Portfolio,
        Self::Insight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Git => "Git",
            Self::Delivery => "Delivery",
            Self::Cairn => "Cairn",
            Self::Portfolio => "Portfolio",
            Self::Insight => "Insight",
        }
    }

    pub fn next(self) -> Self {
        Self::ALL[(self as usize + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        Self::ALL[(self as usize + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProjectSort {
    #[default]
    Name,
    NeedsAttention,
    Recent,
}

impl ProjectSort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::NeedsAttention => "needs attention",
            Self::Recent => "recent activity",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Name => Self::NeedsAttention,
            Self::NeedsAttention => Self::Recent,
            Self::Recent => Self::Name,
        }
    }
}

pub struct App {
    pub root: PathBuf,
    pub dashboard: DashboardSettings,
    pub projects: Vec<Project>,
    pub selected: usize,
    pub tab: Tab,
    pub sort: ProjectSort,
    pub scanning: bool,
    pub filter: String,
    pub searching: bool,
    pub show_help: bool,
    pub show_ai_payload: bool,
    pub portfolio: Option<GithubPortfolio>,
    pub portfolio_loading: bool,
    pub message: String,
    pub remote_error: Option<String>,
    pub ai_error: Option<String>,
    pub ai_credential_source: Option<String>,
    pub ai_model: String,
    pub github_owner: Option<String>,
    loading: HashSet<PathBuf>,
    loaded: HashSet<PathBuf>,
    tx: Sender<RemoteUpdate>,
    rx: Receiver<RemoteUpdate>,
    scan_tx: Sender<Vec<Project>>,
    scan_rx: Receiver<Vec<Project>>,
    portfolio_tx: Sender<Result<GithubPortfolio, String>>,
    portfolio_rx: Receiver<Result<GithubPortfolio, String>>,
    ai_entries: HashMap<PathBuf, AiEntry>,
    ai_loading: HashSet<PathBuf>,
    ai_tx: Sender<AiUpdate>,
    ai_rx: Receiver<AiUpdate>,
}

struct AiEntry {
    fingerprint: String,
    report: AiReport,
}

struct AiUpdate {
    path: PathBuf,
    fingerprint: String,
    result: Result<AiReport, String>,
}

impl App {
    pub fn new(root: PathBuf) -> Self {
        Self::with_preferences(root, DashboardSettings::default())
    }

    pub fn with_preferences(root: PathBuf, dashboard: DashboardSettings) -> Self {
        let (tx, rx) = mpsc::channel();
        let (scan_tx, scan_rx) = mpsc::channel();
        let (portfolio_tx, portfolio_rx) = mpsc::channel();
        let (ai_tx, ai_rx) = mpsc::channel();
        let app = Self {
            root,
            github_owner: dashboard.github_owner.clone(),
            dashboard: dashboard.clone(),
            projects: Vec::new(),
            selected: 0,
            tab: dashboard.tab(),
            sort: dashboard.sort(),
            scanning: true,
            filter: String::new(),
            searching: false,
            show_help: false,
            show_ai_payload: false,
            portfolio: None,
            portfolio_loading: false,
            message: "scanning local repositories…".into(),
            remote_error: None,
            ai_error: None,
            ai_credential_source: ai::credential_source().map(|source| source.label().to_string()),
            ai_model: ai::OpenRouter::model(),
            loading: HashSet::new(),
            loaded: HashSet::new(),
            tx,
            rx,
            scan_tx,
            scan_rx,
            portfolio_tx,
            portfolio_rx,
            ai_entries: HashMap::new(),
            ai_loading: HashSet::new(),
            ai_tx,
            ai_rx,
        };
        app.start_scan();
        app
    }

    pub fn current(&self) -> Option<&Project> {
        self.visible_projects().nth(self.selected)
    }

    pub fn visible_projects(&self) -> impl Iterator<Item = &Project> {
        let filter = self.filter.to_lowercase();
        self.projects
            .iter()
            .filter(move |project| matches_filter(project, &filter))
    }

    pub fn visible_count(&self) -> usize {
        self.visible_projects().count()
    }

    pub fn is_loading(&self) -> bool {
        self.current()
            .is_some_and(|p| self.loading.contains(&p.path))
    }

    pub fn is_ai_loading(&self) -> bool {
        self.current()
            .is_some_and(|project| self.ai_loading.contains(&project.path))
    }

    pub fn current_ai_report(&self) -> Option<&AiReport> {
        let project = self.current()?;
        self.ai_entries
            .get(&project.path)
            .map(|entry| &entry.report)
    }

    pub fn current_ai_is_stale(&self) -> bool {
        let Some(project) = self.current() else {
            return false;
        };
        self.ai_entries.get(&project.path).is_some_and(|entry| {
            entry.fingerprint != AiSnapshot::from_project(project).fingerprint()
        })
    }

    pub fn current_ai_payload(&self) -> Option<String> {
        self.current()
            .map(AiSnapshot::from_project)
            .map(|snapshot| snapshot.compact_json())
    }

    pub fn select_next(&mut self) {
        let count = self.visible_count();
        if count > 0 {
            self.selected = (self.selected + 1) % count;
            self.remote_error = None;
            self.ai_error = None;
            self.ensure_remote();
        }
    }

    pub fn select_previous(&mut self) {
        let count = self.visible_count();
        if count > 0 {
            self.selected = (self.selected + count - 1) % count;
            self.remote_error = None;
            self.ai_error = None;
            self.ensure_remote();
        }
    }

    pub fn select_first(&mut self) {
        if self.visible_count() > 0 {
            self.selected = 0;
            self.remote_error = None;
            self.ai_error = None;
            self.ensure_remote();
        }
    }

    pub fn select_last(&mut self) {
        let count = self.visible_count();
        if count > 0 {
            self.selected = count - 1;
            self.remote_error = None;
            self.ai_error = None;
            self.ensure_remote();
        }
    }

    pub fn select_index(&mut self, index: usize) {
        if index < self.visible_count() {
            self.selected = index;
            self.remote_error = None;
            self.ai_error = None;
            self.ensure_remote();
        }
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn toggle_ai_payload(&mut self) {
        self.show_ai_payload = !self.show_ai_payload;
    }

    pub fn request_analysis(&mut self) {
        let Some(project) = self.current().cloned() else {
            return;
        };
        if self.ai_loading.contains(&project.path) {
            return;
        }
        let snapshot = AiSnapshot::from_project(&project);
        let fingerprint = snapshot.fingerprint();
        if self
            .ai_entries
            .get(&project.path)
            .is_some_and(|entry| entry.fingerprint == fingerprint)
        {
            self.message = "AI brief is current · using cached analysis".into();
            return;
        }
        if self.ai_credential_source.is_none() {
            self.ai_error = Some(ai::missing_key_message().into());
            return;
        }
        self.ai_loading.insert(project.path.clone());
        self.ai_error = None;
        self.message = format!("analyzing bounded metrics with {}…", self.ai_model);
        let path = project.path;
        let tx = self.ai_tx.clone();
        std::thread::spawn(move || {
            let result = ai::OpenRouter::load().and_then(|provider| provider.analyze(&snapshot));
            let _ = tx.send(AiUpdate {
                path,
                fingerprint,
                result,
            });
        });
    }

    pub fn refresh_local(&mut self) {
        if !self.scanning {
            self.scanning = true;
            self.message = "rescanning local repositories…".into();
            self.start_scan();
        }
    }

    pub fn refresh_remote(&mut self) {
        self.schedule_remote(true);
    }

    fn ensure_remote(&mut self) {
        self.schedule_remote(false);
    }

    fn schedule_remote(&mut self, force: bool) {
        let Some(project) = self.current().cloned() else {
            return;
        };
        if self.loading.contains(&project.path) || (!force && self.loaded.contains(&project.path)) {
            return;
        }
        self.loading.insert(project.path.clone());
        self.message = "fetching GitHub and deployment health…".into();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(remote::enrich(&project));
        });
    }

    pub fn poll(&mut self) {
        if let Ok(mut projects) = self.scan_rx.try_recv() {
            let selected = self.current().map(|project| project.path.clone());
            sort_projects(self.sort, &mut projects);
            self.projects = projects;
            self.restore_selection(selected.as_deref());
            self.scanning = false;
            self.loaded.clear();
            self.message = format!("{} projects · local scan complete", self.projects.len());
            self.ensure_remote();
            self.ensure_portfolio();
        }
        if let Ok(result) = self.portfolio_rx.try_recv() {
            self.portfolio_loading = false;
            match result {
                Ok(portfolio) => {
                    self.message = format!(
                        "{} local · {} GitHub repositories",
                        self.projects.len(),
                        portfolio.total
                    );
                    self.portfolio = Some(portfolio);
                }
                Err(error) => self.remote_error = Some(error),
            }
        }
        let mut score_changed = false;
        while let Ok(update) = self.rx.try_recv() {
            self.loading.remove(&update.path);
            self.loaded.insert(update.path.clone());
            if let Some(project) = self.projects.iter_mut().find(|p| p.path == update.path) {
                project.github = update.github;
                if let Some(url) = update.site_url {
                    project.site.url = Some(url);
                    project.site.deployed = Some(true);
                }
                if let Some((status, latency)) = update.site_status {
                    project.site.status_code = Some(status);
                    project.site.latency_ms = Some(latency);
                    project.site.healthy = Some((200..400).contains(&status));
                }
                project.recalculate_score();
                score_changed = true;
            }
            self.remote_error = update.error;
            self.message = "remote scan complete".into();
        }
        while let Ok(update) = self.ai_rx.try_recv() {
            self.ai_loading.remove(&update.path);
            match update.result {
                Ok(report) => {
                    self.message = format!("AI brief ready · {}", report.model);
                    self.ai_entries.insert(
                        update.path,
                        AiEntry {
                            fingerprint: update.fingerprint,
                            report,
                        },
                    );
                    self.ai_error = None;
                }
                Err(error) => {
                    self.message = "AI analysis unavailable · local dashboard unaffected".into();
                    self.ai_error = Some(error);
                }
            }
        }
        if score_changed && self.sort == ProjectSort::NeedsAttention {
            let selected = self.current().map(|project| project.path.clone());
            sort_projects(self.sort, &mut self.projects);
            self.restore_selection(selected.as_deref());
        }
    }

    pub fn cycle_sort(&mut self) {
        let selected = self.current().map(|project| project.path.clone());
        self.sort = self.sort.next();
        sort_projects(self.sort, &mut self.projects);
        self.restore_selection(selected.as_deref());
        self.message = format!("sorted by {}", self.sort.label());
    }

    pub fn start_search(&mut self) {
        self.searching = true;
        self.message = "type to filter projects · enter to apply · esc to clear".into();
    }

    pub fn search_push(&mut self, character: char) {
        self.filter.push(character);
        self.selected = 0;
    }

    pub fn search_pop(&mut self) {
        self.filter.pop();
        self.selected = 0;
    }

    pub fn finish_search(&mut self) {
        self.searching = false;
        self.message = if self.filter.is_empty() {
            "filter cleared".into()
        } else {
            format!("{} matching projects", self.visible_count())
        };
        self.ensure_remote();
    }

    pub fn clear_search(&mut self) {
        self.filter.clear();
        self.searching = false;
        self.selected = 0;
        self.message = "filter cleared".into();
        self.ensure_remote();
    }

    pub fn select_tab(&mut self, tab: Tab) {
        self.tab = tab;
    }

    pub fn open_target(&self) -> Option<&Path> {
        self.current().map(|project| project.path.as_path())
    }

    fn start_scan(&self) {
        let root = self.root.clone();
        let tx = self.scan_tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(scan::scan_all(&root));
        });
    }

    fn ensure_portfolio(&mut self) {
        if self.portfolio.is_some() || self.portfolio_loading {
            return;
        }
        let mut owners = std::collections::HashMap::<String, usize>::new();
        for slug in self
            .projects
            .iter()
            .filter_map(|project| project.remote_slug.as_deref())
        {
            if let Some((owner, _)) = slug.split_once('/') {
                *owners.entry(owner.to_string()).or_default() += 1;
            }
        }
        let owner = self.github_owner.clone().or_else(|| {
            owners
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(owner, _)| owner)
        });
        let Some(owner) = owner else {
            return;
        };
        self.portfolio_loading = true;
        let tx = self.portfolio_tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(remote::portfolio(&owner));
        });
    }

    fn restore_selection(&mut self, path: Option<&Path>) {
        self.selected = path
            .and_then(|path| {
                self.visible_projects()
                    .position(|project| project.path == path)
            })
            .unwrap_or(0)
            .min(self.visible_count().saturating_sub(1));
    }
}

fn matches_filter(project: &Project, filter: &str) -> bool {
    filter.is_empty()
        || project.name.to_lowercase().contains(filter)
        || project
            .description
            .as_deref()
            .is_some_and(|description| description.to_lowercase().contains(filter))
        || project
            .remote_slug
            .as_deref()
            .is_some_and(|slug| slug.to_lowercase().contains(filter))
}

fn sort_projects(sort: ProjectSort, projects: &mut [Project]) {
    match sort {
        ProjectSort::Name => {
            projects.sort_by_key(|project| project.name.to_lowercase());
        }
        ProjectSort::NeedsAttention => projects.sort_by(|a, b| {
            a.score
                .total
                .cmp(&b.score.total)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        }),
        ProjectSort::Recent => projects.sort_by(|a, b| {
            a.git
                .last_commit_age_days
                .unwrap_or(u64::MAX)
                .cmp(&b.git.last_commit_age_days.unwrap_or(u64::MAX))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attention_order_puts_the_lowest_score_first() {
        let mut projects = vec![
            Project {
                name: "healthy".into(),
                score: crate::model::Score {
                    total: 80,
                    ..Default::default()
                },
                ..Default::default()
            },
            Project {
                name: "needs-work".into(),
                score: crate::model::Score {
                    total: 25,
                    ..Default::default()
                },
                ..Default::default()
            },
        ];
        sort_projects(ProjectSort::NeedsAttention, &mut projects);
        assert_eq!(projects[0].name, "needs-work");
    }

    #[test]
    fn filters_names_descriptions_and_slugs_case_insensitively() {
        let project = Project {
            name: "Harrow".into(),
            description: Some("A Cairn backlog reader".into()),
            remote_slug: Some("oddurs/harrow".into()),
            ..Default::default()
        };
        assert!(matches_filter(&project, "harrow"));
        assert!(matches_filter(&project, "cairn"));
        assert!(matches_filter(&project, "oddurs"));
        assert!(!matches_filter(&project, "website"));
    }
}
