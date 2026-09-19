use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app::{ProjectSort, Tab};

const EXAMPLE: &str = r#"# Personal jerk preferences. This file is never read from a repository.

[dashboard]
# The two words in the top-left chrome.
brand = "JERK"
workspace = "project pulse"

# Used when no directory is passed on the command line.
# root = "~/Code"

# overview | git | delivery | cairn | portfolio | insight
default_view = "overview"
# name | attention | recent
default_sort = "name"

# Optional: pin the GitHub owner used by the portfolio view.
# github_owner = "your-handle"

# ANSI role used for selection and identity emphasis. The terminal still
# controls the actual color. Try: blue, cyan, green, yellow, magenta, red.
accent = "blue"
"#;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Settings {
    #[serde(default)]
    pub dashboard: DashboardSettings,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DashboardSettings {
    #[serde(default = "default_brand")]
    pub brand: String,
    #[serde(default = "default_workspace")]
    pub workspace: String,
    pub root: Option<String>,
    #[serde(default = "default_view")]
    pub default_view: String,
    #[serde(default = "default_sort")]
    pub default_sort: String,
    pub github_owner: Option<String>,
    #[serde(default = "default_accent")]
    pub accent: String,
}

impl Default for DashboardSettings {
    fn default() -> Self {
        Self {
            brand: default_brand(),
            workspace: default_workspace(),
            root: None,
            default_view: default_view(),
            default_sort: default_sort(),
            github_owner: None,
            accent: default_accent(),
        }
    }
}

impl DashboardSettings {
    pub fn tab(&self) -> Tab {
        match self.default_view.to_ascii_lowercase().as_str() {
            "git" => Tab::Git,
            "delivery" => Tab::Delivery,
            "cairn" => Tab::Cairn,
            "portfolio" => Tab::Portfolio,
            "insight" | "ai" => Tab::Insight,
            _ => Tab::Overview,
        }
    }

    pub fn sort(&self) -> ProjectSort {
        match self.default_sort.to_ascii_lowercase().as_str() {
            "attention" | "needs-attention" | "needs_attention" => ProjectSort::NeedsAttention,
            "recent" | "recency" => ProjectSort::Recent,
            _ => ProjectSort::Name,
        }
    }

    pub fn root_path(&self) -> Option<PathBuf> {
        self.root.as_deref().map(expand_path)
    }
}

pub fn load(explicit: Option<&Path>) -> (Settings, Option<PathBuf>, Option<String>) {
    let path = explicit.map(Path::to_path_buf).or_else(config_path);
    let Some(path) = path else {
        return (Settings::default(), None, None);
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return (Settings::default(), Some(path), None);
    };
    match toml::from_str::<Settings>(&text) {
        Ok(settings) => (settings, Some(path), None),
        Err(error) => (
            Settings::default(),
            Some(path.clone()),
            Some(format!("could not read {}: {error}", path.display())),
        ),
    }
}

pub fn write_example(explicit: Option<&Path>) -> std::io::Result<PathBuf> {
    let path = explicit
        .map(Path::to_path_buf)
        .or_else(config_path)
        .ok_or_else(|| std::io::Error::other("cannot determine a user config directory"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("{} already exists", path.display()),
        ));
    }
    fs::write(&path, EXAMPLE)?;
    Ok(path)
}

pub fn config_path() -> Option<PathBuf> {
    if let Some(path) = env::var_os("JERK_CONFIG") {
        return Some(PathBuf::from(path));
    }
    #[cfg(target_os = "macos")]
    {
        home_dir().map(|home| home.join("Library/Application Support/jerk/config.toml"))
    }
    #[cfg(target_os = "windows")]
    {
        env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("jerk/config.toml"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|home| home.join(".config")))
            .map(|path| path.join("jerk/config.toml"))
    }
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn expand_path(value: &str) -> PathBuf {
    if value == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(value));
    }
    if let Some(rest) = value.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(value)
}

fn default_brand() -> String {
    "JERK".into()
}

fn default_workspace() -> String {
    "project pulse".into()
}

fn default_view() -> String {
    "overview".into()
}

fn default_sort() -> String {
    "name".into()
}

fn default_accent() -> String {
    "blue".into()
}

#[cfg(test)]
mod tests {
    use super::{DashboardSettings, Settings, expand_path};

    #[test]
    fn defaults_are_stable_and_personal_fields_parse() {
        let settings: Settings = toml::from_str(
            r#"[dashboard]
brand = "ODDURS"
workspace = "studio"
default_view = "portfolio"
default_sort = "attention"
accent = "cyan"
"#,
        )
        .unwrap_or_default();
        assert_eq!(settings.dashboard.brand, "ODDURS");
        assert_eq!(settings.dashboard.tab().label(), "Portfolio");
        assert_eq!(settings.dashboard.sort().label(), "needs attention");
    }

    #[test]
    fn tilde_paths_expand() {
        let path = expand_path("~/Code");
        assert!(path.ends_with("Code"));
    }

    #[test]
    fn default_dashboard_is_usable() {
        let dashboard = DashboardSettings::default();
        assert_eq!(dashboard.tab().label(), "Overview");
        assert_eq!(dashboard.sort().label(), "name");
    }
}
