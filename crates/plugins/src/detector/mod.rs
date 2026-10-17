//! Detection of a project's stack from the manifests present at the root.
//!
//! The [`StackDetector`] scans the project directory looking for known files
//! (`Cargo.toml`, `package.json`, `requirements.txt`, `go.mod`,
//! `pom.xml`, `composer.json`, ...) and returns a [`devforge_shared::ProjectStack`]
//! per ecosystem found.
//!
//! The per-language detectors are split into:
//! - [`compiled`]: compiled/static languages (Rust, Go, JVM, .NET, Swift, C/C++).
//! - [`scripting`]: interpreted languages (Node/TS, Python, PHP, Ruby, Dart, Elixir).

mod compiled;
mod scripting;

use devforge_shared::{Context, Dependency, Error, ProjectStack};
use devforge_sdk::{Capability, ProjectDetector};
use std::path::{Path, PathBuf};

/// Plugin that recognizes the project's stack.
///
/// With `recursive = false` (default) it only looks at the project root. With
/// `recursive = true` (the `--all` flag) it also descends into subfolders looking
/// for project manifests (monorepo / nested manifest), skipping dependency/build
/// dirs and with a depth/count limit.
#[derive(Debug, Default)]
pub struct StackDetector {
    pub recursive: bool,
}

impl Capability for StackDetector {
    fn name(&self) -> &'static str {
        "stack_detector"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_detector(&self) -> Option<&dyn ProjectDetector> {
        Some(self)
    }
}

impl ProjectDetector for StackDetector {
    fn detect(&self, context: &Context) -> Result<Vec<ProjectStack>, Error> {
        let root = &context.project_root;
        if !root.is_dir() {
            return Err(Error::IoError(format!(
                "diretório de projeto não encontrado: {}",
                root.display()
            )));
        }

        let mut stacks = Vec::new();
        // The root is always analyzed (keeps simple projects and source-based C/C++ working).
        stacks.extend(compiled::detect_all(root)?);
        stacks.extend(scripting::detect_all(root)?);

        if self.recursive {
            for dir in discover_nested_manifest_dirs(root) {
                stacks.extend(compiled::detect_all(&dir)?);
                stacks.extend(scripting::detect_all(&dir)?);
            }
        }

        // Deterministic output (useful for tests and for the user), with
        // dedup by manifest: a virtual workspace root expands its members
        // AND the recursive discovery may reach the same manifest again.
        stacks.sort_by(|a, b| a.manifest.cmp(&b.manifest));
        stacks.dedup_by(|a, b| a.manifest == b.manifest);
        Ok(stacks)
    }
}

// ---------------------------------------------------------------------------
// Recursive manifest discovery (monorepo / nested manifest)
// ---------------------------------------------------------------------------

/// Manifests that mark a directory as "contains a project".
/// Kept in sync with the `compiled`/`scripting` detectors.
const KNOWN_MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "go.mod",
    "pom.xml",
    "build.gradle.kts",
    "build.gradle",
    "Package.swift",
    "CMakeLists.txt",
    "package.json",
    "pyproject.toml",
    "requirements.txt",
    "Pipfile",
    "setup.py",
    "composer.json",
    "Gemfile",
    "pubspec.yaml",
    "mix.exs",
];

/// Manifests identified by file extension (e.g. the .NET `.csproj`).
const MANIFEST_EXTS: &[&str] = &["csproj"];

/// Directories that are never worth descending into (dependencies/build/hidden).
const EXCLUDED_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "build",
    "dist",
    "vendor",
    ".git",
    ".venv",
    "venv",
    "__pycache__",
    "Pods",
    ".build",
    "bin",
    "obj",
    ".next",
    ".nuxt",
    ".cache",
    ".dart_tool",
    "_build",
    "coverage",
];

const MAX_DEPTH: usize = 6;
const MAX_DIRS: usize = 64;

/// Finds subdirectories (starting from the root) that contain a project manifest.
/// The root itself is not included (it is already analyzed separately).
fn discover_nested_manifest_dirs(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    // Stack of (directory, depth); the root starts at 0.
    let mut pending: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];

    while let Some((dir, depth)) = pending.pop() {
        if found.len() >= MAX_DIRS || depth >= MAX_DEPTH {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if found.len() >= MAX_DIRS {
                break;
            }
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if EXCLUDED_DIRS.contains(&name.as_ref()) {
                continue;
            }
            if dir_has_manifest(&path) {
                found.push(path.clone());
            }
            pending.push((path, depth + 1));
        }
    }

    found.sort();
    found
}

/// Does the directory contain any known project manifest?
fn dir_has_manifest(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if KNOWN_MANIFESTS.contains(&name.as_ref()) {
            return true;
        }
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| MANIFEST_EXTS.contains(&e))
        {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Shared helpers across the detectors
// ---------------------------------------------------------------------------

/// How to compare a dependency name against a framework keyword.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MatchMode {
    /// Exact equality (after normalization). Good for npm/pip/cargo/gems.
    Exact,
    /// Checks whether the name contains the keyword. Good for Go modules,
    /// Maven coordinates and .NET namespaces.
    Contains,
}

/// Returns the frameworks whose keywords appear in the already-normalized
/// dependency list (see [`dep_names`]).
pub(crate) fn match_frameworks(
    deps: &[String],
    mode: MatchMode,
    table: &[(&str, &[&str])],
) -> Vec<String> {
    let mut found = Vec::new();
    for (framework, keywords) in table {
        let hit = deps.iter().any(|dep| {
            keywords.iter().any(|kw| {
                let kw = normalize(kw);
                match mode {
                    MatchMode::Exact => dep.as_str() == kw.as_str(),
                    MatchMode::Contains => dep.contains(kw.as_str()),
                }
            })
        });
        if hit {
            found.push((*framework).to_string());
        }
    }
    found
}

/// Normalizes a dependency name: lowercase and `_` -> `-`.
///
/// That way `Django` and `django`, or `serde_json` and `serde-json`, match each other.
pub(crate) fn normalize(name: &str) -> String {
    name.trim().to_lowercase().replace('_', "-")
}

/// Extracts the normalized names from a list of dependencies.
pub(crate) fn dep_names(deps: &[Dependency]) -> Vec<String> {
    deps.iter().map(|d| normalize(&d.name)).collect()
}

/// Looks for an exact file in the project root.
pub(crate) fn find_file(root: &Path, name: &str) -> Option<PathBuf> {
    let path = root.join(name);
    path.is_file().then_some(path)
}

/// Looks for the first file with the given extension (e.g. `"csproj"`).
pub(crate) fn find_file_with_ext(root: &Path, ext: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(root).ok()?;
    let mut found: Option<PathBuf> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some(ext) && found.is_none() {
            found = Some(path);
        }
    }
    found
}

/// Reads a file, mapping the error into the domain.
pub(crate) fn read(path: &Path) -> Result<String, Error> {
    std::fs::read_to_string(path).map_err(|e| Error::IoError(format!("{}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use devforge_shared::Language;
    use devforge_sdk::ProjectDetector;

    fn context(dir: &Path) -> Context {
        Context {
            project_root: dir.to_path_buf(),
        }
    }

    #[test]
    fn detects_rust_with_framework() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[package]
name = "exemplo"
version = "0.1.0"

[dependencies]
tauri = "2"
serde = "1"
"#,
        )?;

        let stacks = StackDetector::default().detect(&context(dir.path()))?;
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].language, Language::Rust);
        assert!(stacks[0].frameworks.contains(&"Tauri".to_string()));
        assert_eq!(stacks[0].dependencies.len(), 2);
        Ok(())
    }

    #[test]
    fn virtual_workspace_root_expands_members_with_inherited_versions(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[workspace]
members = ["crates/*"]

[workspace.dependencies]
serde = "1.7.99"
"#,
        )?;
        std::fs::create_dir_all(dir.path().join("crates/app"))?;
        std::fs::create_dir_all(dir.path().join("crates/lib"))?;
        std::fs::write(
            dir.path().join("crates/app/Cargo.toml"),
            r#"
[package]
name = "app"
version = "0.1.0"

[dependencies]
serde = { workspace = true }
"#,
        )?;
        std::fs::write(
            dir.path().join("crates/lib/Cargo.toml"),
            r#"
[package]
name = "lib"
version = "0.1.0"

[dependencies]
libc = "0.2"
"#,
        )?;

        let stacks = StackDetector::default().detect(&context(dir.path()))?;
        assert_eq!(
            stacks.len(),
            2,
            "raiz virtual vira um stack por membro: {stacks:?}"
        );
        // Deterministic order: sorted by manifest (app < lib).
        let app = &stacks[0];
        assert!(app.manifest.ends_with("crates/app/Cargo.toml"));
        assert_eq!(app.name.as_deref(), Some("app"));
        let serde = app
            .dependencies
            .iter()
            .find(|d| d.name == "serde")
            .expect("serde listado");
        assert_eq!(
            serde.version.as_deref(),
            Some("1.7.99"),
            "workspace = true resolve no [workspace.dependencies]"
        );
        assert!(stacks[1].manifest.ends_with("crates/lib/Cargo.toml"));
        Ok(())
    }

    #[test]
    fn workspace_true_without_a_root_stays_none_never_invented(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[package]
name = "solo"
version = "0.1.0"

[dependencies]
serde = { workspace = true }
"#,
        )?;

        let stacks = StackDetector::default().detect(&context(dir.path()))?;
        assert_eq!(stacks.len(), 1);
        let serde = stacks[0]
            .dependencies
            .iter()
            .find(|d| d.name == "serde")
            .unwrap();
        assert_eq!(
            serde.version, None,
            "sem workspace raiz = None honesto, nunca uma versão inventada"
        );
        Ok(())
    }

    #[test]
    fn detects_node_typescript() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        std::fs::write(
            dir.path().join("package.json"),
            r#"{
                "dependencies": { "react": "^18.0.0" },
                "devDependencies": { "typescript": "^5.0.0", "vite": "^5.0.0" }
            }"#,
        )?;

        let stacks = StackDetector::default().detect(&context(dir.path()))?;
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].language, Language::TypeScript);
        assert!(stacks[0].frameworks.contains(&"React".to_string()));
        assert!(stacks[0].frameworks.contains(&"Vite".to_string()));
        Ok(())
    }

    #[test]
    fn detects_python_and_ignores_comments() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        std::fs::write(
            dir.path().join("requirements.txt"),
            "Django>=4.2\nrequests==2.31.0\n# um comentário\n\n",
        )?;

        let stacks = StackDetector::default().detect(&context(dir.path()))?;
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].language, Language::Python);
        assert!(stacks[0].frameworks.contains(&"Django".to_string()));
        assert_eq!(stacks[0].dependencies.len(), 2);
        Ok(())
    }

    #[test]
    fn detects_go() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        std::fs::write(
            dir.path().join("go.mod"),
            "module exemplo\n\ngo 1.22\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.9.0\n\tgorm.io/gorm v1.25.0\n)\n",
        )?;

        let stacks = StackDetector::default().detect(&context(dir.path()))?;
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].language, Language::Go);
        assert!(stacks[0].frameworks.contains(&"Gin".to_string()));
        assert!(stacks[0].frameworks.contains(&"GORM".to_string()));
        Ok(())
    }

    #[test]
    fn makefile_alone_does_not_become_cpp() -> Result<(), Box<dyn std::error::Error>> {
        // Regression: a Go Makefile must not become a C/C++ stack.
        let dir = tempfile::tempdir()?;
        std::fs::write(dir.path().join("go.mod"), "module exemplo\n\ngo 1.22\n")?;
        std::fs::write(dir.path().join("Makefile"), "build:\n\tgo build\n")?;

        let stacks = StackDetector::default().detect(&context(dir.path()))?;
        assert_eq!(stacks.len(), 1, "não deve inventar stack extra: {stacks:?}");
        assert_eq!(stacks[0].language, Language::Go);
        Ok(())
    }

    #[test]
    fn detects_cxx_with_real_source() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        std::fs::write(dir.path().join("main.cpp"), "int main(){return 0;}")?;

        let stacks = StackDetector::default().detect(&context(dir.path()))?;
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].language, Language::Cpp);
        Ok(())
    }

    #[test]
    fn detects_multiple_stacks() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"app\"\n")?;
        std::fs::write(dir.path().join("package.json"), r#"{ "dependencies": { "vue": "^3" } }"#)?;

        let stacks = StackDetector::default().detect(&context(dir.path()))?;
        assert_eq!(stacks.len(), 2);
        Ok(())
    }

    #[test]
    fn recursive_finds_nested_manifest() -> Result<(), Box<dyn std::error::Error>> {
        // Root without a manifest; real project inside `pkgs/api/package.json`.
        let dir = tempfile::tempdir()?;
        let nested = dir.path().join("pkgs/api");
        std::fs::create_dir_all(&nested)?;
        std::fs::write(
            nested.join("package.json"),
            r#"{ "name": "api", "dependencies": { "express": "^4" } }"#,
        )?;

        // Default (non-recursive): does not descend, finds nothing.
        let flat = StackDetector::default().detect(&context(dir.path()))?;
        assert!(flat.is_empty(), "sem --all não deve descer: {flat:?}");

        // Recursive: finds the nested manifest.
        let deep = StackDetector { recursive: true }.detect(&context(dir.path()))?;
        assert_eq!(deep.len(), 1, "deve achar 1 stack: {deep:?}");
        assert_eq!(deep[0].language, Language::JavaScript);
        assert!(deep[0].frameworks.contains(&"Express".to_string()));
        Ok(())
    }

    #[test]
    fn recursive_skips_node_modules() -> Result<(), Box<dyn std::error::Error>> {
        // A bunch of package.json files in node_modules must NOT become a stack.
        let dir = tempfile::tempdir()?;
        let dep = dir.path().join("node_modules/foo");
        std::fs::create_dir_all(&dep)?;
        std::fs::write(dep.join("package.json"), r#"{ "name": "foo" }"#)?;
        std::fs::write(
            dir.path().join("package.json"),
            r#"{ "name": "raiz" }"#,
        )?;

        let stacks = StackDetector { recursive: true }.detect(&context(dir.path()))?;
        assert_eq!(stacks.len(), 1, "só a raiz, não node_modules: {stacks:?}");
        Ok(())
    }

    #[test]
    fn pure_c_does_not_become_cpp() -> Result<(), Box<dyn std::error::Error>> {
        // Only `.c`/`.h` ⇒ must be C, not C++ (false positive fixed).
        let dir = tempfile::tempdir()?;
        std::fs::write(
            dir.path().join("CMakeLists.txt"),
            "project(demo C)\nadd_executable(demo src/main.c)\n",
        )?;
        std::fs::create_dir_all(dir.path().join("src"))?;
        std::fs::write(dir.path().join("src/main.c"), "int main(){}")?;
        std::fs::write(dir.path().join("src/util.h"), "int f(void);")?;

        let stacks = StackDetector::default().detect(&context(dir.path()))?;
        assert_eq!(stacks[0].language, Language::C, "devia ser C: {:?}", stacks[0].language);
        Ok(())
    }

    #[test]
    fn cpp_with_cxx_source_is_cpp() -> Result<(), Box<dyn std::error::Error>> {
        // `.cpp` present ⇒ C++.
        let dir = tempfile::tempdir()?;
        std::fs::write(dir.path().join("CMakeLists.txt"), "project(demo CXX)\n")?;
        std::fs::write(dir.path().join("main.cpp"), "int main(){}")?;
        std::fs::write(dir.path().join("main.h"), "#pragma once")?;

        let stacks = StackDetector::default().detect(&context(dir.path()))?;
        assert_eq!(stacks[0].language, Language::Cpp);
        Ok(())
    }

    #[test]
    fn gradle_kts_extracts_dependency() -> Result<(), Box<dyn std::error::Error>> {
        // Kotlin DSL format: implementation("g:a:v") — the old regex got stuck on `("`.
        let dir = tempfile::tempdir()?;
        std::fs::write(
            dir.path().join("build.gradle.kts"),
            "dependencies {\n    implementation(\"com.squareup.okhttp3:okhttp:4.10.0\")\n}\n",
        )?;

        let stacks = StackDetector::default().detect(&context(dir.path()))?;
        let names: Vec<_> = stacks[0].dependencies.iter().map(|d| d.name.clone()).collect();
        assert!(names.contains(&"com.squareup.okhttp3:okhttp".to_string()), "deps: {names:?}");
        Ok(())
    }
}
