//! **Local** preparation recipes (no Docker), per language.
//!
//! Each function returns the steps needed to download/install the project's
//! dependencies on the user's machine.

use super::{ProvisionAction, ProvisionStep};
use devforge_shared::{Language, ProjectStack};
use devforge_sdk::CommandSpec;
use std::path::Path;

/// Local steps for a stack.
pub(crate) fn steps_for(stack: &ProjectStack, root: &Path) -> Vec<ProvisionStep> {
    let manifest = stack
        .manifest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    match stack.language {
        Language::Rust => vec![run(
            "Baixar e compilar dependências (cargo)",
            "Compila o projeto e baixa todas as crates declaradas.",
            "cargo",
            vec!["build"],
            root,
        )],

        Language::JavaScript | Language::TypeScript => {
            let pm = stack.package_manager.as_deref().unwrap_or("npm");
            vec![run(
                &format!("Instalar dependências ({pm})"),
                "Baixa e instala os pacotes do package.json.",
                pm,
                vec!["install".to_string()],
                root,
            )]
        }

        Language::Python => python_steps(stack, manifest, root),

        Language::Go => vec![run(
            "Baixar módulos (go mod download)",
            "Baixa os módulos declarados no go.mod.",
            "go",
            vec!["mod".to_string(), "download".to_string()],
            root,
        )],

        Language::Java | Language::Kotlin => jvm_steps(stack, manifest, root),

        Language::Php => vec![run(
            "Instalar dependências (composer install)",
            "Instala os pacotes do composer.json.",
            "composer",
            vec!["install".to_string()],
            root,
        )],

        Language::Ruby => vec![run(
            "Instalar gems (bundle install)",
            "Instala as gems do Gemfile.",
            "bundle",
            vec!["install".to_string()],
            root,
        )],

        Language::CSharp => vec![
            run(
                "Restaurar pacotes (.NET)",
                "Baixa os pacotes NuGet do projeto.",
                "dotnet",
                vec!["restore".to_string()],
                root,
            ),
            run(
                "Compilar projeto (.NET)",
                "Compila a solução em modo Debug.",
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
                &format!("Baixar pacotes ({program} pub get)"),
                "Baixa as dependências do pubspec.yaml.",
                program,
                vec!["pub".to_string(), "get".to_string()],
                root,
            )]
        }

        Language::Elixir => vec![
            run(
                "Baixar dependências (mix deps.get)",
                "Baixa as dependências do mix.exs.",
                "mix",
                vec!["deps.get".to_string()],
                root,
            ),
            run(
                "Compilar (mix compile)",
                "Compila o projeto Elixir.",
                "mix",
                vec!["compile".to_string()],
                root,
            ),
        ],

        Language::Swift => vec![run(
            "Compilar pacote (swift build)",
            "Resolve e compila as dependências do Package.swift.",
            "swift",
            vec!["build".to_string()],
            root,
        )],

        Language::C | Language::Cpp => vec![
            run(
                "Configurar build (cmake)",
                "Gera os arquivos de build na pasta build/.",
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
                "Compilar (cmake --build)",
                "Compila o projeto.",
                "cmake",
                vec!["--build".to_string(), "build".to_string()],
                root,
            ),
        ],

        Language::Unknown => Vec::new(),
    }
}

fn python_steps(stack: &ProjectStack, manifest: &str, root: &Path) -> Vec<ProvisionStep> {
    match stack.package_manager.as_deref() {
        Some("poetry") => vec![run(
            "Instalar dependências (poetry install)",
            "Cria o ambiente virtual e instala as dependências.",
            "poetry",
            vec!["install".to_string()],
            root,
        )],
        Some("pipenv") => vec![run(
            "Instalar dependências (pipenv install)",
            "Cria o ambiente virtual e instala as dependências.",
            "pipenv",
            vec!["install".to_string()],
            root,
        )],
        _ => {
            // Virtual environment + installation.
            let mut steps = vec![run(
                "Criar ambiente virtual (python venv)",
                "Cria a pasta .venv isolando as dependências do sistema.",
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
                    "Instalar dependências (pip install)",
                    "Instala as dependências do projeto no ambiente virtual.",
                    pip,
                    args,
                    root,
                ));
            }
            steps
        }
    }
}

fn jvm_steps(stack: &ProjectStack, manifest: &str, root: &Path) -> Vec<ProvisionStep> {
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
                "Compilar (gradle build)",
                "Baixa as dependências e compila o projeto.",
                program,
                vec!["build".to_string()],
                root,
            )]
        }
        _ => vec![run(
            "Baixar dependências (maven)",
            "Resolve as dependências e compila sem rodar testes.",
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
        Language::Rust => "cargo run".to_string(),
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
