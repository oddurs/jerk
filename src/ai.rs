use std::time::Duration;

#[cfg(target_os = "macos")]
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::model::Project;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "dev.jerk.openrouter";
const DEFAULT_MODEL: &str = "openrouter/free";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialSource {
    Environment,
    Keychain,
}

impl CredentialSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Keychain => "macOS Keychain",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct AiSnapshot(Value);

impl AiSnapshot {
    pub fn from_project(project: &Project) -> Self {
        let github = project.github.as_ref().map(|stats| {
            json!({
                "open_pull_requests": stats.open_prs,
                "merged_pull_requests": stats.merged_prs,
                "closed_pull_requests": stats.closed_prs,
                "open_issues": stats.open_issues,
                "stars": stats.stars,
                "forks": stats.forks,
                "unique_views_14d": stats.unique_views_14d,
                "unique_clones_14d": stats.unique_clones_14d,
                "release_downloads": stats.release_downloads,
                "has_release": stats.latest_release.is_some(),
                "ci_state": stats.ci_state,
            })
        });
        let planning = project.cairn.as_ref().map(|cairn| {
            json!({
                "total": cairn.total,
                "open": cairn.open,
                "active": cairn.active,
                "blocked": cairn.blocked,
                "done": cairn.done,
                "dropped": cairn.dropped,
                "milestones": cairn.milestones,
                "completion_percent": cairn.completion,
            })
        });
        Self(json!({
            "schema": 1,
            "project": {
                "name": project.name,
                "kind": project.profile.kind.label(),
                "lifecycle": project.profile.lifecycle.label(),
                "tier": project.profile.tier.label(),
                "profile_declared": matches!(
                    project.profile.source,
                    crate::model::ProfileSource::Declared
                ),
            },
            "effectiveness": {
                "total": project.score.total,
                "confidence": project.score.confidence,
                "shape": { "value": project.score.local, "maximum": 15 },
                "documentation": { "value": project.score.documentation, "maximum": 15 },
                "momentum": { "value": project.score.momentum, "maximum": 20 },
                "delivery": { "value": project.score.delivery, "maximum": 20 },
                "planning": { "value": project.score.planning, "maximum": 20 },
                "reach": { "value": project.score.community, "maximum": 10 },
            },
            "git": {
                "dirty_files": project.git.dirty,
                "ahead": project.git.ahead,
                "behind": project.git.behind,
                "commits_total": project.git.commits,
                "commits_30d": project.git.commits_30d,
                "contributors": project.git.contributors,
                "branches": project.git.branches,
                "tags": project.git.tags,
                "tracked_files": project.git.tracked_files,
                "last_commit_age_days": project.git.last_commit_age_days,
                "additions_30d": project.git.additions_30d,
                "deletions_30d": project.git.deletions_30d,
            },
            "documentation": {
                "readme": project.docs.readme,
                "license": project.docs.license,
                "docs_directory": project.docs.docs_dir,
                "api_docs": project.docs.api_docs,
                "contributing": project.docs.contributing,
                "changelog": project.docs.changelog,
            },
            "delivery": {
                "ci": project.delivery.ci,
                "tests": project.delivery.tests,
                "package": project.delivery.package,
                "release_automation": project.delivery.release_automation,
                "dependency_updates": project.delivery.dependency_updates,
                "site_source": project.site.source,
                "site_deployed": project.site.deployed,
                "site_healthy": project.site.healthy,
                "site_status_code": project.site.status_code,
                "site_latency_ms": project.site.latency_ms,
            },
            "planning": planning,
            "github": github,
        }))
    }

    pub fn compact_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }

    pub fn fingerprint(&self) -> String {
        // Stable FNV-1a is sufficient for invalidating an in-memory cache.
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in self.compact_json().bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        format!("{hash:016x}")
    }
}

#[derive(Clone, Debug)]
pub struct AiReport {
    pub summary: String,
    pub observations: Vec<String>,
    pub risks: Vec<String>,
    pub next_action: String,
    pub model: String,
    pub total_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct GeneratedReport {
    summary: String,
    observations: Vec<String>,
    risks: Vec<String>,
    next_action: String,
}

pub trait AnalysisProvider {
    fn analyze(&self, snapshot: &AiSnapshot) -> Result<AiReport, String>;
}

pub struct OpenRouter {
    key: String,
    model: String,
}

impl OpenRouter {
    pub fn load() -> Result<Self, String> {
        let (key, _) = load_key().ok_or_else(|| missing_key_message().to_string())?;
        let model = std::env::var("OPENROUTER_MODEL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.into());
        Ok(Self { key, model })
    }

    pub fn model() -> String {
        std::env::var("OPENROUTER_MODEL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.into())
    }
}

impl AnalysisProvider for OpenRouter {
    fn analyze(&self, snapshot: &AiSnapshot) -> Result<AiReport, String> {
        let request = json!({
            "model": self.model,
            "temperature": 0.2,
            // This is a small extraction task. Disabling extended reasoning
            // preserves the budget for the structured answer on free models.
            "max_completion_tokens": 1200,
            "reasoning": { "enabled": false, "exclude": true },
            "provider": { "require_parameters": true },
            "messages": [
                {
                    "role": "system",
                    "content": "Return the requested JSON immediately. You are a concise software portfolio analyst. Analyze only the supplied metrics. Never claim to have read source code. Separate observations from risks and give one concrete, high-leverage next action. Do not invent facts."
                },
                {
                    "role": "user",
                    "content": snapshot.compact_json()
                }
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "project_effectiveness_brief",
                    "strict": true,
                    "schema": {
                        "type": "object",
                        "properties": {
                            "summary": { "type": "string" },
                            "observations": {
                                "type": "array",
                                "items": { "type": "string" },
                                "minItems": 1,
                                "maxItems": 4
                            },
                            "risks": {
                                "type": "array",
                                "items": { "type": "string" },
                                "minItems": 1,
                                "maxItems": 4
                            },
                            "next_action": { "type": "string" }
                        },
                        "required": ["summary", "observations", "risks", "next_action"],
                        "additionalProperties": false
                    }
                }
            }
        });
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(60)))
            .build();
        let agent: ureq::Agent = config.into();
        let response = agent
            .post(OPENROUTER_URL)
            .header("Authorization", &format!("Bearer {}", self.key))
            .header("HTTP-Referer", "https://github.com/oddurs/jerk")
            .header("X-Title", "jerk")
            .send_json(&request)
            .map_err(openrouter_error)?;
        let value: Value = response
            .into_body()
            .read_json()
            .map_err(|_| "OpenRouter returned unreadable JSON".to_string())?;
        let content = value
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .filter(|content| !content.trim().is_empty())
            .ok_or_else(|| missing_content_error(&value))?;
        let generated: GeneratedReport = serde_json::from_str(content)
            .map_err(|_| "OpenRouter returned an invalid structured analysis".to_string())?;
        Ok(AiReport {
            summary: bounded(generated.summary, 600),
            observations: bounded_list(generated.observations),
            risks: bounded_list(generated.risks),
            next_action: bounded(generated.next_action, 500),
            model: value
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(&self.model)
                .to_string(),
            total_tokens: value.pointer("/usage/total_tokens").and_then(Value::as_u64),
        })
    }
}

fn missing_content_error(response: &Value) -> String {
    if let Some(message) = response.pointer("/error/message").and_then(Value::as_str) {
        return bounded(format!("OpenRouter: {message}"), 180);
    }
    match response
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
    {
        Some("length") => "OpenRouter exhausted its response budget; try again".into(),
        Some("content_filter") => "OpenRouter declined to analyze these metrics".into(),
        _ => "OpenRouter returned an empty analysis; try again".into(),
    }
}

pub fn credential_source() -> Option<CredentialSource> {
    load_key().map(|(_, source)| source)
}

#[cfg(target_os = "macos")]
pub const fn missing_key_message() -> &'static str {
    "OpenRouter key missing; set OPENROUTER_API_KEY or configure the Keychain"
}

#[cfg(not(target_os = "macos"))]
pub const fn missing_key_message() -> &'static str {
    "OpenRouter key missing; set OPENROUTER_API_KEY"
}

fn load_key() -> Option<(String, CredentialSource)> {
    if let Ok(key) = std::env::var("OPENROUTER_API_KEY")
        && !key.trim().is_empty()
    {
        return Some((key, CredentialSource::Environment));
    }
    keychain_key().map(|key| (key, CredentialSource::Keychain))
}

#[cfg(target_os = "macos")]
fn keychain_key() -> Option<String> {
    let output = Command::new("security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let key = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!key.is_empty()).then_some(key)
}

#[cfg(not(target_os = "macos"))]
fn keychain_key() -> Option<String> {
    None
}

fn openrouter_error(error: ureq::Error) -> String {
    match error {
        ureq::Error::StatusCode(401 | 403) => "OpenRouter rejected the API key".into(),
        ureq::Error::StatusCode(402) => "OpenRouter credits are exhausted".into(),
        ureq::Error::StatusCode(429) => "OpenRouter rate limit reached; try again later".into(),
        ureq::Error::StatusCode(status) => format!("OpenRouter returned HTTP {status}"),
        _ => "Could not reach OpenRouter".into(),
    }
}

fn bounded(value: String, maximum: usize) -> String {
    let mut chars = value.trim().chars();
    let mut output = chars.by_ref().take(maximum).collect::<String>();
    if chars.next().is_some() {
        output.push('…');
    }
    output
}

fn bounded_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .take(4)
        .map(|value| bounded(value, 280))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::model::{CairnStats, Project};

    use super::AiSnapshot;

    #[test]
    fn outbound_snapshot_excludes_paths_urls_and_file_content() {
        let project = Project {
            name: "safe-name".into(),
            path: "/secret/local/path".into(),
            description: Some("SENSITIVE_DESCRIPTION".into()),
            remote_slug: Some("private-owner/private-repo".into()),
            cairn: Some(CairnStats {
                active_items: vec!["SENSITIVE_CAIRN_TITLE".into()],
                ..CairnStats::default()
            }),
            ..Project::default()
        };
        let json = AiSnapshot::from_project(&project).compact_json();

        assert!(json.contains("safe-name"));
        assert!(!json.contains("/secret/local/path"));
        assert!(!json.contains("SENSITIVE_DESCRIPTION"));
        assert!(!json.contains("private-owner"));
        assert!(!json.contains("SENSITIVE_CAIRN_TITLE"));
    }

    #[test]
    fn snapshot_fingerprint_changes_with_metrics() {
        let first = Project::default();
        let mut second = Project::default();
        second.git.commits_30d = 1;

        assert_ne!(
            AiSnapshot::from_project(&first).fingerprint(),
            AiSnapshot::from_project(&second).fingerprint()
        );
    }
}
