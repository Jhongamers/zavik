//! **Analyzer** plugin: analyzes a project's dependencies.
//!
//! Combines two layers:
//! - [`offline`]: local heuristics (always run);
//! - [`registry`]: queries registries for newer versions (opt-in).

mod offline;
mod registry;
mod trust;
mod version;

use zavik_shared::{AnalysisReport, Context, Error, ProjectStack, TrustReport};
use zavik_sdk::{AnalysisOptions, Capability, ProjectAnalyzer};
use std::any::Any;

/// Plugin that analyzes dependencies.
#[derive(Debug, Default)]
pub struct DependencyAnalyzer;

impl Capability for DependencyAnalyzer {
    fn name(&self) -> &'static str {
        "dependency_analyzer"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_analyzer(&self) -> Option<&dyn ProjectAnalyzer> {
        Some(self)
    }
}

impl ProjectAnalyzer for DependencyAnalyzer {
    fn analyze(
        &self,
        context: &Context,
        stacks: &[ProjectStack],
        options: &AnalysisOptions,
    ) -> Result<AnalysisReport, Error> {
        let mut report = AnalysisReport::default();
        offline::analyze(&context.project_root, stacks, &mut report);
        if options.check_latest {
            registry::check_latest(stacks, &mut report);
        }
        Ok(report)
    }

    fn verify_trust(
        &self,
        _context: &Context,
        stacks: &[ProjectStack],
    ) -> Result<TrustReport, Error> {
        let mut report = TrustReport::default();
        trust::verify(stacks, &mut report);
        Ok(report)
    }
}

/// Classification of the source/quality of a declared version.
pub(crate) enum SourceKind {
    /// Came from a normal package registry.
    Registry,
    /// Wildcard / unpinned version (`*`, `latest`, empty, ...).
    Wildcard(String),
    /// No version declared.
    Missing,
    /// Not from the registry (git, path, file, url, ...).
    NonRegistry(String),
}

impl SourceKind {
    pub(crate) fn classify(version: Option<&str>) -> Self {
        let Some(raw) = version else {
            return SourceKind::Missing;
        };

        let value = raw.trim();
        let lower = value.to_ascii_lowercase();
        let is_non_registry = lower.contains("://")
            || lower.starts_with("git+")
            || lower.starts_with("git:")
            || lower.starts_with("file:")
            || lower.starts_with("path:")
            || lower.starts_with("link:")
            || lower.starts_with("workspace:")
            || lower.starts_with("github:");

        if is_non_registry {
            SourceKind::NonRegistry(value.to_string())
        } else if value.is_empty() || lower.contains('*') || matches!(lower.as_str(), "latest" | "x")
        {
            SourceKind::Wildcard(if value.is_empty() {
                "*".to_string()
            } else {
                value.to_string()
            })
        } else {
            SourceKind::Registry
        }
    }
}

// ---------------------------------------------------------------------------
// Online enrichment for `audit` (npm registry)
// ---------------------------------------------------------------------------

/// Registry signals that enrich the `audit` snapshots.
#[derive(Debug, Clone, Default)]
pub struct NpmAuditSignal {
    /// Concatenated text of the install scripts (pre/install/postinstall/prepare).
    pub install_script: Option<String>,
    pub downloads_last_week: Option<u64>,
}

/// Queries the npm registry to enrich the audit snapshots (online mode).
///
/// `deps` = (name, version requirement). Resolves the target (exact version or
/// `latest`), fetches the install scripts and downloads/week. Limits the
/// number of requests ([`MAX_CHECKS`]). Returns a lowercase(name) → signal map.
pub fn npm_audit_signals(deps: &[(String, Option<String>)]) -> std::collections::HashMap<String, NpmAuditSignal> {
    let agent = registry::agent();
    let mut out = std::collections::HashMap::new();

    for (name, requirement) in deps.iter().take(registry::MAX_CHECKS) {
        let target = version::exact_version(requirement.as_deref()).unwrap_or_else(|| "latest".into());
        let mut signal = NpmAuditSignal::default();

        let scripts = registry::npm_install_scripts(&agent, name, &target);
        if !scripts.is_empty() {
            signal.install_script = Some(
                scripts
                    .iter()
                    .map(|(hook, command)| format!("{hook}: {command}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        signal.downloads_last_week = registry::npm_downloads_last_week(&agent, name);
        out.insert(name.to_lowercase(), signal);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StackDetector;
    use zavik_shared::Severity;
    use zavik_sdk::{AnalysisOptions, ProjectAnalyzer, ProjectDetector};
    use std::path::Path;

    fn context(dir: &Path) -> Context {
        Context {
            project_root: dir.to_path_buf(),
        }
    }

    fn analyze(dir: &Path) -> AnalysisReport {
        let context = context(dir);
        let stacks = StackDetector::default()
            .detect(&context)
            .expect("detecção deveria funcionar");
        DependencyAnalyzer
            .analyze(&context, &stacks, &AnalysisOptions::default())
            .expect("análise deveria funcionar")
    }

    #[test]
    fn detects_wildcard_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[dependencies]\nserde = \"*\"\n").unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "").unwrap();

        let report = analyze(dir.path());
        assert!(report
            .findings
            .iter()
            .any(|f| f.severity == Severity::Warning && f.message.contains("no pinned version")));
    }

    #[test]
    fn detects_missing_lockfile() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{ "dependencies": { "react": "^18.0.0" } }"#,
        )
        .unwrap();

        let report = analyze(dir.path());
        assert!(report
            .findings
            .iter()
            .any(|f| f.message.contains("lockfile")));
    }

    #[test]
    fn detects_dependency_outside_registry() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{ "dependencies": { "meu-pacote": "git+https://github.com/user/repo.git" } }"#,
        )
        .unwrap();

        let report = analyze(dir.path());
        assert!(report
            .findings
            .iter()
            .any(|f| f.message.contains("outside the package registry")));
    }

    #[test]
    fn healthy_project_produces_no_warnings() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"ok\"\n\n[dependencies]\nserde = \"1\"\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "").unwrap();

        let report = analyze(dir.path());
        assert_eq!(report.count(Severity::Warning), 0);
        assert_eq!(report.count(Severity::Critical), 0);
    }

    #[test]
    fn detects_duplicate_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{
                "dependencies": { "react": "17.0.0" },
                "devDependencies": { "react": "18.0.0" }
            }"#,
        )
        .unwrap();

        let report = analyze(dir.path());
        assert!(report
            .findings
            .iter()
            .any(|f| f.message.contains("different versions")));
    }

    #[test]
    fn classifies_sources() {
        assert!(matches!(SourceKind::classify(None), SourceKind::Missing));
        assert!(matches!(
            SourceKind::classify(Some("*")),
            SourceKind::Wildcard(_)
        ));
        assert!(matches!(
            SourceKind::classify(Some("^1.2.3")),
            SourceKind::Registry
        ));
        assert!(matches!(
            SourceKind::classify(Some("git+https://x/y.git")),
            SourceKind::NonRegistry(_)
        ));
    }

    #[test]
    fn detects_install_execution_script() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{ "scripts": { "postinstall": "node ./suspeito.js" } }"#,
        )
        .unwrap();

        let context = context(dir.path());
        let stacks = StackDetector::default().detect(&context).unwrap();
        let report = DependencyAnalyzer
            .verify_trust(&context, &stacks)
            .unwrap();

        assert!(report.risks.iter().any(|r| {
            r.severity == Severity::Critical && r.detail.contains("suspeito.js")
        }));
    }

    #[test]
    fn project_without_scripts_is_clean_on_verification() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{ "name": "ok" }"#).unwrap();

        let context = context(dir.path());
        let stacks = StackDetector::default().detect(&context).unwrap();
        let report = DependencyAnalyzer
            .verify_trust(&context, &stacks)
            .unwrap();

        assert!(report.blocking().is_empty());
    }
}
