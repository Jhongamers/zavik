//! Project briefing (the `context` command): how to **build/test/run** and
//! where the **entrypoints** and the **key files** are.
//!
//! Goal: save the time (and the tokens) of an AI that would otherwise burn a
//! ton of `glob`/`grep`/README reading to figure out "how do I run this".
//! It is pure reading — nothing is executed.

use devforge_shared::{Briefing, KeyFile, Language, ProjectStack, Runbook};
use std::path::{Path, PathBuf};

/// Builds the briefing from the already-detected stacks.
pub fn extract_briefing(root: &Path, stacks: &[ProjectStack]) -> Briefing {
    let mut briefing = Briefing::default();
    for stack in stacks {
        build_runbook(root, stack, &mut briefing.runbook);
        collect_entrypoints(root, stack, &mut briefing.entrypoints);
    }
    // Makefile/justfile apply to any language (generic targets).
    makefile_runbook(root, &mut briefing.runbook);
    briefing.entrypoints.sort();
    briefing.entrypoints.dedup();
    briefing.key_files = collect_key_files(root, stacks);
    briefing
}

// ---------------------------------------------------------------------------
// Runbook
// ---------------------------------------------------------------------------

fn build_runbook(root: &Path, stack: &ProjectStack, rb: &mut Runbook) {
    match stack.language {
        Language::Rust => {
            push(&mut rb.setup, "cargo fetch");
            push(&mut rb.build, "cargo build");
            push(&mut rb.test, "cargo test");
            push(&mut rb.run, "cargo run");
            push(&mut rb.lint, "cargo clippy");
        }
        Language::JavaScript | Language::TypeScript => js_runbook(stack, rb),
        Language::Go => {
            push(&mut rb.setup, "go mod download");
            push(&mut rb.build, "go build ./...");
            push(&mut rb.test, "go test ./...");
            push(&mut rb.run, "go run .");
        }
        Language::Python => python_runbook(root, stack, rb),
        Language::Java | Language::Kotlin => jvm_runbook(stack, rb),
        Language::Php => {
            push(&mut rb.setup, "composer install");
            push(&mut rb.test, "composer test");
        }
        Language::Ruby => {
            push(&mut rb.setup, "bundle install");
            push(&mut rb.test, "bundle exec rake test");
        }
        Language::CSharp => {
            push(&mut rb.setup, "dotnet restore");
            push(&mut rb.build, "dotnet build");
            push(&mut rb.test, "dotnet test");
            push(&mut rb.run, "dotnet run");
        }
        Language::Dart => {
            push(&mut rb.setup, "dart pub get");
            push(&mut rb.build, "dart compile exe .");
            push(&mut rb.test, "dart test");
            push(&mut rb.run, "dart run");
        }
        Language::Elixir => {
            push(&mut rb.setup, "mix deps.get");
            push(&mut rb.build, "mix compile");
            push(&mut rb.test, "mix test");
            push(&mut rb.run, "mix run");
        }
        Language::Swift => {
            push(&mut rb.setup, "swift package resolve");
            push(&mut rb.build, "swift build");
            push(&mut rb.test, "swift test");
            push(&mut rb.run, "swift run");
        }
        Language::C | Language::Cpp => {
            push(&mut rb.setup, "cmake -B build");
            push(&mut rb.build, "cmake --build build");
            push(&mut rb.test, "ctest --test-dir build");
        }
        Language::Unknown => {}
    }
}

/// Node/TS: the `package.json` `scripts` are the source of truth.
fn js_runbook(stack: &ProjectStack, rb: &mut Runbook) {
    let pm = stack.package_manager.as_deref().unwrap_or("npm");
    push(&mut rb.setup, format!("{pm} install"));

    let Some(scripts) = package_json_scripts(&stack.manifest) else {
        return;
    };
    let has = |k: &str| scripts.iter().any(|s| s == k);
    if has("build") {
        push(&mut rb.build, format!("{pm} run build"));
    }
    if has("test") {
        push(&mut rb.test, format!("{pm} test"));
    }
    if has("start") {
        push(&mut rb.run, format!("{pm} start"));
    } else if has("dev") {
        push(&mut rb.run, format!("{pm} run dev"));
    } else if has("serve") {
        push(&mut rb.run, format!("{pm} run serve"));
    }
    if has("lint") {
        push(&mut rb.lint, format!("{pm} run lint"));
    } else if has("format") {
        push(&mut rb.lint, format!("{pm} run format"));
    }
}

fn python_runbook(root: &Path, stack: &ProjectStack, rb: &mut Runbook) {
    let dir = manifest_dir(&stack.manifest);
    if dir.join("poetry.lock").is_file() {
        push(&mut rb.setup, "poetry install");
    } else if dir.join("requirements.txt").is_file() {
        push(&mut rb.setup, "pip install -r requirements.txt");
    } else if dir.join("pyproject.toml").is_file() {
        push(&mut rb.setup, "pip install -e .");
    }
    push(&mut rb.test, "pytest");
    if let Some(entry) = first_existing(root, &["main.py", "app.py", "manage.py", "cli.py"]) {
        push(&mut rb.run, format!("python {entry}"));
    }
}

fn jvm_runbook(stack: &ProjectStack, rb: &mut Runbook) {
    let dir = manifest_dir(&stack.manifest);
    let maven = stack.package_manager.as_deref() == Some("maven");
    let wrapper = if maven {
        first_existing(&dir, &["mvnw"])
    } else {
        first_existing(&dir, &["gradlew"])
    };
    let tool = match wrapper {
        Some(w) => format!("./{w}"),
        None if maven => "mvn".to_string(),
        None => "gradle".to_string(),
    };
    push(&mut rb.setup, format!("{tool} -q dependency:resolve"));
    push(&mut rb.build, format!("{tool} package"));
    push(&mut rb.test, format!("{tool} test"));
}

/// Makefile/justfile targets apply to any project.
fn makefile_runbook(root: &Path, rb: &mut Runbook) {
    for name in ["Makefile", "makefile", "GNUmakefile", "justfile", "Justfile"] {
        let path = root.join(name);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let runner = if name.to_lowercase().contains("just") {
            "just"
        } else {
            "make"
        };
        for target in parse_make_targets(&text) {
            match target.as_str() {
                "build" | "all" | "compile" => {
                    push(&mut rb.build, format!("{runner} {target}"))
                }
                "test" | "check" => push(&mut rb.test, format!("{runner} {target}")),
                "run" | "start" | "dev" => push(&mut rb.run, format!("{runner} {target}")),
                "install" | "deps" | "setup" => {
                    push(&mut rb.setup, format!("{runner} {target}"))
                }
                "lint" | "fmt" | "format" => push(&mut rb.lint, format!("{runner} {target}")),
                _ => {}
            }
        }
        break; // one Makefile is enough
    }
}

// ---------------------------------------------------------------------------
// Entrypoints
// ---------------------------------------------------------------------------

fn collect_entrypoints(root: &Path, stack: &ProjectStack, out: &mut Vec<String>) {
    let dir = manifest_dir(&stack.manifest);
    let candidates: &[&str] = match stack.language {
        Language::Rust => &["src/main.rs", "src/lib.rs"],
        Language::Go => &["main.go", "server.go"],
        Language::Python => &["main.py", "app.py", "manage.py", "cli.py"],
        Language::JavaScript | Language::TypeScript => &[
            "index.js",
            "index.ts",
            "src/index.js",
            "src/index.ts",
            "server.js",
            "app.js",
            "src/main.ts",
        ],
        Language::Java | Language::Kotlin => &["src/main/java", "src/main/kotlin"],
        Language::CSharp => &["Program.cs"],
        Language::Dart => &["main.dart", "bin"],
        Language::Elixir => &["lib", "mix.exs"],
        Language::Swift => &["main.swift", "Sources"],
        Language::Php => &["index.php", "public/index.php", "artisan"],
        Language::Ruby => &["config.ru", "app.rb"],
        Language::C | Language::Cpp => &["main.c", "main.cpp"],
        Language::Unknown => &[],
    };
    for cand in candidates {
        let path = dir.join(cand);
        if path.exists() {
            out.push(rel(root, &path));
        }
    }
}

// ---------------------------------------------------------------------------
// Key files
// ---------------------------------------------------------------------------

fn collect_key_files(root: &Path, stacks: &[ProjectStack]) -> Vec<KeyFile> {
    let mut files = Vec::new();
    for stack in stacks {
        files.push(KeyFile {
            kind: "manifest".into(),
            path: rel(root, &stack.manifest),
        });
        let dir = manifest_dir(&stack.manifest);
        for lock in lockfiles_for(stack.language) {
            if dir.join(lock).is_file() {
                files.push(KeyFile {
                    kind: "lockfile".into(),
                    path: rel(root, &dir.join(lock)),
                });
            }
        }
    }
    for (kind, rel_path) in [
        ("ci", ".github/workflows"),
        ("ci", ".gitlab-ci.yml"),
        ("ci", ".circleci/config.yml"),
        ("ci", "Jenkinsfile"),
        ("ci", ".travis.yml"),
        ("ci", "azure-pipelines.yml"),
        ("ci", "bitbucket-pipelines.yml"),
        ("docker", "Dockerfile"),
        ("docker", "docker-compose.yml"),
        ("docker", "compose.yml"),
        ("env", ".env.example"),
        ("env", ".env.sample"),
        ("readme", "README.md"),
        ("readme", "README"),
    ] {
        if root.join(rel_path).exists() {
            files.push(KeyFile {
                kind: kind.into(),
                path: rel_path.into(),
            });
        }
    }
    files.sort_by(|a, b| (&a.kind, &a.path).cmp(&(&b.kind, &b.path)));
    files.dedup_by(|a, b| a.kind == b.kind && a.path == b.path);
    files
}

fn lockfiles_for(language: Language) -> &'static [&'static str] {
    match language {
        Language::JavaScript | Language::TypeScript => &[
            "package-lock.json",
            "yarn.lock",
            "pnpm-lock.yaml",
            "bun.lockb",
            "bun.lock",
        ],
        Language::Rust => &["Cargo.lock"],
        Language::Python => &["poetry.lock", "Pipfile.lock", "uv.lock"],
        Language::Go => &["go.sum"],
        Language::Php => &["composer.lock"],
        Language::Ruby => &["Gemfile.lock"],
        Language::CSharp => &["packages.lock.json"],
        Language::Elixir => &["mix.lock"],
        Language::Dart => &["pubspec.lock"],
        _ => &[],
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Targets of a Makefile (lines `nome:` at the start, without tab/assignment).
fn parse_make_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for line in text.lines() {
        if line.starts_with(['\t', ' ', '#']) {
            continue;
        }
        if let Some(name) = line.split(':').next() {
            let name = name.trim();
            if name.is_empty() || name.contains(' ') || name.contains('=') || name.starts_with('.')
            {
                continue;
            }
            targets.push(name.to_string());
        }
    }
    targets
}

/// Keys of `scripts` in a `package.json`.
fn package_json_scripts(manifest: &Path) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(manifest).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let scripts = json.get("scripts")?.as_object()?;
    Some(scripts.keys().cloned().collect())
}

fn manifest_dir(manifest: &Path) -> PathBuf {
    manifest.parent().unwrap_or(Path::new(".")).to_path_buf()
}

fn first_existing(dir: &Path, candidates: &[&str]) -> Option<String> {
    candidates
        .iter()
        .find(|c| dir.join(c).exists())
        .map(|c| (*c).to_string())
}

fn push(bucket: &mut Vec<String>, cmd: impl Into<String>) {
    let cmd = cmd.into();
    if !bucket.contains(&cmd) {
        bucket.push(cmd);
    }
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use devforge_shared::Dependency;

    #[test]
    fn parse_makefile_targets() {
        let text = "CC = gcc\nbuild:\n\tgcc main.c\n.PHONY: build\ntest:\n\trun_tests\n";
        let targets = parse_make_targets(text);
        assert!(targets.contains(&"build".to_string()));
        assert!(targets.contains(&"test".to_string()));
        assert!(!targets.iter().any(|t| t.contains('=') || t.starts_with('.')));
    }

    #[test]
    fn briefing_node_uses_scripts_and_finds_entry() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{ "scripts": { "build": "vite build", "test": "vitest" } }"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("index.js"), "console.log(1)").unwrap();

        let manifest = dir.path().join("package.json");
        let stack = ProjectStack::new(
            Language::JavaScript,
            manifest,
            Some("npm".to_string()),
        )
        .with_dependencies(vec![Dependency::new("react", Some("^18".into()), devforge_shared::DependencyKind::Runtime)]);

        let briefing = extract_briefing(dir.path(), &[stack]);
        assert!(briefing.runbook.setup.contains(&"npm install".to_string()));
        assert!(briefing.runbook.build.contains(&"npm run build".to_string()));
        assert!(briefing.runbook.test.contains(&"npm test".to_string()));
        assert!(briefing.entrypoints.contains(&"index.js".to_string()));
        assert!(briefing
            .key_files
            .iter()
            .any(|k| k.kind == "manifest" && k.path == "package.json"));
    }
}
