use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use crate::model::Project;
use crate::remote::{self, GithubPortfolio, RemoteUpdate};
use crate::scan;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Overview,
    Git,
    Delivery,
    Cairn,
    Portfolio,
}

impl Tab {
    pub const ALL: [Self; 5] = [
        Self::Overview,
        Self::Git,
        Self::Delivery,
        Self::Cairn,
        Self::Portfolio,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Git => "Git",
            Self::Delivery => "Delivery",
            Self::Cairn => "Cairn",
            Self::Portfolio => "Portfolio",
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
    pub projects: Vec<Project>,
    pub selected: usize,
    pub tab: Tab,
    pub sort: ProjectSort,
    pub scanning: bool,
    pub filter: String,
    pub searching: bool,
    pub show_help: bool,
    pub portfolio: Option<GithubPortfolio>,
    pub portfolio_loading: bool,
    pub message: String,
    pub remote_error: Option<String>,
    loading: HashSet<PathBuf>,
    loaded: HashSet<PathBuf>,
    tx: Sender<RemoteUpdate>,
    rx: Receiver<RemoteUpdate>,
    scan_tx: Sender<Vec<Project>>,
    scan_rx: Receiver<Vec<Project>>,
    portfolio_tx: Sender<Result<GithubPortfolio, String>>,
    portfolio_rx: Receiver<Result<GithubPortfolio, String>>,
}

impl App {
    pub fn new(root: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel();
        let (scan_tx, scan_rx) = mpsc::channel();
        let (portfolio_tx, portfolio_rx) = mpsc::channel();
        let app = Self {
            root,
            projects: Vec::new(),
            selected: 0,
            tab: Tab::Overview,
            sort: ProjectSort::Name,
            scanning: true,
            filter: String::new(),
            searching: false,
            show_help: false,
            portfolio: None,
            portfolio_loading: false,
            message: "scanning local repositories…".into(),
            remote_error: None,
            loading: HashSet::new(),
            loaded: HashSet::new(),
            tx,
            rx,
            scan_tx,
            scan_rx,
            portfolio_tx,
            portfolio_rx,
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

    pub fn select_next(&mut self) {
        let count = self.visible_count();
        if count > 0 {
            self.selected = (self.selected + 1) % count;
            self.remote_error = None;
            self.ensure_remote();
        }
    }

    pub fn select_previous(&mut self) {
        let count = self.visible_count();
        if count > 0 {
            self.selected = (self.selected + count - 1) % count;
            self.remote_error = None;
            self.ensure_remote();
        }
    }

    pub fn select_first(&mut self) {
        if self.visible_count() > 0 {
            self.selected = 0;
            self.remote_error = None;
            self.ensure_remote();
        }
    }

    pub fn select_last(&mut self) {
        let count = self.visible_count();
        if count > 0 {
            self.selected = count - 1;
            self.remote_error = None;
            self.ensure_remote();
        }
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
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
        let Some(owner) = owners
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(owner, _)| owner)
        else {
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
            projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
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
