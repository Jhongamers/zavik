//! **Local** preparation recipes (no Docker), per language.
//!
//! Each function returns the steps needed to download/install the project's
//! dependencies on the user's machine.

use super::{ProvisionAction, ProvisionStep};
use devforge_shared::{Language, ProjectStack};
use devforge_sdk::CommandSpec;
use std::path::Path;

/// Local steps for a stack.
pub(crate) fn steps_for(
    stack: &ProjectStack,
    root: &Path,
    lang: devforge_shared::i18n::Lang,
) -> Vec<ProvisionStep> {
    use devforge_shared::i18n::{t_fmt_in, translate};
    let manifest = stack
        .manifest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    match stack.language {
        Language::Rust => vec![run(
            translate(lang, "Baixar e compilar dependências (cargo)"),
            translate(lang, "Compila o projeto e baixa todas as crates declaradas."),
            "cargo",
            vec!["build"],
            root,
        )],

        Language::JavaScript | Language::TypeScript => {
            let pm = stack.package_manager.as_deref().unwrap_or("npm");
            vec![run(
                &t_fmt_in(lang, "Instalar dependências ({})", &[pm]),
                translate(lang, "Baixa e instala os pacotes do package.json."),
                pm,
                vec!["install".to_string()],
                root,
            )]
        }

        Language::Python => python_steps(stack, manifest, root, lang),

        Language::Go => vec![run(
            translate(lang, "Baixar módulos (go mod download)"),
            translate(lang, "Baixa os módulos declarados no go.mod."),
            "go",
            vec!["mod".to_string(), "download".to_string()],
            root,
        )],

        Language::Java | Language::Kotlin => jvm_steps(stack, manifest, root, lang),

        Language::Php => vec![run(
            translate(lang, "Instalar dependências (composer install)"),
            translate(lang, "Instala os pacotes do composer.json."),
            "composer",
            vec!["install".to_string()],
            root,
        )],

        Language::Ruby => vec![run(
            translate(lang, "Instalar gems (bundle install)"),
            translate(lang, "Instala as gems do Gemfile."),
            "bundle",
            vec!["install".to_string()],
            root,
        )],

        Language::CSharp => vec![
            run(
                translate(lang, "Restaurar pacotes (.NET)"),
                translate(lang, "Baixa os pacotes NuGet do projeto."),
                "dotnet",
                vec!["restore".to_string()],
                root,
            ),
            run(
                translate(lang, "Compilar projeto (.NET)"),
                translate(lang, "Compila a solução em modo Debug."),
                "dotnet",
                vec!["build".to_string()],
                root,
            ),
        ],

        Language::Dart => {
            let program = if has_framework(stack, "Flutter") {
                "flutter"
            } else {
                "dart"
            };
            vec![run(
                &t_fmt_in(lang, "Baixar pacotes ({} pub get)", &[program]),
                translate(lang, "Baixa as dependências do pubspec.yaml."),
                program,
                vec!["pub".to_string(), "get".to_string()],
                root,
            )]
        }

        Language::Elixir => vec![
            run(
                translate(lang, "Baixar dependências (mix deps.get)"),
                translate(lang, "Baixa as dependências do mix.exs."),
                "mix",
                vec!["deps.get".to_string()],
                root,
            ),
            run(
                translate(lang, "Compilar (mix compile)"),
                translate(lang, "Compila o projeto Elixir."),
                "mix",
                vec!["compile".to_string()],
                root,
            ),
        ],

        Language::Swift => vec![run(
            translate(lang, "Compilar pacote (swift build)"),
            translate(lang, "Resolve e compila as dependências do Package.swift."),
            "swift",
            vec!["build".to_string()],
            root,
        )],

        Language::C | Language::Cpp => vec![
            run(
                translate(lang, "Configurar build (cmake)"),
                translate(lang, "Gera os arquivos de build na pasta build/."),
                "cmake",
                vec![
                    "-S".to_string(),
                    ".".to_string(),
                    "-B".to_string(),
                    "build".to_string(),
                ],
                root,
            ),
            run(
                translate(lang, "Compilar (cmake --build)"),
                translate(lang, "Compila o projeto."),
                "cmake",
                vec!["--build".to_string(), "build".to_string()],
                root,
            ),
        ],

        Language::Unknown => Vec::new(),
    }
}

fn python_steps(
    stack: &ProjectStack,
    manifest: &str,
    root: &Path,
    lang: devforge_shared::i18n::Lang,
) -> Vec<ProvisionStep> {
    use devforge_shared::i18n::translate;
    match stack.package_manager.as_deref() {
        Some("poetry") => vec![run(
            translate(lang, "Instalar dependências (poetry install)"),
            translate(lang, "Cria o ambiente virtual e instala as dependências."),
            "poetry",
            vec!["install".to_string()],
            root,
        )],
        Some("pipenv") => vec![run(
            translate(lang, "Instalar dependências (pipenv install)"),
            translate(lang, "Cria o ambiente virtual e instala as dependências."),
            "pipenv",
            vec!["install".to_string()],
            root,
        )],
        _ => {
            // Virtual environment + installation.
            let mut steps = vec![run(
                translate(lang, "Criar ambiente virtual (python venv)"),
                translate(lang, "Cria a pasta .venv isolando as dependências do sistema."),
                "python3",
                vec!["-m".to_string(), "venv".to_string(), ".venv".to_string()],
                root,
            )];

            let pip = if cfg!(windows) {
                ".venv/Scripts/pip"
            } else {
                ".venv/bin/pip"
            };
            let mut args = vec!["install".to_string()];
            match manifest {
                "requirements.txt" => {
                    args.push("-r".to_string());
                    args.push("requirements.txt".to_string());
                }
                "setup.py" => args.push(".".to_string()),
                "pyproject.toml" => args.push(".".to_string()),
                _ => {}
            }
            if args.len() > 1 {
                steps.push(run(
                    translate(lang, "Instalar dependências (pip install)"),
                    translate(lang, "Instala as dependências do projeto no ambiente virtual."),
                    pip,
                    args,
                    root,
                ));
            }
            steps
        }
    }
}

fn jvm_steps(
    stack: &ProjectStack,
    manifest: &str,
    root: &Path,
    lang: devforge_shared::i18n::Lang,
) -> Vec<ProvisionStep> {
    use devforge_shared::i18n::translate;
    match stack.package_manager.as_deref() {
        Some("gradle") => {
            // Prefers the `./gradlew` wrapper when present.
            let program = if root.join("gradlew").is_file() {
                "./gradlew"
            } else {
                "gradle"
            };
            let _ = manifest;
            vec![run(
                translate(lang, "Compilar (gradle build)"),
                translate(lang, "Baixa as dependências e compila o projeto."),
                program,
                vec!["build".to_string()],
                root,
            )]
        }
        _ => vec![run(
            translate(lang, "Baixar dependências (maven)"),
            translate(lang, "Resolve as dependências e compila sem rodar testes."),
            "mvn",
            vec![
                "-q".to_string(),
                "-DskipTests".to_string(),
                "install".to_string(),
            ],
            root,
        )],
    }
}

/// Hints on "how to run" after preparation.
pub(crate) fn run_hints_for(stack: &ProjectStack) -> Vec<String> {
    let hint = match stack.language {
        // NOVO-1 (corpus Fase A, docs/dogfooding.md): a package without
        // binary targets cannot `cargo run` — no hint beats a wrong one.
        Language::Rust => {
            if !has_cargo_bin(stack.manifest.parent().unwrap_or(std::path::Path::new("."))) {
                return Vec::new();
            }
            "cargo run".to_string()
        }
        Language::JavaScript | Language::TypeScript => {
            format!(
                "{} start",
                stack.package_manager.as_deref().unwrap_or("npm")
            )
        }
        Language::Python => "python3 main.py  (ou o seu entrypoint)".to_string(),
        Language::Go => "go run .".to_string(),
        Language::Java | Language::Kotlin => "mvn spring-boot:run  (ou java -jar target/*.jar)".to_string(),
        Language::Php => "php -S localhost:8000 -t public".to_string(),
        Language::Ruby => "bundle exec rails server".to_string(),
        Language::CSharp => "dotnet run".to_string(),
        Language::Dart => {
            if has_framework(stack, "Flutter") {
                "flutter run".to_string()
            } else {
                "dart run".to_string()
            }
        }
        Language::Elixir => "mix phx.server".to_string(),
        Language::Swift => "swift run".to_string(),
        Language::C | Language::Cpp => "./build/app".to_string(),
        Language::Unknown => return Vec::new(),
    };
    vec![hint]
}

/// Whether this package would expose a binary to `cargo run`: an explicit
/// `[[bin]]`, `src/main.rs`, or `src/bin/*`. Package scope, textual
/// `[[bin]]` check (core carries no TOML parser); the runner in plugins
/// keeps the parsing twin — both pinned by tests (NOVO-1).
fn has_cargo_bin(dir: &std::path::Path) -> bool {
    if let Ok(text) = std::fs::read_to_string(dir.join("Cargo.toml")) {
        if text.contains("[[bin]]") {
            return true;
        }
    }
    if dir.join("src").join("main.rs").exists() {
        return true;
    }
    if let Ok(entries) = std::fs::read_dir(dir.join("src").join("bin")) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_rs = path.extension().and_then(|e| e.to_str()) == Some("rs");
            if is_rs || (path.is_dir() && path.join("main.rs").exists()) {
                return true;
            }
        }
    }
    false
}

fn has_framework(stack: &ProjectStack, name: &str) -> bool {
    stack.frameworks.iter().any(|f| f == name)
}

/// Creates a command step that requires permission.
fn run(
    title: &str,
    reason: &str,
    program: &str,
    args: impl IntoIterator<Item = impl Into<String>>,
    root: &Path,
) -> ProvisionStep {
    let spec = CommandSpec::new(program, args, root);
    ProvisionStep {
        title: title.to_string(),
        detail: spec.display(),
        reason: reason.to_string(),
        requires_permission: true,
        action: ProvisionAction::RunCommand(spec),
    }
}
