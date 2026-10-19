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
    fn workspace_path_dependencies_produce_no_version_finding() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.dependencies]\nshared = { path = \"crates/shared\" }\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("crates/app")).unwrap();
        std::fs::write(
            dir.path().join("crates/app/Cargo.toml"),
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nshared = { workspace = true }\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("crates/shared")).unwrap();
        std::fs::write(
            dir.path().join("crates/shared/Cargo.toml"),
            "[package]\nname = \"shared\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "").unwrap();

        // Non-vacuous guard: the members and the inherited dep must actually
        // be detected, otherwise "no finding" below would prove nothing.
        let context = context(dir.path());
        let stacks = StackDetector::default().detect(&context).unwrap();
        assert_eq!(stacks.len(), 2, "raiz virtual expande os membros");
        let app = stacks
            .iter()
            .find(|s| s.name.as_deref() == Some("app"))
            .unwrap();
        assert_eq!(app.dependencies.len(), 1);
        assert!(app.dependencies[0].local, "dep herdada com path é local");

        let report = DependencyAnalyzer
            .analyze(&context, &stacks, &AnalysisOptions::default())
            .unwrap();
        assert!(
            !report
                .findings
                .iter()
                .any(|f| f.message.contains("declares no version")),
            "path-dep de workspace não é 'sem versão': {:?}",
            report.findings
        );
    }

    #[test]
    fn non_local_dependency_without_version_is_still_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"solo\"\nversion = \"0.1.0\"\n\n[dependencies]\nmystery = { workspace = true }\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "").unwrap();

        let report = analyze(dir.path());
        assert!(
            report.findings.iter().any(|f| f.message
                .contains("'mystery' declares no version")),
            "a regra continua pegando dep sem versão que NÃO é local: {:?}",
            report.findings
        );
    }

    #[test]
    fn workspace_path_dependency_is_not_a_trust_risk() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("crates/app")).unwrap();
        std::fs::create_dir_all(dir.path().join("crates/shared")).unwrap();
        std::fs::write(
            dir.path().join("crates/app/Cargo.toml"),
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nshared = { path = \"../shared\" }\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("crates/shared/Cargo.toml"),
            "[package]\nname = \"shared\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let context = context(dir.path());
        let stacks = StackDetector::default().detect(&context).unwrap();
        let report = DependencyAnalyzer.verify_trust(&context, &stacks).unwrap();

        assert!(
            report.risks.is_empty(),
            "código próprio do workspace nunca vira risco de trust (sem rede): {:?}",
            report.risks
        );
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
    fn workspace_member_lockfile_at_root_is_accepted() {
        // Dogfooding #14: when analyzing a single workspace member, the
        // governing Cargo.lock lives at the workspace root, not in the member.
        // The rule walks up only while ancestors declare `[workspace]`, so the
        // root lockfile satisfies the member.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "").unwrap();
        std::fs::create_dir_all(dir.path().join("crates/app")).unwrap();
        std::fs::write(
            dir.path().join("crates/app/Cargo.toml"),
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n",
        )
        .unwrap();

        // Non-vacuous: analyze the MEMBER dir, which has no lockfile of its
        // own; the governing Cargo.lock sits at the workspace root.
        let member = dir.path().join("crates/app");
        let report = analyze(&member);
        assert!(
            !report
                .findings
                .iter()
                .any(|f| f.message.contains("no lockfile found")),
            "membro de workspace aceita o Cargo.lock da raiz: {:?}",
            report.findings
        );
    }

    #[test]
    fn nested_non_workspace_project_still_warns_about_lockfile() {
        // The walk stops at the first ancestor that does NOT declare
        // `[workspace]`: a nested project under a plain outer package must NOT
        // borrow the outer lockfile, so the reproducibility warning still fires.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"outer\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "").unwrap();
        std::fs::create_dir_all(dir.path().join("nested")).unwrap();
        std::fs::write(
            dir.path().join("nested/Cargo.toml"),
            "[package]\nname = \"nested\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n",
        )
        .unwrap();

        let nested = dir.path().join("nested");
        let report = analyze(&nested);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.message.contains("no lockfile found")),
            "projeto aninhado fora de workspace ainda avisa sem lockfile: {:?}",
            report.findings
        );
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

    #[test]
    fn rust_git_dependency_is_flagged_as_non_registry_in_trust() {
        // Regression for dogfooding #13: a Rust `git = …` dep used to reach
        // the analyzer with `version: None` (the origin was dropped in the
        // detector), so it was classified Missing and the trust "outside the
        // registry" rule — which only fires for NonRegistry — never saw it.
        // The detector now records `git+<url>`, so trust flags it.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"gitdep\"\nversion = \"0.1.0\"\n\n[dependencies]\ngitty = { git = \"https://example.com/gitty.git\" }\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "").unwrap();

        let context = context(dir.path());
        let stacks = StackDetector::default().detect(&context).unwrap();
        // Non-vacuous: the git dep must actually be detected as non-registry.
        let gitty = stacks[0]
            .dependencies
            .iter()
            .find(|d| d.name == "gitty")
            .expect("gitty listado");
        assert_eq!(
            gitty.version.as_deref(),
            Some("git+https://example.com/gitty.git"),
            "detector preserva a origem git no campo version"
        );

        let report = DependencyAnalyzer
            .verify_trust(&context, &stacks)
            .unwrap();
        assert!(
            report
                .risks
                .iter()
                .any(|r| r.message.contains("comes from outside the registry")
                    && r.package.as_deref() == Some("gitty")),
            "git-dep do Rust agora vira risco de trust: {:?}",
            report.risks
        );
    }

    #[test]
    fn rust_git_dependency_reports_origin_not_missing_version() {
        // Same root cause as #13, seen in `analyze`: the finding must name the
        // git origin, never the misleading "declares no version".
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"gitdep\"\nversion = \"0.1.0\"\n\n[dependencies]\ngitty = { git = \"https://example.com/gitty.git\" }\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "").unwrap();

        let report = analyze(dir.path());
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.message.contains("outside the package registry")
                    && f.message.contains("git+https://example.com/gitty.git")),
            "analyze aponta a origem git: {:?}",
            report.findings
        );
        assert!(
            !report
                .findings
                .iter()
                .any(|f| f.message.contains("declares no version")),
            "git-dep NUNCA e reportado como 'sem versao': {:?}",
            report.findings
        );
    }
}
