//! Detectors for compiled/static languages.
//!
//! Covers: Rust, Go, JVM (Java/Kotlin), .NET (C#), Swift and native C/C++.

use super::{dep_names, find_file, find_file_with_ext, match_frameworks, read, MatchMode};
use zavik_shared::{Dependency, DependencyKind, Error, Language, ProjectStack};
use std::path::{Path, PathBuf};

/// Runs all detectors in this module.
pub(crate) fn detect_all(root: &Path) -> Result<Vec<ProjectStack>, Error> {
    let mut stacks = Vec::new();
    stacks.extend(detect_rust(root)?);
    if let Some(stack) = detect_go(root)? {
        stacks.push(stack);
    }
    if let Some(stack) = detect_jvm(root)? {
        stacks.push(stack);
    }
    if let Some(stack) = detect_dotnet(root)? {
        stacks.push(stack);
    }
    if let Some(stack) = detect_swift(root)? {
        stacks.push(stack);
    }
    if let Some(stack) = detect_native(root)? {
        stacks.push(stack);
    }
    Ok(stacks)
}

// --- Rust ------------------------------------------------------------------

const RUST_FRAMEWORKS: &[(&str, &[&str])] = &[
    ("Tauri", &["tauri"]),
    ("Actix Web", &["actix-web"]),
    ("Axum", &["axum"]),
    ("Rocket", &["rocket"]),
    ("Warp", &["warp"]),
    ("Tokio", &["tokio"]),
    ("Bevy", &["bevy"]),
    ("Yew", &["yew"]),
    ("Leptos", &["leptos"]),
    ("Dioxus", &["dioxus"]),
];

/// Detects the Rust stack(s) rooted at `root`.
///
/// A **virtual workspace root** (no `[package]`, has `[workspace].members`)
/// expands into one stack per member — discovered by dogfooding this repo:
/// pointing `zavik` at the workspace root saw "0 dependencies" and the
/// `audit` approved nothing (docs/dogfooding.md). Member dependencies
/// declared with `workspace = true` resolve their version from the nearest
/// `[workspace.dependencies]` (manifest's own file first, then ancestors).
fn detect_rust(root: &Path) -> Result<Vec<ProjectStack>, Error> {
    let Some(manifest) = find_file(root, "Cargo.toml") else {
        return Ok(Vec::new());
    };
    let text = read(&manifest)?;
    let value: toml::Value = text
        .parse()
        .map_err(|e| Error::SerializationError(format!("Cargo.toml: {e}")))?;

    if value.get("package").is_none() {
        let members = value
            .get("workspace")
            .and_then(|w| w.get("members"))
            .and_then(|m| m.as_array());
        if let Some(members) = members {
            let exclude = workspace_list(&value, "exclude");
            let mut stacks = Vec::new();
            for dir in expand_members(root, members, &exclude) {
                if let Some(stack) = rust_stack_for(&dir)? {
                    stacks.push(stack);
                }
            }
            if !stacks.is_empty() {
                return Ok(stacks);
            }
            // Virtual root with no resolvable member: keep the honest bare
            // stack (audit reports "0 packages" rather than inventing).
        }
    }

    let inherited = workspace_dependencies(&manifest, &value);
    Ok(vec![rust_stack_from(manifest, value, &inherited)])
}

/// One stack for the `Cargo.toml` inside `dir` (member of a workspace).
fn rust_stack_for(dir: &Path) -> Result<Option<ProjectStack>, Error> {
    let manifest = dir.join("Cargo.toml");
    if !manifest.is_file() {
        return Ok(None);
    }
    let text = read(&manifest)?;
    let value: toml::Value = text
        .parse()
        .map_err(|e| Error::SerializationError(format!("Cargo.toml: {e}")))?;
    let inherited = workspace_dependencies(&manifest, &value);
    Ok(Some(rust_stack_from(manifest, value, &inherited)))
}

/// Builds the stack from an already-parsed manifest.
fn rust_stack_from(
    manifest: std::path::PathBuf,
    value: toml::Value,
    inherited: &toml::map::Map<String, toml::Value>,
) -> ProjectStack {
    let mut deps = Vec::new();
    for (table, kind) in [
        ("dependencies", DependencyKind::Runtime),
        ("dev-dependencies", DependencyKind::Development),
        ("build-dependencies", DependencyKind::Build),
    ] {
        if let Some(entries) = value.get(table).and_then(|v| v.as_table()) {
            for (name, spec) in entries {
                let (version, local) = toml_source(name, spec, inherited);
                let mut dependency = Dependency::new(name.clone(), version, kind);
                dependency.local = local;
                deps.push(dependency);
            }
        }
    }

    let frameworks = match_frameworks(&dep_names(&deps), MatchMode::Exact, RUST_FRAMEWORKS);
    let name = value
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    ProjectStack::new(Language::Rust, manifest, Some("cargo".to_string()))
        .with_name(name)
        .with_frameworks(frameworks)
        .with_dependencies(deps)
}

/// Version requirement and local-ness of a Cargo dependency entry.
/// Accepts `serde = "1"`, `serde = { version = "1", ... }` and
/// `serde = { workspace = true }` (version from `[workspace.dependencies]`).
///
/// Returns `(version, local)`: `local` marks a dependency declared with
/// `path = …` (directly or inherited), i.e. one that resolves from the local
/// filesystem rather than from a registry — its version lives in the target
/// manifest, so the analyzer must not report it as missing. A `workspace = true`
/// entry that the workspace never declares stays honestly `(None, false)`.
fn toml_source(
    name: &str,
    spec: &toml::Value,
    inherited: &toml::map::Map<String, toml::Value>,
) -> (Option<String>, bool) {
    if let Some(version) = plain_version(spec) {
        return (Some(version), spec.get("path").is_some());
    }
    if spec.get("workspace").and_then(|v| v.as_bool()) == Some(true) {
        return match inherited.get(name) {
            Some(entry) => (plain_version(entry), entry.get("path").is_some()),
            None => (None, false),
        };
    }
    (None, spec.get("path").is_some())
}

/// Version of a plain spec: `"1"` or `{ version = "1" }`.
fn plain_version(spec: &toml::Value) -> Option<String> {
    spec.as_str()
        .map(str::to_string)
        .or_else(|| spec.get("version").and_then(|v| v.as_str()).map(str::to_string))
}

/// `[workspace.dependencies]` of the workspace this manifest belongs to:
/// the manifest's own file first (a package can BE its workspace root),
/// then the nearest ancestor `Cargo.toml` that declares one.
fn workspace_dependencies(
    manifest: &Path,
    value: &toml::Value,
) -> toml::map::Map<String, toml::Value> {
    if let Some(table) = workspace_dep_table(value) {
        return table;
    }
    let mut dir = manifest.parent();
    while let Some(current) = dir {
        let candidate = current.join("Cargo.toml");
        if candidate.is_file() && candidate != manifest {
            if let Ok(text) = std::fs::read_to_string(&candidate) {
                if let Ok(parsed) = text.parse::<toml::Value>() {
                    if let Some(table) = workspace_dep_table(&parsed) {
                        return table;
                    }
                }
            }
        }
        dir = current.parent();
    }
    toml::map::Map::new()
}

fn workspace_dep_table(value: &toml::Value) -> Option<toml::map::Map<String, toml::Value>> {
    value
        .get("workspace")?
        .get("dependencies")?
        .as_table()
        .cloned()
}

/// String list under `[workspace]` (e.g. `exclude`).
fn workspace_list(value: &toml::Value, key: &str) -> Vec<String> {
    value
        .get("workspace")
        .and_then(|w| w.get(key))
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Expands `[workspace].members` patterns (literal segments and `*`
/// wildcards) into member directories holding a `Cargo.toml`, honoring
/// `exclude`, sorted + deduped for deterministic output.
pub(crate) fn expand_members(root: &Path, members: &[toml::Value], exclude: &[String]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for pattern in members.iter().filter_map(|v| v.as_str()) {
        let segments: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
        let mut matched = Vec::new();
        expand_segments(root, &segments, &mut matched);
        for dir in matched {
            let relative = dir
                .strip_prefix(root)
                .unwrap_or(&dir)
                .to_string_lossy()
                .replace('\\', "/");
            if exclude.iter().any(|e| e.trim_end_matches('/') == relative) {
                continue;
            }
            if dir.join("Cargo.toml").is_file() {
                dirs.push(dir);
            }
        }
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

fn expand_segments(base: &Path, segments: &[&str], out: &mut Vec<PathBuf>) {
    let Some((segment, rest)) = segments.split_first() else {
        out.push(base.to_path_buf());
        return;
    };
    if *segment == "*" {
        let Ok(entries) = std::fs::read_dir(base) else {
            return;
        };
        let mut dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        dirs.sort();
        for dir in dirs {
            expand_segments(&dir, rest, out);
        }
    } else {
        expand_segments(&base.join(segment), rest, out);
    }
}

// --- Go --------------------------------------------------------------------

const GO_FRAMEWORKS: &[(&str, &[&str])] = &[
    ("Gin", &["gin-gonic/gin"]),
    ("Echo", &["labstack/echo"]),
    ("Fiber", &["gofiber/fiber"]),
    ("GORM", &["gorm.io/gorm"]),
    ("Chi", &["go-chi/chi"]),
    ("Cobra", &["spf13/cobra"]),
];

fn detect_go(root: &Path) -> Result<Option<ProjectStack>, Error> {
    let Some(manifest) = find_file(root, "go.mod") else {
        return Ok(None);
    };

    let text = read(&manifest)?;
    let mut deps = Vec::new();
    let mut module_name: Option<String> = None;
    let mut in_require_block = false;

    for raw in text.lines() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("module ") {
            // `module github.com/user/project` -> "project"
            module_name = Some(
                rest.trim()
                    .rsplit('/')
                    .next()
                    .unwrap_or(rest.trim())
                    .to_string(),
            );
            continue;
        }
        if line.starts_with("require (") || line == "require" {
            in_require_block = true;
            continue;
        }
        if in_require_block && line == ")" {
            in_require_block = false;
            continue;
        }
        if line.starts_with("//") {
            continue;
        }

        // Dependency line: `<module> <version> [// indirect]`
        let candidate = if in_require_block {
            Some(line)
        } else {
            line.strip_prefix("require ")
        };

        if let Some(entry) = candidate {
            let mut parts = entry.split_whitespace();
            if let (Some(name), Some(version)) = (parts.next(), parts.next()) {
                deps.push(Dependency::new(name, Some(version.to_string()), DependencyKind::Runtime));
            }
        }
    }

    let frameworks = match_frameworks(&dep_names(&deps), MatchMode::Contains, GO_FRAMEWORKS);
    Ok(Some(
        ProjectStack::new(Language::Go, manifest, Some("go modules".to_string()))
            .with_name(module_name)
            .with_frameworks(frameworks)
            .with_dependencies(deps),
    ))
}

// --- JVM (Java / Kotlin) ---------------------------------------------------

const JVM_FRAMEWORKS: &[(&str, &[&str])] = &[
    ("Spring", &["spring"]),
    ("Quarkus", &["quarkus"]),
    ("Micronaut", &["micronaut"]),
    ("Jakarta EE", &["jakarta"]),
    ("Android", &["androidx", "android"]),
];

fn detect_jvm(root: &Path) -> Result<Option<ProjectStack>, Error> {
    if let Some(manifest) = find_file(root, "pom.xml") {
        return detect_maven(manifest).map(Some);
    }

    if let Some(manifest) = find_file(root, "build.gradle.kts").or_else(|| find_file(root, "build.gradle"))
    {
        return detect_gradle(manifest).map(Some);
    }

    Ok(None)
}

fn detect_maven(manifest: std::path::PathBuf) -> Result<ProjectStack, Error> {
    let text = read(&manifest)?;
    let block = regex::Regex::new(r"(?s)<dependency>(.*?)</dependency>").expect("regex válido");
    let group = regex::Regex::new(r"<groupId>\s*([^<]+?)\s*</groupId>").expect("regex válido");
    let artifact = regex::Regex::new(r"<artifactId>\s*([^<]+?)\s*</artifactId>").expect("regex válido");
    let version = regex::Regex::new(r"<version>\s*([^<]+?)\s*</version>").expect("regex válido");

    let mut deps = Vec::new();
    for capture in block.captures_iter(&text) {
        let body = &capture[1];
        if let Some(artifact_capture) = artifact.captures(body) {
            let artifact_id = artifact_capture[1].trim();
            // Stores the full coordinate "groupId:artifactId" — it is what
            // allows checking existence on Maven Central later.
            let name = match group.captures(body) {
                Some(g) if !g[1].trim().is_empty() => {
                    format!("{}:{}", g[1].trim(), artifact_id)
                }
                _ => artifact_id.to_string(),
            };
            let version = version.captures(body).map(|c| c[1].trim().to_string());
            deps.push(Dependency::new(name, version, DependencyKind::Runtime));
        }
    }

    let frameworks = match_frameworks(&dep_names(&deps), MatchMode::Contains, JVM_FRAMEWORKS);
    Ok(
        ProjectStack::new(Language::Java, manifest, Some("maven".to_string()))
            .with_frameworks(frameworks)
            .with_dependencies(deps),
    )
}

fn detect_gradle(manifest: std::path::PathBuf) -> Result<ProjectStack, Error> {
    let text = read(&manifest)?;
    let is_kotlin = manifest.extension().and_then(|e| e.to_str()) == Some("kts");
    let dep_re = regex::Regex::new(
        r#"(?m)^\s*(implementation|api|compileOnly|runtimeOnly|testImplementation|testCompile|kapt|classpath)\s*[\('"]\s*['"]?([^'"\)]+)"#,
    )
    .expect("regex válido");

    let mut deps = Vec::new();
    for capture in dep_re.captures_iter(&text) {
        let coordinate = capture[2].trim();
        // Coordinate in the form `group:artifact:version`.
        let mut parts = coordinate.split(':');
        let group = parts.next().unwrap_or("");
        let artifact = parts.next().unwrap_or("");
        let version = parts.next().map(str::to_string);
        // Stores "group:artifact" (without the version) — the coordinate for Maven Central.
        let name = if artifact.is_empty() {
            group.to_string()
        } else {
            format!("{group}:{artifact}")
        };
        let kind = if capture[1].starts_with("test") {
            DependencyKind::Development
        } else {
            DependencyKind::Runtime
        };
        deps.push(Dependency::new(name, version, kind));
    }

    let frameworks = match_frameworks(&dep_names(&deps), MatchMode::Contains, JVM_FRAMEWORKS);
    let language = if is_kotlin { Language::Kotlin } else { Language::Java };
    Ok(
        ProjectStack::new(language, manifest, Some("gradle".to_string()))
            .with_frameworks(frameworks)
            .with_dependencies(deps),
    )
}

// --- .NET (C#) -------------------------------------------------------------

const DOTNET_FRAMEWORKS: &[(&str, &[&str])] = &[
    ("ASP.NET Core", &["microsoft.aspnetcore", "aspnetcore"]),
    ("Entity Framework", &["microsoft.entityframeworkcore"]),
    ("MAUI", &["microsoft.maui"]),
    ("Xamarin", &["xamarin"]),
];

fn detect_dotnet(root: &Path) -> Result<Option<ProjectStack>, Error> {
    let Some(manifest) = find_file_with_ext(root, "csproj") else {
        return Ok(None);
    };

    let text = read(&manifest)?;
    let dep_re = regex::Regex::new(
        r#"<PackageReference\s+Include="([^"]+)"(?:\s+Version="([^"]+)")?\s*/?>"#,
    )
    .expect("regex válido");

    let mut deps = Vec::new();
    for capture in dep_re.captures_iter(&text) {
        deps.push(Dependency::new(
            capture[1].to_string(),
            capture.get(2).map(|m| m.as_str().to_string()),
            DependencyKind::Runtime,
        ));
    }

    let frameworks = match_frameworks(&dep_names(&deps), MatchMode::Contains, DOTNET_FRAMEWORKS);
    Ok(Some(
        ProjectStack::new(Language::CSharp, manifest, Some("nuget".to_string()))
            .with_frameworks(frameworks)
            .with_dependencies(deps),
    ))
}

// --- Swift -----------------------------------------------------------------

const SWIFT_FRAMEWORKS: &[(&str, &[&str])] = &[
    ("Vapor", &["vapor"]),
    ("Hummingbird", &["hummingbird"]),
];

fn detect_swift(root: &Path) -> Result<Option<ProjectStack>, Error> {
    let Some(manifest) = find_file(root, "Package.swift") else {
        return Ok(None);
    };

    let text = read(&manifest)?;
    let pkg_re = regex::Regex::new(r#"\.package\(\s*(?:name:\s*"([^"]+)"|url:\s*"([^"]+)")"#)
        .expect("regex válido");

    let mut deps = Vec::new();
    for capture in pkg_re.captures_iter(&text) {
        let raw = capture.get(1).or_else(|| capture.get(2)).map(|m| m.as_str());
        if let Some(raw) = raw {
            // From a URL, we use the last segment (without `.git`) as the name.
            let name = if raw.contains("://") {
                raw.rsplit('/')
                    .next()
                    .unwrap_or(raw)
                    .trim_end_matches(".git")
                    .to_string()
            } else {
                raw.to_string()
            };
            deps.push(Dependency::new(name, None, DependencyKind::Runtime));
        }
    }

    let frameworks = match_frameworks(&dep_names(&deps), MatchMode::Contains, SWIFT_FRAMEWORKS);
    Ok(Some(
        ProjectStack::new(Language::Swift, manifest, Some("swiftpm".to_string()))
            .with_frameworks(frameworks)
            .with_dependencies(deps),
    ))
}

// --- C / C++ ---------------------------------------------------------------

fn detect_native(root: &Path) -> Result<Option<ProjectStack>, Error> {
    // CMake is a strong and reliable signal of C/C++.
    if let Some(manifest) = find_file(root, "CMakeLists.txt") {
        return Ok(Some(ProjectStack::new(
            classify_c_or_cpp(root),
            manifest,
            Some("cmake".to_string()),
        )));
    }
    // A Makefile alone is NOT C/C++ — Go, Ruby, Elixir etc. also use them.
    // We only assume C/C++ if there is real C/C++ source in the project.
    if let Some(source) = find_cxx_source(root) {
        let makefile = find_file(root, "Makefile").or_else(|| find_file(root, "makefile"));
        let (manifest, package_manager) = match makefile {
            Some(path) => (path, Some("make".to_string())),
            None => (source, None),
        };
        return Ok(Some(ProjectStack::new(
            classify_c_or_cpp(root),
            manifest,
            package_manager,
        )));
    }
    Ok(None)
}

/// Distinguishes C from C++ by the source extensions: if there is ANY
/// C++-specific file (`.cpp`, `.cc`, `.cxx`, `.hpp`...), it's C++; only `.c`/`.h` is C.
fn classify_c_or_cpp(root: &Path) -> Language {
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext = ext.to_ascii_lowercase();
                if CPP_EXTS.contains(&ext.as_str()) {
                    return Language::Cpp;
                }
            }
        }
    }
    // With no C++-specific extension ⇒ assume C.
    Language::C
}

const CXX_EXTS: &[&str] = &["c", "cpp", "cc", "cxx", "h", "hpp", "hh"];
/// Extensions that only C++ uses (a lone `.c`/`.h` does NOT count as C++).
const CPP_EXTS: &[&str] = &["cpp", "cc", "cxx", "hpp", "hh", "hxx"];

/// Looks for C/C++ source in the root (and one level of subdirectory, e.g. `src/`).
fn find_cxx_source(root: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(root).ok()?;
    let mut dirs: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        } else if is_cxx(&path) {
            return Some(path);
        }
    }
    for dir in dirs {
        if let Ok(sub) = std::fs::read_dir(&dir) {
            for entry in sub.flatten() {
                let path = entry.path();
                if path.is_file() && is_cxx(&path) {
                    return Some(path);
                }
            }
        }
    }
    None
}

fn is_cxx(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| CXX_EXTS.contains(&e))
}
