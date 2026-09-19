use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::model::{
    Expectations, Healthcheck, Lifecycle, ProfileSource, Project, ProjectComponent, ProjectKind,
    ProjectProfile, Tier,
};

pub const VERSION: u16 = 1;

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    schema: Option<u16>,
    #[serde(default)]
    project: ProjectConfig,
    #[serde(default)]
    expect: ExpectConfig,
    #[serde(default, rename = "component")]
    components: Vec<ProjectComponent>,
    #[serde(default, rename = "healthcheck")]
    healthchecks: Vec<Healthcheck>,
}

#[derive(Debug, Default, Deserialize)]
struct ProjectConfig {
    kind: Option<ProjectKind>,
    lifecycle: Option<Lifecycle>,
    tier: Option<Tier>,
    intent: Option<String>,
    #[serde(default)]
    outcomes: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ExpectConfig {
    documentation: Option<bool>,
    tests: Option<bool>,
    ci: Option<bool>,
    packaging: Option<bool>,
    releases: Option<bool>,
    site: Option<bool>,
    deployment: Option<bool>,
    planning: Option<bool>,
    collaboration: Option<bool>,
}

pub fn load(project: &Project) -> ProjectProfile {
    let mut profile = infer(project);
    let config_path = project.path.join(".jerk.toml");
    let Ok(text) = fs::read_to_string(config_path) else {
        return profile;
    };
    let config = match toml::from_str::<FileConfig>(&text) {
        Ok(config) => config,
        Err(error) => {
            profile.config_error = Some(error.to_string());
            return profile;
        }
    };
    if config.schema.unwrap_or(VERSION) != VERSION {
        profile.config_error = Some(format!(
            "unsupported .jerk.toml schema {}",
            config.schema.unwrap_or_default()
        ));
        return profile;
    }

    profile.source = ProfileSource::Declared;
    if let Some(kind) = config.project.kind {
        profile.kind = kind;
    }
    if let Some(lifecycle) = config.project.lifecycle {
        profile.lifecycle = lifecycle;
    }
    if let Some(tier) = config.project.tier {
        profile.tier = tier;
    }
    profile.intent = config.project.intent.or(profile.intent);
    profile.outcomes = config.project.outcomes;
    profile.components = config.components;
    profile.healthchecks = config.healthchecks;
    merge_expectations(&mut profile.expectations, config.expect);
    profile
}

fn infer(project: &Project) -> ProjectProfile {
    let kind = infer_kind(project);
    let lifecycle = infer_lifecycle(project);
    let tier = Tier::Experimental;
    ProjectProfile {
        kind,
        lifecycle,
        tier,
        source: ProfileSource::Inferred,
        intent: project.description.clone(),
        expectations: expectations(kind, lifecycle, tier, project.cairn.is_some()),
        ..ProjectProfile::default()
    }
}

fn infer_kind(project: &Project) -> ProjectKind {
    let name = project.name.to_lowercase();
    let description = project
        .description
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    let cargo = read(&project.path.join("Cargo.toml"));
    let package = read(&project.path.join("package.json"));

    if name.starts_with("homebrew-") || name.starts_with("scoop-") {
        ProjectKind::Distribution
    } else if name.contains("theme") || description.contains("color scheme") {
        ProjectKind::Theme
    } else if cargo.contains("ratatui") || description.contains("terminal dashboard") {
        ProjectKind::Tui
    } else if package.contains("\"next\"")
        || package.contains("\"vite\"")
        || package.contains("\"svelte")
        || package.contains("\"astro\"")
    {
        ProjectKind::Web
    } else if description.contains("simulation") || description.contains("modelled") {
        ProjectKind::Simulation
    } else if description.contains("research") || description.contains("measure of") {
        ProjectKind::Research
    } else if description.contains("art") || description.contains("interactive essay") {
        ProjectKind::Creative
    } else if description.contains("config") || name.contains("dotfiles") {
        ProjectKind::Configuration
    } else if project.path.join("src/lib.rs").exists() && !project.path.join("src/main.rs").exists()
    {
        ProjectKind::Library
    } else if description.contains("cli") || cargo.contains("[[bin]]") {
        ProjectKind::Cli
    } else if project.site.source {
        ProjectKind::Web
    } else if project.delivery.package {
        ProjectKind::Application
    } else {
        ProjectKind::Unknown
    }
}

fn infer_lifecycle(project: &Project) -> Lifecycle {
    if project.git.commits < 5 && project.git.tags == 0 {
        return Lifecycle::Incubating;
    }
    match project.git.last_commit_age_days {
        Some(0..=30) => Lifecycle::Active,
        Some(31..=120) => Lifecycle::Maintenance,
        Some(121..=365) => Lifecycle::Paused,
        Some(_) => Lifecycle::Paused,
        None => Lifecycle::Incubating,
    }
}

fn expectations(
    kind: ProjectKind,
    lifecycle: Lifecycle,
    tier: Tier,
    has_cairn: bool,
) -> Expectations {
    let code = !matches!(
        kind,
        ProjectKind::Creative
            | ProjectKind::Configuration
            | ProjectKind::Distribution
            | ProjectKind::Learning
            | ProjectKind::Unknown
    );
    let releases = matches!(
        kind,
        ProjectKind::Cli
            | ProjectKind::Tui
            | ProjectKind::Library
            | ProjectKind::Application
            | ProjectKind::Theme
    );
    let site = matches!(kind, ProjectKind::Web);
    let active = matches!(lifecycle, Lifecycle::Incubating | Lifecycle::Active);
    Expectations {
        documentation: !matches!(kind, ProjectKind::Configuration),
        tests: code,
        ci: code,
        packaging: code,
        releases,
        site,
        deployment: site,
        planning: active && has_cairn,
        collaboration: matches!(tier, Tier::Flagship | Tier::Supported),
    }
}

fn merge_expectations(target: &mut Expectations, source: ExpectConfig) {
    macro_rules! merge {
        ($field:ident) => {
            if let Some(value) = source.$field {
                target.$field = value;
            }
        };
    }
    merge!(documentation);
    merge!(tests);
    merge!(ci);
    merge!(packaging);
    merge!(releases);
    merge!(site);
    merge!(deployment);
    merge!(planning);
    merge!(collaboration);
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GitStats, Project};
    use tempfile::tempdir;

    #[test]
    fn recent_ratatui_projects_infer_as_active_tuis() {
        let project = Project {
            name: "reader".into(),
            description: Some("A terminal dashboard".into()),
            git: GitStats {
                commits: 12,
                last_commit_age_days: Some(2),
                ..Default::default()
            },
            ..Default::default()
        };
        let profile = infer(&project);
        assert_eq!(profile.kind, ProjectKind::Tui);
        assert_eq!(profile.lifecycle, Lifecycle::Active);
        assert!(profile.expectations.tests);
        assert!(!profile.expectations.site);
    }

    #[test]
    fn committed_intent_overrides_inference() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        fs::write(
            directory.path().join(".jerk.toml"),
            r#"
                schema = 1
                [project]
                kind = "creative"
                lifecycle = "complete"
                tier = "personal"
                intent = "Exist beautifully."
                [expect]
                tests = false
                ci = false
            "#,
        )?;
        let project = Project {
            path: directory.path().into(),
            ..Default::default()
        };
        let profile = load(&project);
        assert_eq!(profile.source, ProfileSource::Declared);
        assert_eq!(profile.kind, ProjectKind::Creative);
        assert_eq!(profile.lifecycle, Lifecycle::Complete);
        assert!(!profile.expectations.tests);
        assert_eq!(profile.intent.as_deref(), Some("Exist beautifully."));
        Ok(())
    }
}
