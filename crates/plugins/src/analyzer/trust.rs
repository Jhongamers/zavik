//! Trust verification **before** installing (supply-chain pre-flight).
//!
//! The idea is exactly not to blindly trust the manifest: Zavik "does a
//! look-up" and exposes what could execute arbitrary code or what does not match
//! the registry. Each risk here becomes an **individual** confirmation — never
//! covered by "yes to all" nor by `--yes`. It protects the case of an AI driving
//! the tool without realizing what it is telling it to install.
//!
//! Three checks:
//! 1. npm lifecycle scripts that run on `npm install` (arbitrary code);
//! 2. Dependencies from outside the registry (git/path/file);
//! 3. Presence in the official registry (anti-typo/typosquat/fraud) — the "look-up".

use super::registry::{self, Presence, MAX_CHECKS};
use super::version::exact_version;
use super::SourceKind;
use zavik_shared::{Language, ProjectStack, TrustReport, TrustRisk};
use std::collections::HashSet;

/// npm hooks that run code when the project is installed.
const INSTALL_HOOKS: &[&str] = &["preinstall", "install", "postinstall", "prepare"];

/// Below this (downloads/week on npm) the package is suspected of being a
/// typo/typosquat of something popular. `lodahs` ~125; `lodash` ~162.000.000.
const LOW_DOWNLOADS: u64 = 1_000;

/// Runs all the trust checks and accumulates the risks in `report`.
pub(crate) fn verify(stacks: &[ProjectStack], report: &mut TrustReport) {
    for stack in stacks {
        if matches!(stack.language, Language::JavaScript | Language::TypeScript) {
            check_lifecycle_scripts(&stack.manifest, report);
        }
    }

    for stack in stacks {
        for dependency in &stack.dependencies {
            if let SourceKind::NonRegistry(_) = SourceKind::classify(dependency.version.as_deref())
            {
                report.push(TrustRisk::warning(
                    dependency.name.clone(),
                    format!(
                        "{}: '{}' comes from outside the registry (git/path/file). \
                         Installs third-party code without package-manager audit.",
                        stack.language, dependency.name
                    ),
                    dependency.version.clone().unwrap_or_default(),
                ));
            }
        }
    }

    check_dependency_install_scripts(stacks, report);
    check_registry_presence(stacks, report);
}

/// Reads the `package.json` and reports scripts that run code on `npm install`.
fn check_lifecycle_scripts(manifest: &std::path::Path, report: &mut TrustReport) {
    let Ok(text) = std::fs::read_to_string(manifest) else {
        return;
    };
    let Ok(json) = text.parse::<serde_json::Value>() else {
        return;
    };
    let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) else {
        return;
    };

    for hook in INSTALL_HOOKS {
        if let Some(command) = scripts.get(*hook).and_then(|c| c.as_str()) {
            if command.trim().is_empty() {
                continue;
            }
            report.push(TrustRisk::critical(
                manifest.display().to_string(),
                format!(
                    "Script '{hook}' runs arbitrary code on `npm install` \
                     (for anyone who clones this project)."
                ),
                command.to_string(),
            ));
        }
    }
}

/// Sees which npm dependencies run an install script (`postinstall` etc.)
/// — read from the registry, without downloading. Aggregates everything into a
/// single warning (not one per package) so it doesn't become spam, but still
/// requires the user to acknowledge it before `npm install` executes
/// third-party code.
fn check_dependency_install_scripts(stacks: &[ProjectStack], report: &mut TrustReport) {
    let agent = registry::agent();
    let mut checked: HashSet<String> = HashSet::new();
    let mut entries: Vec<String> = Vec::new();
    let mut requests = 0usize;

    'stacks: for stack in stacks {
        if !matches!(stack.language, Language::JavaScript | Language::TypeScript) {
            continue;
        }
        for dependency in &stack.dependencies {
            if !matches!(
                SourceKind::classify(dependency.version.as_deref()),
                SourceKind::Registry
            ) {
                continue;
            }
            if !checked.insert(dependency.name.to_lowercase()) {
                continue;
            }
            if requests >= MAX_CHECKS {
                break 'stacks;
            }
            requests += 1;

            let target =
                exact_version(dependency.version.as_deref()).unwrap_or_else(|| "latest".into());
            for (hook, command) in registry::npm_install_scripts(&agent, &dependency.name, &target)
            {
                entries.push(format!(
                    "{} → {hook}: {}",
                    dependency.name,
                    truncate(&command, 120)
                ));
            }
        }
    }

    if !entries.is_empty() {
        report.push(TrustRisk::warning(
            "dependencies".to_string(),
            format!(
                "{} dependency/dependencies run install script(s) on `npm install` \
                 (third-party code). Review before proceeding.",
                entries.len()
            ),
            entries.join("\n"),
        ));
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max).collect();
        format!("{cut}…")
    }
}

/// Checks in the registry whether each registry package exists (the "look-up").
fn check_registry_presence(stacks: &[ProjectStack], report: &mut TrustReport) {
    let agent = registry::agent();
    let mut checked: HashSet<(Language, String)> = HashSet::new();
    let mut requests = 0usize;
    let mut any_answered = false;
    let mut unknowns: Vec<(&ProjectStack, &str)> = Vec::new();

    'stacks: for stack in stacks {
        for dependency in &stack.dependencies {
            if !matches!(
                SourceKind::classify(dependency.version.as_deref()),
                SourceKind::Registry
            ) {
                continue;
            }
            let key = (stack.language, dependency.name.to_lowercase());
            if !checked.insert(key) {
                continue;
            }
            if requests >= MAX_CHECKS {
                report.push(TrustRisk::info(
                    dependency.name.clone(),
                    format!("Registry verification limited to {MAX_CHECKS} packages."),
                    String::new(),
                ));
                break 'stacks;
            }
            requests += 1;

            let is_node = matches!(
                stack.language,
                Language::JavaScript | Language::TypeScript
            );
            if is_node {
                // npm: a single call, which confirms existence (404) AND popularity.
                match registry::npm_downloads_last_week(&agent, &dependency.name) {
                    Some(0) => {
                        any_answered = true;
                        report.push(TrustRisk::critical(
                            dependency.name.clone(),
                            format!(
                                "{}: '{}' does not exist on npm. Possible typo, private or \
                                 removed. Check before installing.",
                                stack.language, dependency.name
                            ),
                            format!("npm doesn't know '{}'", dependency.name),
                        ));
                    }
                    Some(n) if n < LOW_DOWNLOADS => {
                        any_answered = true;
                        report.push(TrustRisk::warning(
                            dependency.name.clone(),
                            format!(
                                "{}: '{}' has only {n} download(s)/week on npm. Confirm \
                                 it is the expected package (possible typo of a popular package).",
                                stack.language, dependency.name
                            ),
                            format!("https://www.npmjs.com/package/{}", dependency.name),
                        ));
                    }
                    Some(_) => any_answered = true,
                    None => unknowns.push((stack, dependency.name.as_str())),
                }
            } else {
                match registry::presence(&agent, stack.language, &dependency.name) {
                    Presence::Absent => {
                        any_answered = true;
                        report.push(TrustRisk::critical(
                            dependency.name.clone(),
                            format!(
                                "{}: '{}' does not exist in the official registry. Possible typo, \
                                 private package or fraud. Check before installing.",
                                stack.language, dependency.name
                            ),
                            format!("registry doesn't know '{}'", dependency.name),
                        ));
                    }
                    Presence::Unknown => unknowns.push((stack, dependency.name.as_str())),
                    Presence::Present(_) => any_answered = true,
                }
            }
        }
    }

    // If NOTHING answered, we are probably offline: one warning instead of one per package.
    if !any_answered && !unknowns.is_empty() {
        report.push(TrustRisk::info(
            "registry",
            "No network: couldn't verify packages against the registry (offline?).",
            format!("{} package(s) not verified.", unknowns.len()),
        ));
        return;
    }
    for (stack, name) in unknowns {
        report.push(TrustRisk::info(
            name.to_string(),
            format!(
                "{}: couldn't verify '{name}' against the registry (network/registry unavailable).",
                stack.language
            ),
            String::new(),
        ));
    }
}
