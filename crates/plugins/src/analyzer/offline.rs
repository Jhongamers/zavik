//! **Offline** analysis: heuristics that don't depend on the network.
//!
//! Rules:
//! - wildcard / unpinned version (`*`, `latest`, empty, ...);
//! - dependency with no declared version;
//! - dependency coming from outside the registry (git/path/file/...);
//! - missing lockfile (build not reproducible);
//! - same dependency declared with different versions in the same stack.
//!
//! Local (`path = …`) dependencies are exempt from the version rules: they
//! resolve from this project's own source tree, so there is no registry
//! version to declare or pin — reporting one would be noise (dogfooding on
//! Zavik itself reported 16 such findings for its own crates).

use super::SourceKind;
use zavik_shared::{AnalysisReport, Finding, Language, ProjectStack};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

/// Runs all the offline rules.
pub(crate) fn analyze(root: &Path, stacks: &[ProjectStack], report: &mut AnalysisReport) {
    for stack in stacks {
        check_lockfile(root, stack, report);
        for dependency in &stack.dependencies {
            if dependency.local {
                // First-party path/workspace-member code: no registry
                // version applies, so none of the version rules below do.
                continue;
            }
            check_dependency(stack, dependency, report);
        }
        check_duplicate_versions(stack, report);
    }
}

fn check_dependency(
    stack: &ProjectStack,
    dependency: &zavik_shared::Dependency,
    report: &mut AnalysisReport,
) {
    let name = &dependency.name;
    match SourceKind::classify(dependency.version.as_deref()) {
        SourceKind::NonRegistry(source) => report.push(Finding::info(format!(
            "{}: '{name}' comes from '{source}' (outside the package registry).",
            stack.language
        ))),
        SourceKind::Wildcard(value) => report.push(Finding::warning(
            name.clone(),
            format!(
                "{}: '{name}' has no pinned version ({value}). Pin a version for reproducible builds.",
                stack.language
            ),
        )),
        SourceKind::Missing => report.push(Finding::info(format!(
            "{}: '{name}' declares no version.",
            stack.language
        ))),
        SourceKind::Registry => {}
    }
}

fn check_duplicate_versions(stack: &ProjectStack, report: &mut AnalysisReport) {
    let mut versions: HashMap<String, BTreeSet<String>> = HashMap::new();
    for dependency in &stack.dependencies {
        if let SourceKind::Registry = SourceKind::classify(dependency.version.as_deref()) {
            if let Some(version) = &dependency.version {
                versions
                    .entry(normalize(&dependency.name))
                    .or_default()
                    .insert(version.trim().to_string());
            }
        }
    }

    for (name, found) in versions {
        if found.len() > 1 {
            report.push(Finding::warning(
                name.clone(),
                format!(
                    "{}: '{name}' appears with different versions: {}.",
                    stack.language,
                    found.into_iter().collect::<Vec<_>>().join(", ")
                ),
            ));
        }
    }
}

/// Whether a lockfile is present for `root`, looking only where Cargo would
/// actually place one. For a plain package that is `root` itself. When `root`
/// is inside a workspace (or is the workspace root), Cargo keeps a single
/// `Cargo.lock` at the workspace root, so we walk up — but *only* while each
/// ancestor `Cargo.toml` still declares `[workspace]`. The moment an ancestor
/// is not a workspace root we stop, so a nested package that merely sits
/// under an unrelated directory keeps its honest "no lockfile" warning
/// instead of borrowing a lockfile that doesn't govern it.
fn lockfile_present(language: Language, root: &Path, expected: &[&str]) -> bool {
    if expected.iter().any(|name| root.join(name).is_file()) {
        return true;
    }
    if language != Language::Rust {
        // Workspace inheritance is a Cargo concept; other ecosystems resolve
        // from the analyzed root.
        return false;
    }
    let mut dir = root.parent();
    while let Some(ancestor) = dir {
        let manifest = ancestor.join("Cargo.toml");
        if !manifest.is_file() {
            // A plain directory (e.g. the `crates/` between a member and the
            // workspace root) hosts no package, so it governs nothing — keep
            // walking up.
            dir = ancestor.parent();
            continue;
        }
        if !declares_workspace(&manifest) {
            // A real package that is not a workspace root: its lockfile
            // governs only itself, so the searched root genuinely has none.
            return false;
        }
        if expected.iter().any(|name| ancestor.join(name).is_file()) {
            return true;
        }
        dir = ancestor.parent();
    }
    false
}

/// True when `manifest` parses and declares a `[workspace]` table.
fn declares_workspace(manifest: &Path) -> bool {
    std::fs::read_to_string(manifest)
        .ok()
        .and_then(|text| text.parse::<toml::Value>().ok())
        .map(|value| value.get("workspace").is_some())
        .unwrap_or(false)
}

fn check_lockfile(root: &Path, stack: &ProjectStack, report: &mut AnalysisReport) {
    // Plain `pip` has no standard lockfile; avoid noise.
    if stack.language == Language::Python
        && !matches!(
            stack.package_manager.as_deref(),
            Some("poetry") | Some("pipenv") | Some("uv")
        )
    {
        return;
    }

    let expected = lockfiles(stack.language);
    if expected.is_empty() {
        return;
    }
    if lockfile_present(stack.language, root, expected) {
        return;
    }

    report.push(Finding::warning(
        stack.manifest.display().to_string(),
        format!(
            "{}: no lockfile found (expected: {}). The build may not be reproducible.",
            stack.language,
            expected.join(" or ")
        ),
    ));
}

fn lockfiles(language: Language) -> &'static [&'static str] {
    match language {
        Language::Rust => &["Cargo.lock"],
        Language::JavaScript | Language::TypeScript => &[
            "package-lock.json",
            "yarn.lock",
            "pnpm-lock.yaml",
            "bun.lockb",
            "bun.lock",
        ],
        Language::Python => &["poetry.lock", "Pipfile.lock", "uv.lock"],
        Language::Go => &["go.sum"],
        Language::Php => &["composer.lock"],
        Language::Ruby => &["Gemfile.lock"],
        Language::Dart => &["pubspec.lock"],
        Language::Elixir => &["mix.lock"],
        Language::Swift => &["Package.resolved"],
        Language::Java | Language::Kotlin | Language::CSharp | Language::C | Language::Cpp
        | Language::Unknown => &[],
    }
}

fn normalize(name: &str) -> String {
    name.trim().to_lowercase().replace('_', "-")
}
