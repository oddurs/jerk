use std::collections::VecDeque;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::{CairnStats, DeliveryStats, DocsStats, GitStats, Project, SiteStats};
use crate::schema;

pub fn discover(root: &Path) -> Vec<PathBuf> {
    if is_repo(root) {
        return vec![root.to_path_buf()];
    }

    let mut repos = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name();
            let visible = !name.to_string_lossy().starts_with('.');
            (visible && path.is_dir() && is_repo(&path)).then_some(path)
        })
        .collect::<Vec<_>>();
    repos.sort_by_key(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });
    repos
}

fn is_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

pub fn scan_all(root: &Path) -> Vec<Project> {
    let repos = discover(root);
    if repos.len() < 2 {
        return repos.into_iter().map(|path| scan_project(&path)).collect();
    }
    let count = repos.len();
    let jobs = Arc::new(Mutex::new(
        repos.into_iter().enumerate().collect::<VecDeque<_>>(),
    ));
    let (tx, rx) = mpsc::channel();
    std::thread::scope(|scope| {
        let workers = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(4)
            .min(8)
            .min(count);
        for _ in 0..workers {
            let jobs = Arc::clone(&jobs);
            let tx = tx.clone();
            scope.spawn(move || {
                loop {
                    let job = jobs.lock().ok().and_then(|mut queue| queue.pop_front());
                    let Some((index, path)) = job else { break };
                    if tx.send((index, scan_project(&path))).is_err() {
                        break;
                    }
                }
            });
        }
    });
    drop(tx);
    let mut projects = rx.into_iter().collect::<Vec<_>>();
    projects.sort_by_key(|(index, _)| *index);
    projects.into_iter().map(|(_, project)| project).collect()
}

pub fn scan_project(path: &Path) -> Project {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let metadata = metadata(path);
    let remote = git(path, &["remote", "get-url", "origin"]);
    let remote_slug = remote.as_deref().and_then(github_slug);
    let tracked = git(path, &["ls-files"])
        .map(|output| output.lines().map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut project = Project {
        name,
        path: path.to_path_buf(),
        description: metadata.description,
        remote_slug,
        git: git_stats(path, &tracked),
        docs: docs_stats(path),
        delivery: delivery_stats(path, &tracked),
        site: site_stats(path, metadata.homepage, &tracked),
        cairn: cairn_stats(path),
        ..Project::default()
    };
    project.profile = schema::load(&project);
    project.recalculate_score();
    project
}

fn git_stats(path: &Path, tracked: &[String]) -> GitStats {
    let branch = git(path, &["branch", "--show-current"])
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "detached".into());
    let dirty = git(path, &["status", "--porcelain"])
        .map(|s| s.lines().count())
        .unwrap_or(0);
    let (behind, ahead) = git(
        path,
        &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
    )
    .and_then(|s| {
        let mut values = s.split_whitespace().filter_map(|n| n.parse::<usize>().ok());
        Some((values.next()?, values.next()?))
    })
    .unwrap_or_default();
    let commits = git_count(path, &["rev-list", "--count", "HEAD"]);
    let commits_30d = git_count(path, &["rev-list", "--count", "--since=30.days", "HEAD"]);
    let contributors = git(path, &["shortlog", "-sne", "HEAD"])
        .map(|s| s.lines().filter(|line| !line.trim().is_empty()).count())
        .unwrap_or(0);
    let branches = git(
        path,
        &["for-each-ref", "--format=%(refname)", "refs/heads/"],
    )
    .map(|s| s.lines().count())
    .unwrap_or(0);
    let tags = git(path, &["tag", "--list"])
        .map(|s| s.lines().count())
        .unwrap_or(0);
    let timestamp = git(path, &["log", "-1", "--format=%ct"]).and_then(|s| s.parse::<u64>().ok());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let last_commit_age_days = timestamp.map(|then| now.saturating_sub(then) / 86_400);
    let last_commit_date = git(path, &["log", "-1", "--date=short", "--format=%cd"]);
    let last_subject = git(path, &["log", "-1", "--format=%s"]);
    let (additions_30d, deletions_30d) = churn(path);
    let activity = activity(path, now);

    GitStats {
        branch,
        dirty,
        ahead,
        behind,
        commits,
        commits_30d,
        contributors,
        branches,
        tags,
        tracked_files: tracked.len(),
        languages: languages(tracked),
        additions_30d,
        deletions_30d,
        last_commit_age_days,
        last_commit_date,
        last_subject,
        activity,
    }
}

fn languages(paths: &[String]) -> Vec<(String, usize)> {
    let mut counts = HashMap::<&'static str, usize>::new();
    for path in paths {
        let extension = Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let language = match extension.as_str() {
            "rs" => "Rust",
            "ts" | "tsx" => "TypeScript",
            "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
            "py" => "Python",
            "go" => "Go",
            "c" | "h" => "C",
            "cc" | "cpp" | "cxx" | "hpp" => "C++",
            "swift" => "Swift",
            "rb" => "Ruby",
            "java" => "Java",
            "kt" | "kts" => "Kotlin",
            "sh" | "bash" | "zsh" => "Shell",
            "html" | "css" | "scss" | "sass" => "Web",
            _ => continue,
        };
        *counts.entry(language).or_default() += 1;
    }
    let mut languages: Vec<(String, usize)> = counts
        .into_iter()
        .map(|(name, count)| (name.into(), count))
        .collect::<Vec<_>>();
    languages.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    languages.truncate(4);
    languages
}

fn git_count(path: &Path, args: &[&str]) -> usize {
    git(path, args)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or_default()
}

fn git(path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn churn(path: &Path) -> (usize, usize) {
    let Some(output) = git(path, &["log", "--since=30.days", "--numstat", "--format="]) else {
        return (0, 0);
    };
    output.lines().fold((0, 0), |(add, del), line| {
        let mut fields = line.split('\t');
        let a = fields
            .next()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let d = fields
            .next()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        (add + a, del + d)
    })
}

fn activity(path: &Path, now: u64) -> Vec<u64> {
    let mut weeks = vec![0; 12];
    let Some(output) = git(path, &["log", "--since=84.days", "--format=%ct"]) else {
        return weeks;
    };
    for stamp in output.lines().filter_map(|line| line.parse::<u64>().ok()) {
        let age = now.saturating_sub(stamp) / (7 * 86_400);
        if age < 12 {
            weeks[11 - age as usize] += 1;
        }
    }
    weeks
}

fn docs_stats(path: &Path) -> DocsStats {
    DocsStats {
        readme: has_prefix(path, "readme"),
        license: has_prefix(path, "license") || has_prefix(path, "copying"),
        contributing: has_prefix(path, "contributing"),
        changelog: has_prefix(path, "changelog") || has_prefix(path, "changes"),
        docs_dir: path.join("docs").is_dir() || path.join("doc").is_dir(),
        api_docs: path.join("rustdoc.toml").exists()
            || path.join("mkdocs.yml").exists()
            || path.join("docs/conf.py").exists()
            || path.join("typedoc.json").exists(),
    }
}

fn has_prefix(path: &Path, prefix: &str) -> bool {
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .to_lowercase()
                .starts_with(prefix)
        })
}

fn delivery_stats(path: &Path, tracked: &[String]) -> DeliveryStats {
    let github = path.join(".github/workflows");
    let gitlab = path.join(".gitlab-ci.yml");
    let tests = ["tests", "test", "spec", "__tests__"]
        .iter()
        .any(|name| path.join(name).is_dir())
        || tracked.iter().any(|file| is_test_file(file))
        || package_has_test_script(path)
        || has_inline_tests(path, tracked);
    let package = [
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "Gemfile",
        "Makefile",
    ]
    .iter()
    .any(|name| path.join(name).exists());
    let release_automation = github.is_dir()
        && fs::read_dir(&github)
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                name.contains("release") || name.contains("publish") || name.contains("deploy")
            });
    DeliveryStats {
        ci: github.is_dir() || gitlab.exists() || path.join(".circleci").is_dir(),
        tests,
        package,
        release_automation,
        dependency_updates: path.join(".github/dependabot.yml").exists()
            || path.join("renovate.json").exists()
            || path.join("renovate.json5").exists(),
    }
}

fn is_test_file(file: &str) -> bool {
    let lower = file.to_ascii_lowercase();
    lower.contains("/tests/")
        || lower.contains("/__tests__/")
        || lower.ends_with("_test.go")
        || lower.ends_with("_test.py")
        || lower.ends_with(".test.ts")
        || lower.ends_with(".test.tsx")
        || lower.ends_with(".test.js")
        || lower.ends_with(".spec.ts")
        || lower.ends_with(".spec.js")
}

fn package_has_test_script(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path.join("package.json")) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    value
        .pointer("/scripts/test")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|script| !script.contains("no test specified") && !script.trim().is_empty())
}

fn has_inline_tests(path: &Path, tracked: &[String]) -> bool {
    tracked
        .iter()
        .filter(|file| file.ends_with(".rs"))
        .take(256)
        .any(|file| {
            fs::read_to_string(path.join(file))
                .ok()
                .is_some_and(|source| source.contains("#[cfg(test)]"))
        })
}

fn site_stats(path: &Path, homepage: Option<String>, tracked: &[String]) -> SiteStats {
    let dedicated_directory = ["site", "www", "website", "public"]
        .iter()
        .any(|name| path.join(name).is_dir());
    let site_config = [
        "vercel.json",
        "netlify.toml",
        "astro.config.mjs",
        "next.config.js",
        "next.config.mjs",
        "mkdocs.yml",
    ]
    .iter()
    .any(|name| path.join(name).exists());
    let pages_evidence = tracked.iter().any(|file| {
        let lower = file.to_ascii_lowercase();
        lower == "docs/cname"
            || lower == "docs/index.html"
            || (lower.starts_with(".github/workflows/")
                && (lower.contains("pages") || lower.contains("deploy")))
    });
    let web_package = fs::read_to_string(path.join("package.json"))
        .ok()
        .is_some_and(|text| {
            [
                "\"next\"",
                "\"vite\"",
                "\"astro\"",
                "\"gatsby\"",
                "\"sveltekit\"",
            ]
            .iter()
            .any(|needle| text.contains(needle))
        });
    let source = dedicated_directory || site_config || pages_evidence || web_package;
    SiteStats {
        source,
        url: homepage.filter(|url| !is_repository_url(url)),
        ..SiteStats::default()
    }
}

fn is_repository_url(url: &str) -> bool {
    url.contains("github.com/") || url.contains("gitlab.com/")
}

#[derive(Default)]
struct Metadata {
    description: Option<String>,
    homepage: Option<String>,
}

fn metadata(path: &Path) -> Metadata {
    let cargo = path.join("Cargo.toml");
    if let Ok(text) = fs::read_to_string(cargo) {
        if let Ok(value) = text.parse::<toml::Value>()
            && let Some(package) = value.get("package")
        {
            return Metadata {
                description: package
                    .get("description")
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned),
                homepage: package
                    .get("homepage")
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned),
            };
        }
        // Cargo accepts new manifest syntax before the generic TOML parser in
        // an older jerk necessarily does. These two scalar fields are stable.
        let package = section_scalars(&text, "package");
        if !package.is_empty() {
            return Metadata {
                description: package.get("description").cloned(),
                homepage: package.get("homepage").cloned(),
            };
        }
    }
    let package = path.join("package.json");
    if let Ok(text) = fs::read_to_string(package)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
    {
        return Metadata {
            description: value
                .get("description")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            homepage: value
                .get("homepage")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        };
    }
    Metadata::default()
}

fn section_scalars(text: &str, wanted: &str) -> HashMap<String, String> {
    let mut in_section = false;
    let mut values = HashMap::new();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or_default().trim();
        if line.starts_with('[') {
            in_section = line == format!("[{wanted}]");
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches(['"', '\'']);
        if !value.is_empty() {
            values.insert(key.trim().to_string(), value.to_string());
        }
    }
    values
}

pub fn github_slug(remote: &str) -> Option<String> {
    let remote = remote.trim().trim_end_matches('/').trim_end_matches(".git");
    let path = if let Some(rest) = remote.strip_prefix("git@github.com:") {
        rest
    } else if let Some(rest) = remote.strip_prefix("ssh://git@github.com/") {
        rest
    } else if let Some(rest) = remote.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = remote.strip_prefix("http://github.com/") {
        rest
    } else {
        return None;
    };
    (path.split('/').count() == 2).then(|| path.to_string())
}

fn cairn_stats(path: &Path) -> Option<CairnStats> {
    let text = fs::read_to_string(path.join("cairn.toml")).ok()?;
    let (item_dir, statuses) = cairn_config(&text);

    let mut stats = CairnStats::default();
    let mut milestones = HashSet::new();
    for entry in fs::read_dir(path.join(&item_dir))
        .into_iter()
        .flatten()
        .flatten()
    {
        let item_path = entry.path();
        if item_path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Ok(item) = fs::read_to_string(item_path) else {
            continue;
        };
        let fields = frontmatter(&item);
        let status = fields.get("status").map(String::as_str).unwrap_or("open");
        let category = statuses.get(status).map(String::as_str).unwrap_or(status);
        let title = fields
            .get("title")
            .cloned()
            .unwrap_or_else(|| "Untitled".into());
        stats.total += 1;
        match category {
            "done" => stats.done += 1,
            "dropped" => stats.dropped += 1,
            "active" => {
                stats.active += 1;
                if stats.active_items.len() < 5 {
                    stats.active_items.push(title);
                }
            }
            _ => {
                stats.open += 1;
                if stats.next_items.len() < 5 {
                    stats.next_items.push(title);
                }
            }
        }
        if status.to_lowercase().contains("block") {
            stats.blocked += 1;
        }
        if let Some(milestone) = fields.get("milestone").filter(|m| !m.is_empty()) {
            milestones.insert(milestone.clone());
        }
    }
    stats.milestones = milestones.len();
    let decided = stats.total.saturating_sub(stats.dropped);
    stats.completion = if decided == 0 {
        0
    } else {
        ((stats.done * 100) / decided) as u16
    };
    Some(stats)
}

/// Read only the two pieces the dashboard needs. Cairn deliberately permits
/// newer schema constructs than the TOML crate linked into an older jerk may
/// know about, so a narrow line reader is more forward-compatible than
/// rejecting the entire project configuration.
fn cairn_config(text: &str) -> (String, HashMap<String, String>) {
    enum Section {
        Other,
        Project,
        Status,
    }

    let mut section = Section::Other;
    let mut item_dir = "cairn/items".to_string();
    let mut statuses = HashMap::new();
    let mut status_name: Option<String> = None;
    let mut status_category: Option<String> = None;
    let commit_status = |name: &mut Option<String>,
                         category: &mut Option<String>,
                         statuses: &mut HashMap<String, String>| {
        if let (Some(name), Some(category)) = (name.take(), category.take()) {
            statuses.insert(name, category);
        }
    };

    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or_default().trim();
        match line {
            "[project]" => {
                commit_status(&mut status_name, &mut status_category, &mut statuses);
                section = Section::Project;
                continue;
            }
            "[[status]]" => {
                commit_status(&mut status_name, &mut status_category, &mut statuses);
                section = Section::Status;
                continue;
            }
            _ if line.starts_with('[') => {
                commit_status(&mut status_name, &mut status_category, &mut statuses);
                section = Section::Other;
                continue;
            }
            _ => {}
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches(['"', '\'']).to_string();
        match (&section, key.trim()) {
            (Section::Project, "dir") => item_dir = value,
            (Section::Status, "name") => status_name = Some(value),
            (Section::Status, "category") => status_category = Some(value),
            _ => {}
        }
    }
    commit_status(&mut status_name, &mut status_category, &mut statuses);
    (item_dir, statuses)
}

fn frontmatter(text: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return fields;
    }
    for line in lines {
        let line = line.trim();
        if line == "---" {
            break;
        }
        if line.starts_with([' ', '-']) {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim().trim_matches(['"', '\'']);
            fields.insert(key.trim().to_string(), value.to_string());
        }
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_github_slugs_from_common_remotes() {
        assert_eq!(
            github_slug("git@github.com:oddurs/jerk.git"),
            Some("oddurs/jerk".into())
        );
        assert_eq!(
            github_slug("https://github.com/oddurs/jerk.git"),
            Some("oddurs/jerk".into())
        );
        assert_eq!(github_slug("https://example.com/oddurs/jerk"), None);
    }

    #[test]
    fn reads_the_yaml_subset_cairn_uses() {
        let fields = frontmatter("---\nid: 4\ntitle: Ship it\nstatus: doing\n---\nBody");
        assert_eq!(fields.get("title").map(String::as_str), Some("Ship it"));
        assert_eq!(fields.get("status").map(String::as_str), Some("doing"));
    }

    #[test]
    fn reads_cairn_status_categories_without_owning_the_schema() {
        let text = r#"
            [project]
            dir = "work/items"
            [[status]]
            name = "doing"
            category = "active"
            [[status]]
            name = "done"
            category = "done"
        "#;
        let (dir, statuses) = cairn_config(text);
        assert_eq!(dir, "work/items");
        assert_eq!(statuses.get("doing").map(String::as_str), Some("active"));
    }

    #[test]
    fn language_shape_ignores_data_and_markup_noise() {
        let paths = vec![
            "src/main.rs".into(),
            "src/lib.rs".into(),
            "scripts/release.py".into(),
            "README.md".into(),
            "fixtures/data.json".into(),
        ];
        assert_eq!(
            languages(&paths),
            vec![("Rust".into(), 2), ("Python".into(), 1)]
        );
    }
}
