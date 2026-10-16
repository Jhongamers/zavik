//! Detectors for interpreted / dynamic languages.
//!
//! Covers: Node (JavaScript/TypeScript), Python, PHP, Ruby, Dart/Flutter and
//! Elixir.

use super::{dep_names, find_file, match_frameworks, read, MatchMode};
use devforge_shared::{Dependency, DependencyKind, Error, Language, ProjectStack};
use std::path::{Path, PathBuf};

/// Runs all detectors in this module.
pub(crate) fn detect_all(root: &Path) -> Result<Vec<ProjectStack>, Error> {
    let mut stacks = Vec::new();
    if let Some(stack) = detect_node(root)? {
        stacks.push(stack);
    }
    if let Some(stack) = detect_python(root)? {
        stacks.push(stack);
    }
    if let Some(stack) = detect_php(root)? {
        stacks.push(stack);
    }
    if let Some(stack) = detect_ruby(root)? {
        stacks.push(stack);
    }
    if let Some(stack) = detect_dart(root)? {
        stacks.push(stack);
    }
    if let Some(stack) = detect_elixir(root)? {
        stacks.push(stack);
    }
    Ok(stacks)
}

// --- Node (JavaScript / TypeScript) ----------------------------------------

const NODE_FRAMEWORKS: &[(&str, &[&str])] = &[
    ("React", &["react"]),
    ("Next.js", &["next"]),
    ("Vue", &["vue"]),
    ("Nuxt", &["nuxt"]),
    ("Angular", &["@angular/core"]),
    ("Svelte", &["svelte", "@sveltejs/kit"]),
    ("Astro", &["astro"]),
    ("Remix", &["@remix-run/react"]),
    ("Express", &["express"]),
    ("NestJS", &["@nestjs/core"]),
    ("Electron", &["electron"]),
    ("Tauri", &["@tauri-apps/api", "@tauri-apps/cli"]),
    ("Vite", &["vite"]),
    ("jQuery", &["jquery"]),
];

fn detect_node(root: &Path) -> Result<Option<ProjectStack>, Error> {
    let Some(manifest) = find_file(root, "package.json") else {
        return Ok(None);
    };

    let text = read(&manifest)?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| Error::SerializationError(format!("package.json: {e}")))?;

    let mut deps = Vec::new();
    collect_json_deps(&value, "dependencies", DependencyKind::Runtime, &mut deps);
    collect_json_deps(&value, "devDependencies", DependencyKind::Development, &mut deps);
    collect_json_deps(&value, "peerDependencies", DependencyKind::Peer, &mut deps);
    collect_json_deps(&value, "optionalDependencies", DependencyKind::Runtime, &mut deps);

    let names = dep_names(&deps);
    let is_typescript =
        names.iter().any(|n| n == "typescript") || root.join("tsconfig.json").is_file();
    let language = if is_typescript {
        Language::TypeScript
    } else {
        Language::JavaScript
    };

    let name = value.get("name").and_then(|v| v.as_str()).map(str::to_string);
    Ok(Some(
        ProjectStack::new(language, manifest, detect_node_package_manager(root))
            .with_name(name)
            .with_frameworks(match_frameworks(&names, MatchMode::Exact, NODE_FRAMEWORKS))
            .with_dependencies(deps),
    ))
}

fn collect_json_deps(
    value: &serde_json::Value,
    key: &str,
    kind: DependencyKind,
    out: &mut Vec<Dependency>,
) {
    if let Some(map) = value.get(key).and_then(|v| v.as_object()) {
        for (name, version) in map {
            out.push(Dependency::new(
                name.clone(),
                version.as_str().map(str::to_string),
                kind,
            ));
        }
    }
}

fn detect_node_package_manager(root: &Path) -> Option<String> {
    for (lockfile, manager) in [
        ("pnpm-lock.yaml", "pnpm"),
        ("yarn.lock", "yarn"),
        ("bun.lockb", "bun"),
        ("bun.lock", "bun"),
        ("package-lock.json", "npm"),
    ] {
        if root.join(lockfile).is_file() {
            return Some(manager.to_string());
        }
    }
    Some("npm".to_string())
}

// --- Python ----------------------------------------------------------------

const PYTHON_FRAMEWORKS: &[(&str, &[&str])] = &[
    ("Django", &["django"]),
    ("Flask", &["flask"]),
    ("FastAPI", &["fastapi"]),
    ("Starlette", &["starlette"]),
    ("Tornado", &["tornado"]),
    ("Scrapy", &["scrapy"]),
    ("Celery", &["celery"]),
    ("Streamlit", &["streamlit"]),
    ("Pytest", &["pytest"]),
    ("SQLAlchemy", &["sqlalchemy"]),
];

fn detect_python(root: &Path) -> Result<Option<ProjectStack>, Error> {
    let mut deps = Vec::new();
    let mut manifest: Option<PathBuf> = None;
    let mut package_manager: Option<String> = None;
    let mut name: Option<String> = None;

    if let Some(path) = find_file(root, "pyproject.toml") {
        let text = read(&path)?;
        let value: toml::Value = text
            .parse()
            .map_err(|e| Error::SerializationError(format!("pyproject.toml: {e}")))?;

        name = value
            .get("project")
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())
            .or_else(|| {
                value
                    .get("tool")
                    .and_then(|t| t.get("poetry"))
                    .and_then(|p| p.get("name"))
                    .and_then(|v| v.as_str())
            })
            .map(str::to_string);

        // PEP 621 standard: [project] dependencies = ["django>=4.2", ...]
        if let Some(list) = value.get("project").and_then(|p| p.get("dependencies")).and_then(|d| d.as_array())
        {
            for item in list.iter().filter_map(|i| i.as_str()) {
                let (name, version) = parse_requirement(item);
                if !name.is_empty() {
                    deps.push(Dependency::new(name, version, DependencyKind::Runtime));
                }
            }
        }
        if let Some(groups) = value
            .get("project")
            .and_then(|p| p.get("optional-dependencies"))
            .and_then(|d| d.as_table())
        {
            for list in groups.values().filter_map(|v| v.as_array()) {
                for item in list.iter().filter_map(|i| i.as_str()) {
                    let (name, version) = parse_requirement(item);
                    if !name.is_empty() {
                        deps.push(Dependency::new(name, version, DependencyKind::Development));
                    }
                }
            }
        }

        // Poetry: [tool.poetry.dependencies]
        if let Some(poetry) = value.get("tool").and_then(|t| t.get("poetry")) {
            package_manager = Some("poetry".to_string());
            if let Some(table) = poetry.get("dependencies").and_then(|d| d.as_table()) {
                for (name, spec) in table {
                    if name == "python" {
                        continue;
                    }
                    let version = spec
                        .as_str()
                        .map(str::to_string)
                        .or_else(|| spec.get("version").and_then(|v| v.as_str()).map(str::to_string));
                    deps.push(Dependency::new(name.clone(), version, DependencyKind::Runtime));
                }
            }
        }

        manifest = Some(path);
    }

    if let Some(path) = find_file(root, "requirements.txt") {
        let text = read(&path)?;
        for (name, version) in parse_requirements_file(&text) {
            deps.push(Dependency::new(name, version, DependencyKind::Runtime));
        }
        manifest.get_or_insert(path);
    }

    if let Some(path) = find_file(root, "Pipfile") {
        let text = read(&path)?;
        if let Ok(value) = text.parse::<toml::Value>() {
            for (table, kind) in [
                ("packages", DependencyKind::Runtime),
                ("dev-packages", DependencyKind::Development),
            ] {
                if let Some(entries) = value.get(table).and_then(|v| v.as_table()) {
                    for (name, spec) in entries {
                        let version = spec
                            .as_str()
                            .filter(|s| *s != "*")
                            .map(str::to_string)
                            .or_else(|| spec.get("version").and_then(|v| v.as_str()).map(str::to_string));
                        deps.push(Dependency::new(name.clone(), version, kind));
                    }
                }
            }
        }
        package_manager = Some("pipenv".to_string());
        manifest.get_or_insert(path);
    }

    if let Some(path) = find_file(root, "setup.py") {
        manifest.get_or_insert(path);
        package_manager.get_or_insert_with(|| "setuptools".to_string());
    }

    let Some(manifest) = manifest else {
        return Ok(None);
    };

    let package_manager = package_manager.unwrap_or_else(|| "pip".to_string());
    let frameworks = match_frameworks(&dep_names(&deps), MatchMode::Exact, PYTHON_FRAMEWORKS);
    Ok(Some(
        ProjectStack::new(Language::Python, manifest, Some(package_manager))
            .with_name(name)
            .with_frameworks(frameworks)
            .with_dependencies(deps),
    ))
}

/// Parses a Python requirement line: `name[extras] spec ; marker`.
fn parse_requirement(spec: &str) -> (String, Option<String>) {
    let spec = spec.trim();
    let end = spec
        .find(|c: char| {
            matches!(c, '[' | '=' | '<' | '>' | '!' | '~' | ';' | ' ' | '(')
        })
        .unwrap_or(spec.len());
    let name = spec[..end].trim().to_string();

    let rest = spec[end..].trim();
    if rest.is_empty() {
        return (name, None);
    }
    // Removes extras `[...]` and discards the environment marker after `;`.
    let rest = if rest.starts_with('[') {
        rest.find(']').map(|i| rest[i + 1..].trim()).unwrap_or("")
    } else {
        rest
    };
    let version = rest.split(';').next().unwrap_or("").trim();
    let version = (!version.is_empty()).then(|| version.to_string());
    (name, version)
}

fn parse_requirements_file(text: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        // Ignores empty lines and flags (-r, -e, --hash ...).
        if line.is_empty() || line.starts_with('-') {
            continue;
        }
        let (name, version) = parse_requirement(line);
        if !name.is_empty() {
            out.push((name, version));
        }
    }
    out
}

// --- PHP -------------------------------------------------------------------

const PHP_FRAMEWORKS: &[(&str, &[&str])] = &[
    ("Laravel", &["laravel"]),
    ("Symfony", &["symfony"]),
    ("CakePHP", &["cakephp"]),
    ("Slim", &["slim/slim"]),
    ("CodeIgniter", &["codeigniter"]),
    ("WordPress", &["wordpress"]),
];

fn detect_php(root: &Path) -> Result<Option<ProjectStack>, Error> {
    let Some(manifest) = find_file(root, "composer.json") else {
        return Ok(None);
    };

    let text = read(&manifest)?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| Error::SerializationError(format!("composer.json: {e}")))?;

    let mut deps = Vec::new();
    for (key, kind) in [
        ("require", DependencyKind::Runtime),
        ("require-dev", DependencyKind::Development),
    ] {
        if let Some(map) = value.get(key).and_then(|v| v.as_object()) {
            for (name, version) in map {
                // Ignores the PHP runtime and native extensions.
                if name == "php" || name.starts_with("ext-") || name.starts_with("lib-") {
                    continue;
                }
                deps.push(Dependency::new(
                    name.clone(),
                    version.as_str().map(str::to_string),
                    kind,
                ));
            }
        }
    }

    let frameworks = match_frameworks(&dep_names(&deps), MatchMode::Contains, PHP_FRAMEWORKS);
    let name = value.get("name").and_then(|v| v.as_str()).map(str::to_string);
    Ok(Some(
        ProjectStack::new(Language::Php, manifest, Some("composer".to_string()))
            .with_name(name)
            .with_frameworks(frameworks)
            .with_dependencies(deps),
    ))
}

// --- Ruby ------------------------------------------------------------------

const RUBY_FRAMEWORKS: &[(&str, &[&str])] = &[
    ("Ruby on Rails", &["rails"]),
    ("Sinatra", &["sinatra"]),
    ("Hanami", &["hanami"]),
    ("Jekyll", &["jekyll"]),
];

fn detect_ruby(root: &Path) -> Result<Option<ProjectStack>, Error> {
    let Some(manifest) = find_file(root, "Gemfile") else {
        return Ok(None);
    };

    let text = read(&manifest)?;
    let gem_re = regex::Regex::new(r#"^\s*gem\s+['"]([^'"]+)['"](?:\s*,\s*['"]([^'"]+)['"])?"#)
        .expect("regex válido");

    let mut deps = Vec::new();
    for line in text.lines() {
        if let Some(capture) = gem_re.captures(line) {
            deps.push(Dependency::new(
                capture[1].to_string(),
                capture.get(2).map(|m| m.as_str().to_string()),
                DependencyKind::Runtime,
            ));
        }
    }

    let frameworks = match_frameworks(&dep_names(&deps), MatchMode::Exact, RUBY_FRAMEWORKS);
    Ok(Some(
        ProjectStack::new(Language::Ruby, manifest, Some("bundler".to_string()))
            .with_frameworks(frameworks)
            .with_dependencies(deps),
    ))
}

// --- Dart / Flutter --------------------------------------------------------

const DART_FRAMEWORKS: &[(&str, &[&str])] = &[("Flutter", &["flutter"])];

fn detect_dart(root: &Path) -> Result<Option<ProjectStack>, Error> {
    let Some(manifest) = find_file(root, "pubspec.yaml") else {
        return Ok(None);
    };

    let text = read(&manifest)?;
    let mut deps = Vec::new();
    let mut current: Option<DependencyKind> = None;
    let mut name: Option<String> = None;

    for raw in text.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = raw.len() - raw.trim_start().len();
        if indent == 0 {
            if let Some(value) = trimmed.strip_prefix("name:") {
                name = Some(value.trim().to_string());
            }
            // Only the dependency sections matter to us; any other key resets it.
            current = match trimmed.split(':').next().unwrap_or("").trim() {
                "dependencies" => Some(DependencyKind::Runtime),
                "dev_dependencies" => Some(DependencyKind::Development),
                _ => None,
            };
            continue;
        }
        if let Some(kind) = current {
            if let Some((name, value)) = trimmed.split_once(':') {
                let name = name.trim();
                let version = value.trim();
                if name.is_empty() || name.starts_with("sdk") {
                    continue;
                }
                let version = (!version.is_empty()).then(|| version.to_string());
                deps.push(Dependency::new(name.to_string(), version, kind));
            }
        }
    }

    let frameworks = match_frameworks(&dep_names(&deps), MatchMode::Exact, DART_FRAMEWORKS);
    Ok(Some(
        ProjectStack::new(Language::Dart, manifest, Some("pub".to_string()))
            .with_name(name)
            .with_frameworks(frameworks)
            .with_dependencies(deps),
    ))
}

// --- Elixir ----------------------------------------------------------------

const ELIXIR_FRAMEWORKS: &[(&str, &[&str])] = &[
    ("Phoenix", &["phoenix"]),
    ("Ecto", &["ecto"]),
    ("LiveView", &["phoenix_live_view"]),
];

fn detect_elixir(root: &Path) -> Result<Option<ProjectStack>, Error> {
    let Some(manifest) = find_file(root, "mix.exs") else {
        return Ok(None);
    };

    let text = read(&manifest)?;
    // Captures entries in the `deps` list: {:name, "~> 1.0"}.
    let dep_re = regex::Regex::new(r#"\{\s*:([a-zA-Z0-9_]+)\s*,\s*"([^"]*)""#).expect("regex válido");

    let mut deps = Vec::new();
    for capture in dep_re.captures_iter(&text) {
        deps.push(Dependency::new(
            capture[1].to_string(),
            Some(capture[2].to_string()),
            DependencyKind::Runtime,
        ));
    }

    let frameworks = match_frameworks(&dep_names(&deps), MatchMode::Contains, ELIXIR_FRAMEWORKS);
    Ok(Some(
        ProjectStack::new(Language::Elixir, manifest, Some("mix".to_string()))
            .with_frameworks(frameworks)
            .with_dependencies(deps),
    ))
}
