use std::fs;
use std::path::{Component, Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde_yaml::Value;

use crate::{BrainRecord, CoreError, EgressPolicy, KairosConfig, Result, RouteResult};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccessDisposition {
    Allowed,
    ExplicitRequestRequired,
    Denied,
}

#[derive(Clone, Debug)]
pub struct AccessGrant {
    pub brain_id: String,
    pub relative_path: String,
    pub purpose: String,
}

/// The boundary across which approved note contents may travel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentDestination {
    LocalOllama,
    ExternalMcp,
}

fn normalized_relative_path(path: &str) -> Result<PathBuf> {
    let path = Path::new(path);
    if path.is_absolute() || path.as_os_str().is_empty() {
        return Err(CoreError::InvalidPath(
            "path must be a non-empty relative path".to_owned(),
        ));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(CoreError::InvalidPath(format!(
            "parent or absolute components are not allowed: {}",
            path.display()
        )));
    }
    Ok(path.to_path_buf())
}

fn matches_any(path: &str, patterns: &[String]) -> Result<bool> {
    if patterns.is_empty() {
        return Ok(false);
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern).map_err(|error| {
            CoreError::InvalidPath(format!("invalid policy glob {pattern:?}: {error}"))
        })?);
    }
    let set: GlobSet = builder
        .build()
        .map_err(|error| CoreError::InvalidPath(format!("invalid policy glob set: {error}")))?;
    Ok(set.is_match(path))
}

fn frontmatter(text: &str) -> Option<&str> {
    let remaining = text.strip_prefix("---\n")?;
    let end = remaining.find("\n---")?;
    Some(&remaining[..end])
}

fn yaml_tags_contain_private(value: &Value) -> bool {
    let Some(tags) = value.get("tags") else {
        return false;
    };
    match tags {
        Value::String(tag) => tag.eq_ignore_ascii_case("private"),
        Value::Sequence(tags) => tags.iter().any(|tag| {
            tag.as_str()
                .is_some_and(|tag| tag.eq_ignore_ascii_case("private"))
        }),
        _ => false,
    }
}

fn content_requires_explicit_access(text: &str) -> bool {
    if text.contains("#private") {
        return true;
    }
    let Some(frontmatter) = frontmatter(text) else {
        return false;
    };
    let Ok(metadata) = serde_yaml::from_str::<Value>(frontmatter) else {
        return false;
    };
    metadata
        .get("agent_access")
        .and_then(Value::as_str)
        .is_some_and(|access| access.eq_ignore_ascii_case("explicit_only"))
        || yaml_tags_contain_private(&metadata)
}

fn has_matching_grant(brain: &BrainRecord, path: &str, grants: &[AccessGrant]) -> bool {
    grants.iter().any(|grant| {
        grant.brain_id == brain.id
            && grant.relative_path == path
            && !grant.purpose.trim().is_empty()
    })
}

/// Check a configured startup path before resolving or reading it. An empty
/// `startup_allow` list deliberately means no startup files are approved.
pub fn preflight_startup_access(
    brain: &BrainRecord,
    relative_path: &str,
    grants: &[AccessGrant],
) -> Result<AccessDisposition> {
    let relative_path = normalized_relative_path(relative_path)?;
    let relative = relative_path.to_string_lossy().replace('\\', "/");

    if matches_any(&relative, &brain.read_policy.deny_patterns)? {
        return Ok(AccessDisposition::Denied);
    }
    if !matches_any(&relative, &brain.read_policy.startup_allow)? {
        return Ok(AccessDisposition::Denied);
    }
    if matches_any(&relative, &brain.read_policy.explicit_only_patterns)?
        && !has_matching_grant(brain, &relative, grants)
    {
        return Ok(AccessDisposition::ExplicitRequestRequired);
    }
    Ok(AccessDisposition::Allowed)
}

/// Decide whether text can be surfaced. `visibility: private` is intentionally
/// not an access block: it is a public-export boundary, not an agent boundary.
pub fn evaluate_access(
    brain: &BrainRecord,
    relative_path: &str,
    text: &str,
    grants: &[AccessGrant],
) -> Result<AccessDisposition> {
    let relative_path = normalized_relative_path(relative_path)?;
    let relative = relative_path.to_string_lossy().replace('\\', "/");

    if matches_any(&relative, &brain.read_policy.deny_patterns)? {
        return Ok(AccessDisposition::Denied);
    }
    let requires_explicit = matches_any(&relative, &brain.read_policy.explicit_only_patterns)?
        || content_requires_explicit_access(text);
    if requires_explicit && !has_matching_grant(brain, &relative, grants) {
        return Ok(AccessDisposition::ExplicitRequestRequired);
    }
    Ok(AccessDisposition::Allowed)
}

/// Enforce the registered brain's egress rule before note contents leave the
/// local Kairos process. MCP has no trustworthy way to establish per-turn user
/// consent in this alpha, so callers must pass `false` for it.
pub fn enforce_content_egress(
    config: &KairosConfig,
    route: &RouteResult,
    destination: ContentDestination,
    user_confirmed: bool,
) -> Result<()> {
    if destination == ContentDestination::LocalOllama {
        return Ok(());
    }

    for routed in &route.brains {
        let brain = config
            .brains
            .iter()
            .find(|brain| brain.id == routed.id)
            .ok_or_else(|| CoreError::BrainNotFound(routed.id.clone()))?;
        match brain.egress_policy {
            EgressPolicy::LocalOnly => {
                return Err(CoreError::PolicyDenied(
                    "a local_only brain cannot send note contents through an external MCP host"
                        .to_owned(),
                ));
            }
            EgressPolicy::CloudAllowed if !user_confirmed => {
                return Err(CoreError::PolicyDenied(
                    "external note-content egress requires explicit per-turn user confirmation"
                        .to_owned(),
                ));
            }
            EgressPolicy::RedactRequired => {
                return Err(CoreError::PolicyDenied(
                    "external note-content egress requires redaction, which this alpha does not implement"
                        .to_owned(),
                ));
            }
            EgressPolicy::CloudAllowed => {}
        }
    }
    Ok(())
}

/// Resolve and canonicalize a Markdown file while rejecting path traversal and
/// symlink escapes outside the registered brain root.
pub fn canonicalize_allowed_file(brain: &BrainRecord, relative_path: &str) -> Result<PathBuf> {
    let relative_path = normalized_relative_path(relative_path)?;
    if relative_path
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("md")
    {
        return Err(CoreError::PolicyDenied(
            "Kairos alpha only reads Markdown notes".to_owned(),
        ));
    }
    let root = fs::canonicalize(&brain.root_path).map_err(|error| {
        CoreError::InvalidPath(format!(
            "cannot access registered root {}: {error}",
            brain.root_path.display()
        ))
    })?;
    let candidate = root.join(relative_path);
    let canonical = fs::canonicalize(&candidate).map_err(|error| {
        CoreError::InvalidPath(format!("cannot read {}: {error}", candidate.display()))
    })?;
    if !canonical.starts_with(&root) {
        return Err(CoreError::PolicyDenied(format!(
            "symlink escape outside registered root: {}",
            canonical.display()
        )));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BrainRole, EgressPolicy, KairosConfig, ReadPolicy, RouteResult, RoutedBrain, WritePolicy,
    };

    fn brain() -> BrainRecord {
        BrainRecord {
            id: "test".to_owned(),
            name: "Test".to_owned(),
            role: BrainRole::Other,
            root_path: std::env::temp_dir(),
            router_paths: Vec::new(),
            context_paths: Vec::new(),
            enabled: true,
            read_policy: ReadPolicy {
                explicit_only_patterns: vec!["90_Private/**".to_owned()],
                deny_patterns: vec![".git/**".to_owned()],
                ..Default::default()
            },
            egress_policy: EgressPolicy::LocalOnly,
            write_policy: WritePolicy::ReadOnly,
        }
    }

    #[test]
    fn protected_frontmatter_is_withheld_without_a_grant() {
        let result = evaluate_access(
            &brain(),
            "notes/secret.md",
            "---\nagent_access: explicit_only\n---\nprivate text",
            &[],
        )
        .unwrap();
        assert_eq!(result, AccessDisposition::ExplicitRequestRequired);
    }

    #[test]
    fn public_visibility_does_not_block_local_reading() {
        let result = evaluate_access(
            &brain(),
            "notes/context.md",
            "---\nvisibility: private\n---\ncontext",
            &[],
        )
        .unwrap();
        assert_eq!(result, AccessDisposition::Allowed);
    }

    #[test]
    fn parent_paths_are_rejected() {
        let error = canonicalize_allowed_file(&brain(), "../outside.md").unwrap_err();
        assert!(error.to_string().contains("parent"));
    }

    #[test]
    fn startup_allowlist_fails_closed() {
        let result = preflight_startup_access(&brain(), "notes/context.md", &[]).unwrap();
        assert_eq!(result, AccessDisposition::Denied);
    }

    #[test]
    fn external_egress_rejects_local_only_brains() {
        let brain = brain();
        let config = KairosConfig {
            version: 1,
            source_router_path: std::env::temp_dir().join("router.md"),
            brains: vec![brain],
        };
        let route = RouteResult {
            query: "test".to_owned(),
            brains: vec![RoutedBrain {
                id: "test".to_owned(),
                name: "Test".to_owned(),
                score: 1,
                reason: "test".to_owned(),
            }],
            requires_choice: false,
            unavailable_brains: Vec::new(),
        };
        let error = enforce_content_egress(&config, &route, ContentDestination::ExternalMcp, false)
            .unwrap_err();
        assert!(error.to_string().contains("local_only"));
    }
}
