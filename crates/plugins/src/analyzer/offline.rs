//! **Offline** analysis: heuristics that don't depend on the network.
//!
//! Rules:
//! - wildcard / unpinned version (`*`, `latest`, empty, ...);
//! - dependency with no declared version;
//! - dependency coming from outside the registry (git/path/file/...);
//! - missing lockfile (build not reproducible);
//! - same dependency declared with different versions in the same stack.

use super::SourceKind;
use devforge_shared::{AnalysisReport, Finding, Language, ProjectStack};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

/// Runs all the offline rules.
pub(crate) fn analyze(root: &Path, stacks: &[ProjectStack], report: &mut AnalysisReport) {
    for stack in stacks {
        check_lockfile(root, stack, report);
        for dependency in &stack.dependencies {
            check_dependency(stack, dependency, report);
        }
        check_duplicate_versions(stack, report);
    }
}

fn check_dependency(
    stack: &ProjectStack,
    dependency: &devforge_shared::Dependency,
    report: &mut AnalysisReport,
) {
    let name = &dependency.name;
    match SourceKind::classify(dependency.version.as_deref()) {
        SourceKind::NonRegistry(source) => report.push(Finding::info(format!(
            "{}: '{name}' vem de '{source}' (fora do registro de pacotes).",
            stack.language
        ))),
        SourceKind::Wildcard(value) => report.push(Finding::warning(
            name.clone(),
            format!(
                "{}: '{name}' está sem versão fixada ({value}). Fixe uma versão para builds reprodutíveis.",
                stack.language
            ),
        )),
        SourceKind::Missing => report.push(Finding::info(format!(
            "{}: '{name}' não declara versão.",
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
                    "{}: '{name}' aparece com versões diferentes: {}.",
                    stack.language,
                    found.into_iter().collect::<Vec<_>>().join(", ")
                ),
            ));
        }
    }
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
    if expected.iter().any(|name| root.join(name).is_file()) {
        return;
    }

    report.push(Finding::warning(
        stack.manifest.display().to_string(),
        format!(
            "{}: nenhum lockfile encontrado (esperado: {}). O build pode não ser reproduzível.",
            stack.language,
            expected.join(" ou ")
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
