//! **Online** access to package registries (crates.io, npm, PyPI).
//!
//! It is DevForge's "look-up" layer: instead of blindly trusting the manifest,
//! we consult the real registry. Used by:
//! - [`check_latest`] (`analyze --online` command): finds newer versions;
//! - [`presence`] (trust verification): checks whether the package **exists**.

use super::version::{base_version, compare};
use super::SourceKind;
use devforge_shared::{AnalysisReport, Finding, Language, ProjectStack};
use std::collections::HashSet;

/// Request cap so we neither take too long nor hit the rate limit.
pub(crate) const MAX_CHECKS: usize = 40;
const USER_AGENT: &str = "devforge-analyzer/0.1 (devforge)";

/// Result of querying the registry about a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Presence {
    /// The package exists. `Some(latest)` when we managed to read the latest version.
    Present(Option<String>),
    /// The registry answered 404: the package **does not exist** (suspicious).
    Absent,
    /// Network/error/language without a registry: **cannot be confirmed**.
    /// (We never report as fraud something we merely failed to verify.)
    Unknown,
}

/// Builds the HTTP agent (with User-Agent, required by crates.io).
pub(crate) fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().user_agent(USER_AGENT).build()
}

/// Queries the latest version of each registry dependency.
pub(crate) fn check_latest(stacks: &[ProjectStack], report: &mut AnalysisReport) {
    let agent = agent();
    let mut checked: HashSet<(Language, String)> = HashSet::new();
    let mut requests = 0usize;
    let mut truncated = false;

    'stacks: for stack in stacks {
        for dependency in &stack.dependencies {
            if !matches!(
                SourceKind::classify(dependency.version.as_deref()),
                SourceKind::Registry
            ) {
                continue;
            }
            let Some(current) = base_version(dependency.version.as_deref()) else {
                continue;
            };
            let key = (stack.language, dependency.name.to_lowercase());
            if !checked.insert(key) {
                continue;
            }
            if requests >= MAX_CHECKS {
                truncated = true;
                break 'stacks;
            }

            if let Presence::Present(Some(latest)) =
                presence(&agent, stack.language, &dependency.name)
            {
                requests += 1;
                if compare(&latest, &current) == std::cmp::Ordering::Greater {
                    report.push(Finding::warning(
                        dependency.name.clone(),
                        format!(
                            "{}: '{}' tem versão mais nova no registro: {current} -> {latest}.",
                            stack.language, dependency.name
                        ),
                    ));
                }
            } else {
                // Absence/errors are the trust verification's job, not analyze's.
                requests += 1;
            }
        }
    }

    if truncated {
        report.push(Finding::info(format!(
            "Checagem online limitada a {MAX_CHECKS} dependências."
        )));
    }
}

/// Asks the registry whether the package exists (and, if possible, what the latest version is).
pub(crate) fn presence(agent: &ureq::Agent, language: Language, name: &str) -> Presence {
    let Some(url) = url_for(language, name) else {
        return Presence::Unknown;
    };
    match agent.get(&url).call() {
        Ok(response) => match response.into_json::<serde_json::Value>() {
            Ok(json) => Presence::Present(latest_from(language, &json)),
            // Answered 200 but we couldn't parse it: exists, version unknown.
            Err(_) => Presence::Present(None),
        },
        Err(ureq::Error::Status(404, _)) => Presence::Absent,
        // 403 (rate limit), 5xx, network down, etc.: nothing can be asserted.
        Err(_) => Presence::Unknown,
    }
}

/// Downloads in the last week on npm. `None` if we don't know (network/error).
///
/// It is the best anti-typosquat signal: a typosquat of a popular package has
/// tiny downloads (e.g. `lodahs` ~125/week) while the real package
/// (`lodash`) has millions.
pub(crate) fn npm_downloads_last_week(agent: &ureq::Agent, name: &str) -> Option<u64> {
    let url = format!(
        "https://api.npmjs.org/downloads/point/last-week/{}",
        encode(name)
    );
    let json = match agent.get(&url).call() {
        Ok(response) => response.into_json::<serde_json::Value>().ok()?,
        // 404 = package does not exist on npm.
        Err(ureq::Error::Status(404, _)) => return Some(0),
        Err(_) => return None,
    };
    json.get("downloads").and_then(|d| d.as_u64())
}

fn url_for(language: Language, name: &str) -> Option<String> {
    match language {
        Language::Rust => Some(format!("https://crates.io/api/v1/crates/{name}")),
        Language::JavaScript | Language::TypeScript => {
            Some(format!("https://registry.npmjs.org/{}", encode(name)))
        }
        Language::Python => Some(format!("https://pypi.org/pypi/{name}/json")),
        // Below: EXISTENCE check only (200 vs 404) — anti-fraud.
        Language::Ruby => Some(format!("https://rubygems.org/api/v1/gems/{name}.json")),
        Language::Php => Some(format!("https://repo.packagist.org/p2/{name}.json")),
        // NuGet requires the id in lowercase in the flatcontainer.
        Language::CSharp => Some(format!(
            "https://api.nuget.org/v3-flatcontainer/{}/index.json",
            name.to_lowercase()
        )),
        Language::Dart => Some(format!("https://pub.dev/api/packages/{name}")),
        Language::Elixir => Some(format!("https://hex.pm/api/packages/{name}")),
        // Go: official proxy; uppercase escaped with '!' (e.g. !sirupsen).
        Language::Go => Some(format!(
            "https://proxy.golang.org/{}/@latest",
            go_escape(name)
        )),
        // Maven Central: coordinate "groupId:artifactId" -> path in repo1.
        // Without groupId the path can't be built (then it becomes None/Unknown).
        Language::Java | Language::Kotlin => {
            let (group, artifact) = name.split_once(':')?;
            let group_path = group.replace('.', "/");
            Some(format!(
                "https://repo1.maven.org/maven2/{group_path}/{artifact}/"
            ))
        }
        _ => None,
    }
}

fn latest_from(language: Language, json: &serde_json::Value) -> Option<String> {
    match language {
        Language::Rust => {
            let krate = json.get("crate")?;
            krate
                .get("max_stable_version")
                .or_else(|| krate.get("newest_version"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        }
        Language::JavaScript | Language::TypeScript => json
            .get("dist-tags")
            .and_then(|tags| tags.get("latest"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        Language::Python => json
            .get("info")
            .and_then(|info| info.get("version"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        // The Go proxy returns `{ "Version": "v1.9.0", ... }`.
        Language::Go => json
            .get("Version")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        _ => None,
    }
}

/// Escapes uppercase letters in the Go module path (`Sirupsen` -> `!sirupsen`),
/// as required by `proxy.golang.org`. Slashes and dots stay as they are.
fn go_escape(module: &str) -> String {
    module
        .chars()
        .map(|c| {
            if c.is_ascii_uppercase() {
                format!("!{}", c.to_ascii_lowercase())
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// Encodes npm scoped names (`@scope/name` -> `@scope%2Fname`).
fn encode(name: &str) -> String {
    name.replace('/', "%2F")
}

/// npm hooks that run code on `npm install`.
pub(crate) const NPM_INSTALL_HOOKS: &[&str] = &["preinstall", "install", "postinstall", "prepare"];

/// Fetches the install scripts of an npm package version **via the registry**
/// (without downloading the tarball). `target` is an exact version or `"latest"`.
///
/// This is how we grab a DEPENDENCY's `postinstall` before running
/// `npm install` — the most dangerous supply-chain vector.
pub(crate) fn npm_install_scripts(
    agent: &ureq::Agent,
    name: &str,
    target: &str,
) -> Vec<(String, String)> {
    let url = format!("https://registry.npmjs.org/{}/{}", encode(name), target);
    let Ok(response) = agent.get(&url).call() else {
        return Vec::new();
    };
    let Ok(json) = response.into_json::<serde_json::Value>() else {
        return Vec::new();
    };
    let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) else {
        return Vec::new();
    };

    NPM_INSTALL_HOOKS
        .iter()
        .filter_map(|hook| {
            scripts
                .get(*hook)
                .and_then(|c| c.as_str())
                .filter(|c| !c.trim().is_empty())
                .map(|command| (hook.to_string(), command.to_string()))
        })
        .collect()
}
