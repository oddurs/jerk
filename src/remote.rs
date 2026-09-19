use std::path::PathBuf;
use std::process::Command;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::model::{GithubStats, Project};

#[derive(Debug)]
pub struct RemoteUpdate {
    pub path: PathBuf,
    pub github: Option<GithubStats>,
    pub site_url: Option<String>,
    pub site_status: Option<(u16, u64)>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct GithubPortfolio {
    pub owner: String,
    pub total: usize,
    pub public: usize,
    pub private: usize,
    pub archived: usize,
    pub active_30d: usize,
    pub stale_1y_unarchived: usize,
    pub public_missing_description: usize,
    pub public_missing_license: usize,
    pub public_missing_topics: usize,
    pub with_releases: usize,
    pub with_homepage: usize,
}

pub fn portfolio(owner: &str) -> Result<GithubPortfolio, String> {
    let output = Command::new("gh")
        .args([
            "repo",
            "list",
            owner,
            "--limit",
            "500",
            "--source",
            "--json",
            "description,isPrivate,isArchived,pushedAt,homepageUrl,licenseInfo,repositoryTopics,latestRelease",
        ])
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_HTTP_TIMEOUT", "12")
        .output()
        .map_err(|_| "GitHub CLI is not installed".to_string())?;
    if !output.status.success() {
        return Err("GitHub portfolio unavailable".into());
    }
    let repos: Vec<Value> = serde_json::from_slice(&output.stdout)
        .map_err(|_| "GitHub returned invalid portfolio data".to_string())?;
    let now = Utc::now();
    let mut stats = GithubPortfolio {
        owner: owner.into(),
        total: repos.len(),
        ..Default::default()
    };
    for repo in repos {
        let private = repo
            .get("isPrivate")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let archived = repo
            .get("isArchived")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        stats.private += usize::from(private);
        stats.public += usize::from(!private);
        stats.archived += usize::from(archived);
        stats.with_releases += usize::from(repo.get("latestRelease").is_some_and(|v| !v.is_null()));
        stats.with_homepage += usize::from(
            repo.get("homepageUrl")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty()),
        );
        if !private {
            stats.public_missing_description += usize::from(
                repo.get("description")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty),
            );
            stats.public_missing_license +=
                usize::from(repo.get("licenseInfo").is_none_or(Value::is_null));
            stats.public_missing_topics += usize::from(
                repo.get("repositoryTopics")
                    .and_then(Value::as_array)
                    .is_none_or(Vec::is_empty),
            );
        }
        let age = repo
            .get("pushedAt")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|pushed| {
                now.signed_duration_since(pushed.with_timezone(&Utc))
                    .num_days()
            });
        stats.active_30d += usize::from(age.is_some_and(|days| days <= 30));
        stats.stale_1y_unarchived += usize::from(!archived && age.is_some_and(|days| days > 365));
    }
    Ok(stats)
}

pub fn enrich(project: &Project) -> RemoteUpdate {
    let mut update = RemoteUpdate {
        path: project.path.clone(),
        github: None,
        site_url: project.site.url.clone(),
        site_status: None,
        error: None,
    };
    let Some(slug) = project.remote_slug.as_deref() else {
        update.error = Some("no GitHub remote".into());
        return update;
    };
    let Some((owner, name)) = slug.split_once('/') else {
        update.error = Some("unrecognized GitHub remote".into());
        return update;
    };

    match github(owner, name) {
        Ok((stats, homepage)) => {
            update.github = Some(stats);
            if update.site_url.is_none() {
                update.site_url = homepage.filter(|url| !url.contains("github.com/"));
            }
        }
        Err(error) => update.error = Some(error),
    }

    if update.site_url.is_none() {
        update.site_url = deployment_url(slug).or_else(|| pages_url(slug));
    }
    if let Some(url) = update.site_url.as_deref() {
        update.site_status = health(url);
    }
    update
}

fn github(owner: &str, name: &str) -> Result<(GithubStats, Option<String>), String> {
    const QUERY: &str = r#"query($owner:String!,$name:String!){repository(owner:$owner,name:$name){stargazerCount forkCount homepageUrl pullRequests(states:OPEN){totalCount} merged:pullRequests(states:MERGED){totalCount} closed:pullRequests(states:CLOSED){totalCount} issues(states:OPEN){totalCount} releases(first:1,orderBy:{field:CREATED_AT,direction:DESC}){nodes{tagName}} defaultBranchRef{target{... on Commit{statusCheckRollup{state}}}}}}"#;
    let output = Command::new("gh")
        .args([
            "api",
            "graphql",
            "-f",
            &format!("owner={owner}"),
            "-f",
            &format!("name={name}"),
            "-f",
            &format!("query={QUERY}"),
        ])
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_HTTP_TIMEOUT", "8")
        .output()
        .map_err(|_| "GitHub CLI is not installed".to_string())?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        return Err(if message.contains("auth") || message.contains("login") {
            "run `gh auth login` for PR and issue stats".into()
        } else {
            "GitHub stats unavailable".into()
        });
    }
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "GitHub returned invalid data".to_string())?;
    let repo = value
        .pointer("/data/repository")
        .ok_or_else(|| "GitHub repository not found".to_string())?;
    let count = |pointer: &str| {
        repo.pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize
    };
    let mut stats = GithubStats {
        open_prs: count("/pullRequests/totalCount"),
        merged_prs: count("/merged/totalCount"),
        closed_prs: count("/closed/totalCount"),
        open_issues: count("/issues/totalCount"),
        stars: count("/stargazerCount"),
        forks: count("/forkCount"),
        latest_release: repo
            .pointer("/releases/nodes/0/tagName")
            .and_then(Value::as_str)
            .map(str::to_owned),
        ci_state: repo
            .pointer("/defaultBranchRef/target/statusCheckRollup/state")
            .and_then(Value::as_str)
            .map(str::to_owned),
        ..GithubStats::default()
    };
    let slug = format!("{owner}/{name}");
    stats.unique_views_14d = gh_api(&format!("repos/{slug}/traffic/views"))
        .and_then(|value| value.get("uniques")?.as_u64())
        .map(|value| value as usize);
    stats.unique_clones_14d = gh_api(&format!("repos/{slug}/traffic/clones"))
        .and_then(|value| value.get("uniques")?.as_u64())
        .map(|value| value as usize);
    stats.release_downloads = gh_api(&format!("repos/{slug}/releases?per_page=20")).map(|value| {
        value
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|release| {
                release
                    .get("assets")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
            })
            .filter_map(|asset| asset.get("download_count").and_then(Value::as_u64))
            .sum::<u64>() as usize
    });
    stats.latest_workflow =
        gh_api(&format!("repos/{slug}/actions/runs?per_page=1")).and_then(|value| {
            let run = value.pointer("/workflow_runs/0")?;
            let name = run.get("name")?.as_str()?;
            let state = run
                .get("conclusion")
                .and_then(Value::as_str)
                .or_else(|| run.get("status").and_then(Value::as_str))?;
            Some(format!("{state} · {name}"))
        });
    let homepage = repo
        .get("homepageUrl")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok((stats, homepage))
}

fn deployment_url(slug: &str) -> Option<String> {
    let deployments = gh_api(&format!("repos/{slug}/deployments?per_page=1"))?;
    let id = deployments.get(0)?.get("id")?.as_u64()?;
    let statuses = gh_api(&format!(
        "repos/{slug}/deployments/{id}/statuses?per_page=1"
    ))?;
    statuses
        .get(0)?
        .get("environment_url")?
        .as_str()
        .filter(|url| !url.is_empty())
        .map(str::to_owned)
}

fn pages_url(slug: &str) -> Option<String> {
    gh_api(&format!("repos/{slug}/pages"))?
        .get("html_url")?
        .as_str()
        .map(str::to_owned)
}

fn gh_api(endpoint: &str) -> Option<Value> {
    let output = Command::new("gh")
        .args(["api", "--cache", "5m", endpoint])
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_HTTP_TIMEOUT", "8")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| serde_json::from_slice(&output.stdout).ok())
        .flatten()
}

fn health(url: &str) -> Option<(u16, u64)> {
    let null_device = if cfg!(target_os = "windows") {
        "NUL"
    } else {
        "/dev/null"
    };
    let output = Command::new("curl")
        .args(["-L", "-sS", "-o", null_device, "--max-time", "6"])
        .args(["-w", "%{http_code} %{time_total}", url])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut parts = text.split_whitespace();
    let status = parts.next()?.parse::<u16>().ok()?;
    let seconds = parts.next()?.parse::<f64>().ok()?;
    Some((status, (seconds * 1000.0) as u64))
}
