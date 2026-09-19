use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub description: Option<String>,
    pub remote_slug: Option<String>,
    pub profile: ProjectProfile,
    pub git: GitStats,
    pub docs: DocsStats,
    pub delivery: DeliveryStats,
    pub site: SiteStats,
    pub cairn: Option<CairnStats>,
    pub github: Option<GithubStats>,
    pub score: Score,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct GitStats {
    pub branch: String,
    pub dirty: usize,
    pub ahead: usize,
    pub behind: usize,
    pub commits: usize,
    pub commits_30d: usize,
    pub contributors: usize,
    pub branches: usize,
    pub tags: usize,
    pub tracked_files: usize,
    pub languages: Vec<(String, usize)>,
    pub additions_30d: usize,
    pub deletions_30d: usize,
    pub last_commit_age_days: Option<u64>,
    pub last_commit_date: Option<String>,
    pub last_subject: Option<String>,
    pub activity: Vec<u64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct DocsStats {
    pub readme: bool,
    pub license: bool,
    pub contributing: bool,
    pub changelog: bool,
    pub docs_dir: bool,
    pub api_docs: bool,
}

impl DocsStats {
    pub fn count(&self) -> usize {
        [
            self.readme,
            self.license,
            self.contributing,
            self.changelog,
            self.docs_dir,
            self.api_docs,
        ]
        .into_iter()
        .filter(|present| *present)
        .count()
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct DeliveryStats {
    pub ci: bool,
    pub tests: bool,
    pub package: bool,
    pub release_automation: bool,
    pub dependency_updates: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SiteStats {
    pub source: bool,
    pub url: Option<String>,
    pub deployed: Option<bool>,
    pub healthy: Option<bool>,
    pub status_code: Option<u16>,
    pub latency_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CairnStats {
    pub total: usize,
    pub open: usize,
    pub active: usize,
    pub done: usize,
    pub dropped: usize,
    pub blocked: usize,
    pub milestones: usize,
    pub completion: u16,
    pub active_items: Vec<String>,
    pub next_items: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct GithubStats {
    pub open_prs: usize,
    pub merged_prs: usize,
    pub closed_prs: usize,
    pub open_issues: usize,
    pub stars: usize,
    pub forks: usize,
    pub unique_views_14d: Option<usize>,
    pub unique_clones_14d: Option<usize>,
    pub release_downloads: Option<usize>,
    pub latest_workflow: Option<String>,
    pub latest_release: Option<String>,
    pub ci_state: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Score {
    pub total: u16,
    pub local: u16,
    pub documentation: u16,
    pub momentum: u16,
    pub delivery: u16,
    pub planning: u16,
    pub community: u16,
    pub confidence: u16,
    pub signals: Vec<Signal>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Signal {
    pub kind: SignalKind,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalKind {
    Good,
    Warn,
    Bad,
    Info,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProjectProfile {
    pub kind: ProjectKind,
    pub lifecycle: Lifecycle,
    pub tier: Tier,
    pub source: ProfileSource,
    pub intent: Option<String>,
    pub outcomes: Vec<String>,
    pub expectations: Expectations,
    pub components: Vec<ProjectComponent>,
    pub healthchecks: Vec<Healthcheck>,
    pub config_error: Option<String>,
}

impl Default for ProjectProfile {
    fn default() -> Self {
        Self {
            kind: ProjectKind::Unknown,
            lifecycle: Lifecycle::Incubating,
            tier: Tier::Experimental,
            source: ProfileSource::Inferred,
            intent: None,
            outcomes: Vec::new(),
            expectations: Expectations::default(),
            components: Vec::new(),
            healthchecks: Vec::new(),
            config_error: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectKind {
    Cli,
    Tui,
    Web,
    Library,
    Service,
    Research,
    Creative,
    Simulation,
    Theme,
    Infrastructure,
    Configuration,
    Distribution,
    Learning,
    Application,
    #[default]
    Unknown,
}

impl ProjectKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cli => "CLI",
            Self::Tui => "TUI",
            Self::Web => "WEB",
            Self::Library => "LIBRARY",
            Self::Service => "SERVICE",
            Self::Research => "RESEARCH",
            Self::Creative => "CREATIVE",
            Self::Simulation => "SIMULATION",
            Self::Theme => "THEME",
            Self::Infrastructure => "INFRA",
            Self::Configuration => "CONFIG",
            Self::Distribution => "DISTRIBUTION",
            Self::Learning => "LEARNING",
            Self::Application => "APP",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Lifecycle {
    #[default]
    Incubating,
    Active,
    Maintenance,
    Complete,
    Paused,
    Archived,
}

impl Lifecycle {
    pub fn label(self) -> &'static str {
        match self {
            Self::Incubating => "INCUBATING",
            Self::Active => "ACTIVE",
            Self::Maintenance => "MAINTENANCE",
            Self::Complete => "COMPLETE",
            Self::Paused => "PAUSED",
            Self::Archived => "ARCHIVED",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    Flagship,
    Supported,
    #[default]
    Experimental,
    Personal,
    Archive,
}

impl Tier {
    pub fn label(self) -> &'static str {
        match self {
            Self::Flagship => "FLAGSHIP",
            Self::Supported => "SUPPORTED",
            Self::Experimental => "EXPERIMENT",
            Self::Personal => "PERSONAL",
            Self::Archive => "ARCHIVE",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileSource {
    Declared,
    #[default]
    Inferred,
}

#[derive(Clone, Debug, Serialize)]
pub struct Expectations {
    pub documentation: bool,
    pub tests: bool,
    pub ci: bool,
    pub packaging: bool,
    pub releases: bool,
    pub site: bool,
    pub deployment: bool,
    pub planning: bool,
    pub collaboration: bool,
}

impl Default for Expectations {
    fn default() -> Self {
        Self {
            documentation: true,
            tests: true,
            ci: true,
            packaging: true,
            releases: true,
            site: true,
            deployment: true,
            planning: true,
            collaboration: true,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProjectComponent {
    pub name: String,
    pub kind: String,
    pub target: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Healthcheck {
    pub name: String,
    pub url: String,
    #[serde(default = "default_status")]
    pub expect_status: u16,
}

fn default_status() -> u16 {
    200
}

impl Project {
    pub fn recalculate_score(&mut self) {
        let mut s = Score::default();

        if self.profile.expectations.documentation {
            s.documentation += u16::from(self.docs.readme) * 6;
            s.documentation += u16::from(self.docs.license) * 3;
            s.documentation += u16::from(self.docs.docs_dir || self.docs.api_docs) * 3;
            s.documentation += u16::from(self.docs.contributing || self.docs.changelog) * 3;
        } else {
            s.documentation = 15;
        }

        s.momentum += if self.git.commits_30d >= 10 {
            8
        } else if self.git.commits_30d > 0 {
            5
        } else {
            0
        };
        s.momentum += match self.git.last_commit_age_days {
            Some(0..=14) => 4,
            Some(15..=60) => 2,
            _ => 0,
        };
        s.momentum += if self.git.contributors >= 2 { 4 } else { 2 };
        s.momentum += u16::from(self.git.tags > 0) * 2;
        s.momentum += u16::from(self.git.dirty == 0) * 2;
        if matches!(
            self.profile.lifecycle,
            Lifecycle::Complete | Lifecycle::Paused | Lifecycle::Archived
        ) {
            s.momentum = 20;
        }

        s.delivery += if self.profile.expectations.ci {
            u16::from(self.delivery.ci) * 6
        } else {
            6
        };
        s.delivery += if self.profile.expectations.tests {
            u16::from(self.delivery.tests) * 5
        } else {
            5
        };
        s.delivery += if self.profile.expectations.packaging {
            u16::from(self.delivery.package) * 3
        } else {
            3
        };
        s.delivery += if self.profile.expectations.releases {
            u16::from(self.delivery.release_automation) * 2
        } else {
            2
        };
        s.delivery += if self.profile.expectations.site {
            u16::from(self.site.source)
        } else {
            1
        };
        s.delivery += if self.profile.expectations.deployment {
            u16::from(self.site.healthy == Some(true)) * 3
        } else {
            3
        };

        if !self.profile.expectations.planning {
            s.planning = 20;
        } else if let Some(cairn) = &self.cairn {
            s.planning += 5;
            s.planning += cairn.completion / 10;
            s.planning += u16::from(cairn.active > 0) * 3;
            s.planning += u16::from(cairn.blocked == 0) * 2;
        }

        // Fifteen points describe the basic shape of a real, shareable project.
        s.local += 5;
        s.local += u16::from(self.remote_slug.is_some()) * 5;
        s.local += u16::from(self.description.is_some()) * 3;
        s.local += u16::from(self.git.commits > 1) * 2;

        if !self.profile.expectations.collaboration {
            s.community = 10;
        } else if let Some(gh) = &self.github {
            s.community += u16::from(gh.merged_prs > 0);
            s.community += u16::from(gh.ci_state.as_deref() == Some("SUCCESS")) * 3;
            s.community += u16::from(gh.latest_release.is_some()) * 2;
            s.community += u16::from(gh.unique_clones_14d.unwrap_or_default() > 0) * 2;
            s.community += u16::from(gh.unique_views_14d.unwrap_or_default() > 0);
            s.community += u16::from(gh.open_prs < 10);
        }

        s.total = s.documentation + s.momentum + s.delivery + s.planning + s.local + s.community;
        s.total = s.total.min(100);
        s.confidence = 55;
        if self.profile.source == ProfileSource::Declared {
            s.confidence += 15;
        }
        if self.github.is_some() || !self.profile.expectations.collaboration {
            s.confidence += 15;
        }
        if self.site.healthy.is_some() || !self.profile.expectations.deployment {
            s.confidence += 15;
        }
        if self.profile.config_error.is_some() {
            s.confidence = s.confidence.saturating_sub(20);
        }
        s.signals = self.signals();
        self.score = s;
    }

    fn signals(&self) -> Vec<Signal> {
        let mut out = Vec::new();
        if self.git.dirty > 0 {
            out.push(Signal::warn(format!(
                "{} uncommitted changes",
                self.git.dirty
            )));
        }
        match self.git.last_commit_age_days {
            Some(days) if days > 90 => out.push(Signal::bad(format!("quiet for {days} days"))),
            Some(days) if days <= 14 => out.push(Signal::good("active recently")),
            _ => {}
        }
        if self.profile.expectations.documentation && !self.docs.readme {
            out.push(Signal::bad("README missing"));
        }
        if self.profile.expectations.ci && !self.delivery.ci {
            out.push(Signal::warn("no CI workflow"));
        }
        if self.profile.expectations.tests && !self.delivery.tests {
            out.push(Signal::warn("no test suite detected"));
        }
        if let Some(cairn) = &self.cairn {
            if cairn.blocked > 0 {
                out.push(Signal::bad(format!(
                    "{} Cairn items blocked",
                    cairn.blocked
                )));
            }
            if cairn.active > 0 {
                out.push(Signal::info(format!("{} items in progress", cairn.active)));
            }
        } else if self.profile.expectations.planning {
            out.push(Signal::warn("no Cairn roadmap"));
        }
        match self.site.healthy {
            Some(true) => out.push(Signal::good("deployment responding")),
            Some(false) => out.push(Signal::bad("deployment unhealthy")),
            None if self.site.source && self.profile.expectations.deployment => {
                out.push(Signal::info("site not checked yet"));
            }
            None => {}
        }
        if let Some(error) = &self.profile.config_error {
            out.push(Signal::bad(format!("invalid .jerk.toml: {error}")));
        }
        if self.git.ahead > 0 {
            out.push(Signal::warn(format!(
                "{} commits not pushed",
                self.git.ahead
            )));
        }
        if out.is_empty() {
            out.push(Signal::good("no obvious risks"));
        }
        out.truncate(6);
        out
    }

    pub fn grade(&self) -> &'static str {
        match self.score.total {
            90..=100 => "exceptional",
            75..=89 => "shipping",
            60..=74 => "healthy",
            40..=59 => "forming",
            _ => "early",
        }
    }
}

impl Signal {
    fn good(label: impl Into<String>) -> Self {
        Self {
            kind: SignalKind::Good,
            label: label.into(),
        }
    }

    fn warn(label: impl Into<String>) -> Self {
        Self {
            kind: SignalKind::Warn,
            label: label.into(),
        }
    }

    fn bad(label: impl Into<String>) -> Self {
        Self {
            kind: SignalKind::Bad,
            label: label.into(),
        }
    }

    fn info(label: impl Into<String>) -> Self {
        Self {
            kind: SignalKind::Info,
            label: label.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_is_bounded_and_rewards_project_hygiene() {
        let mut project = Project {
            docs: DocsStats {
                readme: true,
                license: true,
                contributing: true,
                changelog: true,
                docs_dir: true,
                api_docs: true,
            },
            delivery: DeliveryStats {
                ci: true,
                tests: true,
                package: true,
                release_automation: true,
                dependency_updates: true,
            },
            ..Project::default()
        };
        project.git.commits = 50;
        project.git.commits_30d = 12;
        project.git.contributors = 3;
        project.git.tags = 4;
        project.git.last_commit_age_days = Some(1);
        project.remote_slug = Some("oddurs/jerk".into());
        project.description = Some("dashboard".into());
        project.cairn = Some(CairnStats {
            total: 10,
            done: 8,
            active: 1,
            completion: 80,
            ..CairnStats::default()
        });
        project.github = Some(GithubStats {
            merged_prs: 4,
            latest_release: Some("v0.1".into()),
            ci_state: Some("SUCCESS".into()),
            ..GithubStats::default()
        });
        project.site.healthy = Some(true);
        project.recalculate_score();
        assert!(project.score.total >= 90);
        assert!(project.score.total <= 100);
    }
}
