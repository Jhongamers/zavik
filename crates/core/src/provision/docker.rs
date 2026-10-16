//! Preparation recipes **with Docker**: `Dockerfile` generation and image
//! build.
//!
//! The generated `Dockerfile`s are a **starting point**: each one carries a
//! comment warning that the `CMD`/variables may need adjustment.

use super::{ProvisionAction, ProvisionStep};
use devforge_shared::{Language, ProjectStack};
use devforge_sdk::CommandSpec;
use std::path::Path;

/// Steps for Docker mode: check docker, generate Dockerfile(s) and build.
pub(crate) fn steps_for(stacks: &[ProjectStack], root: &Path) -> Vec<ProvisionStep> {
    let mut steps = Vec::new();

    // 0. Check whether Docker is available (no permission required).
    steps.push(ProvisionStep {
        title: "Verificar Docker".to_string(),
        detail: "docker --version".to_string(),
        reason: "Confere se o Docker está instalado antes de gerar/buildar imagens."
            .to_string(),
        requires_permission: false,
        action: ProvisionAction::RunCommand(CommandSpec::new(
            "docker",
            ["--version"],
            root,
        )),
    });

    if stacks.is_empty() {
        return steps;
    }

    let project = project_name(stacks, root);
    let single = stacks.len() == 1;

    // 1. One Dockerfile per stack (just `Dockerfile` if there is only one),
    //    already noting which image will be built from it.
    let mut build_targets: Vec<(String, String)> = Vec::new();
    for stack in stacks {
        let slug = language_slug(stack.language);
        let file_name = if single {
            "Dockerfile".to_string()
        } else {
            format!("Dockerfile.{slug}")
        };
        let image = if single {
            project.clone()
        } else {
            format!("{project}:{slug}")
        };
        let path = root.join(&file_name);

        steps.push(ProvisionStep {
            title: format!("Gerar {file_name}"),
            detail: path.display().to_string(),
            reason: format!(
                "Cria o Dockerfile para o stack {} usando a imagem base oficial.",
                stack.language
            ),
            requires_permission: true,
            action: ProvisionAction::WriteFile {
                path,
                contents: dockerfile_for(stack, &project),
            },
        });

        build_targets.push((file_name, image));
    }

    // 2. Build of each image.
    for (dockerfile, image) in build_targets {
        let args = vec![
            "build".to_string(),
            "-t".to_string(),
            image.clone(),
            "-f".to_string(),
            dockerfile,
            ".".to_string(),
        ];
        let spec = CommandSpec::new("docker", args, root);
        steps.push(ProvisionStep {
            title: format!("Construir imagem {image}"),
            detail: spec.display(),
            reason: "Executa o `docker build` (pode baixar imagens e demorar).".to_string(),
            requires_permission: true,
            action: ProvisionAction::RunCommand(spec),
        });
    }

    steps
}

/// Hints for running in a container.
pub(crate) fn run_hints_for(stacks: &[ProjectStack], root: &Path) -> Vec<String> {
    let project = project_name(stacks, root);
    image_names(stacks, &project)
        .into_iter()
        .map(|image| format!("docker run --rm -it {image}"))
        .collect()
}

/// Project name (sanitized to become an image tag).
fn project_name(stacks: &[ProjectStack], root: &Path) -> String {
    let raw = stacks
        .iter()
        .find_map(|s| s.name.clone())
        .or_else(|| {
            root.file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "devforge-app".to_string());
    sanitize(&raw)
}

/// List of images (one per stack), in the same order as the stacks.
fn image_names(stacks: &[ProjectStack], project: &str) -> Vec<String> {
    if stacks.len() == 1 {
        vec![project.to_string()]
    } else {
        stacks
            .iter()
            .map(|s| format!("{project}:{}", language_slug(s.language)))
            .collect()
    }
}

/// Short, safe slug for the language (used in file names and tags).
fn language_slug(language: Language) -> &'static str {
    match language {
        Language::Rust => "rust",
        Language::JavaScript => "js",
        Language::TypeScript => "ts",
        Language::Python => "python",
        Language::Go => "go",
        Language::Java => "java",
        Language::Kotlin => "kotlin",
        Language::Php => "php",
        Language::Ruby => "ruby",
        Language::CSharp => "csharp",
        Language::Dart => "dart",
        Language::Swift => "swift",
        Language::C => "c",
        Language::Cpp => "cpp",
        Language::Elixir => "elixir",
        Language::Unknown => "app",
    }
}

/// Sanitizes text to be valid as a Docker tag (`[a-z0-9._-]`).
fn sanitize(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    let out = out.trim_matches(|c| c == '-' || c == '_' || c == '.').to_string();
    if out.is_empty() {
        "devforge-app".to_string()
    } else {
        out
    }
}

/// Generates the Dockerfile content for a stack.
fn dockerfile_for(stack: &ProjectStack, project: &str) -> String {
    let pm = stack.package_manager.as_deref().unwrap_or("npm");
    match stack.language {
        Language::Rust => "# Gerado pelo DevForge. Ajuste o CMD/entrypoint conforme o seu binário.\n\
             FROM rust:1-bookworm\n\
             WORKDIR /app\n\
             COPY . .\n\
             RUN cargo build --release\n\
             CMD [\"cargo\", \"run\", \"--release\"]\n"
            .to_string(),
        Language::JavaScript | Language::TypeScript => format!(
            "# Gerado pelo DevForge. Ajuste o CMD conforme o seu package.json.\n\
             FROM node:20-alpine\n\
             WORKDIR /app\n\
             COPY package*.json ./\n\
             RUN {pm} install\n\
             COPY . .\n\
             CMD [\"{pm}\", \"start\"]\n"
        ),
        Language::Python => {
            let install = if stack.manifest.ends_with("requirements.txt") {
                "pip install --no-cache-dir -r requirements.txt"
            } else {
                "pip install --no-cache-dir ."
            };
            format!(
                "# Gerado pelo DevForge. Ajuste o CMD para o seu entrypoint.\n\
                 FROM python:3.12-slim\n\
                 WORKDIR /app\n\
                 COPY . .\n\
                 RUN {install}\n\
                 CMD [\"python\", \"main.py\"]\n"
            )
        }
        Language::Go => "# Gerado pelo DevForge (build multi-stage).\n\
             FROM golang:1.22-alpine AS builder\n\
             WORKDIR /app\n\
             COPY . .\n\
             RUN go build -o /app/app .\n\
             \n\
             FROM alpine:3.19\n\
             COPY --from=builder /app/app /usr/local/bin/app\n\
             CMD [\"app\"]\n"
            .to_string(),
        Language::Java | Language::Kotlin => {
            if pm == "gradle" {
                "# Gerado pelo DevForge. Ajuste o CMD conforme seu build Gradle.\n\
                 FROM eclipse-temurin:21-jdk AS builder\n\
                 WORKDIR /app\n\
                 COPY . .\n\
                 RUN ./gradlew build -x test || gradle build -x test\n\
                 \n\
                 FROM eclipse-temurin:21-jre\n\
                 WORKDIR /app\n\
                 COPY --from=builder /app/build/libs/*.jar app.jar\n\
                 CMD [\"java\", \"-jar\", \"app.jar\"]\n"
                    .to_string()
            } else {
                "# Gerado pelo DevForge. Ajuste o CMD conforme seu build Maven.\n\
                 FROM maven:3.9-eclipse-temurin-21 AS builder\n\
                 WORKDIR /app\n\
                 COPY . .\n\
                 RUN mvn -q -DskipTests package\n\
                 \n\
                 FROM eclipse-temurin:21-jre\n\
                 WORKDIR /app\n\
                 COPY --from=builder /app/target/*.jar app.jar\n\
                 CMD [\"java\", \"-jar\", \"app.jar\"]\n"
                    .to_string()
            }
        }
        Language::Php => "# Gerado pelo DevForge. Ajuste o CMD/docroot conforme o projeto.\n\
             FROM php:8.3-cli\n\
             WORKDIR /app\n\
             RUN apt-get update && apt-get install -y unzip git \
                 && curl -sS https://getcomposer.org/installer | php -- \
                    --install-dir=/usr/local/bin --filename=composer\n\
             COPY . .\n\
             RUN composer install --no-interaction\n\
             CMD [\"php\", \"-S\", \"0.0.0.0:8000\", \"-t\", \"public\"]\n"
            .to_string(),
        Language::Ruby => "# Gerado pelo DevForge. Ajuste o CMD conforme o framework.\n\
             FROM ruby:3.3-slim\n\
             WORKDIR /app\n\
             COPY . .\n\
             RUN bundle install\n\
             CMD [\"bundle\", \"exec\", \"rails\", \"server\", \"-b\", \"0.0.0.0\"]\n"
            .to_string(),
        Language::CSharp => format!(
            "# Gerado pelo DevForge. Ajuste o nome do .dll no CMD.\n\
             FROM mcr.microsoft.com/dotnet/sdk:8.0 AS builder\n\
             WORKDIR /app\n\
             COPY . .\n\
             RUN dotnet publish -c Release -o /out\n\
             \n\
             FROM mcr.microsoft.com/dotnet/aspnet:8.0\n\
             WORKDIR /app\n\
             COPY --from=builder /out .\n\
             CMD [\"dotnet\", \"{project}.dll\"]\n"
        ),
        Language::Dart => {
            if stack.frameworks.iter().any(|f| f == "Flutter") {
                "# Gerado pelo DevForge (Flutter).\n\
                 FROM ghcr.io/cirruslabs/flutter:stable\n\
                 WORKDIR /app\n\
                 COPY . .\n\
                 RUN flutter pub get\n\
                 CMD [\"flutter\", \"run\"]\n"
                    .to_string()
            } else {
                "# Gerado pelo DevForge (Dart).\n\
                 FROM dart:stable\n\
                 WORKDIR /app\n\
                 COPY . .\n\
                 RUN dart pub get\n\
                 CMD [\"dart\", \"run\"]\n"
                    .to_string()
            }
        }
        Language::Elixir => "# Gerado pelo DevForge. Ajuste o CMD conforme o projeto.\n\
             FROM hexpm/elixir:1.16-erlang-26-debian-bookworm-slim\n\
             WORKDIR /app\n\
             COPY . .\n\
             RUN mix local.hex --force && mix local.rebar --force \
                 && mix deps.get && mix compile\n\
             CMD [\"mix\", \"phx.server\"]\n"
            .to_string(),
        Language::Swift => "# Gerado pelo DevForge.\n\
             FROM swift:5.9\n\
             WORKDIR /app\n\
             COPY . .\n\
             RUN swift build\n\
             CMD [\"swift\", \"run\"]\n"
            .to_string(),
        Language::C | Language::Cpp => "# Gerado pelo DevForge. Ajuste o CMD conforme o binário.\n\
             FROM gcc:13\n\
             WORKDIR /app\n\
             RUN apt-get update && apt-get install -y cmake\n\
             COPY . .\n\
             RUN cmake -S . -B build && cmake --build build\n\
             CMD [\"./build/app\"]\n"
            .to_string(),
        Language::Unknown => "# Gerado pelo DevForge.\nFROM alpine:3.19\n".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use devforge_shared::{Language, ProjectStack};
    use std::path::PathBuf;

    fn stack(language: Language, name: &str) -> ProjectStack {
        ProjectStack::new(language, PathBuf::from(format!("/p/{name}")), None)
            .with_name(Some(name.to_string()))
    }

    #[test]
    fn generates_dockerfile_for_rust() {
        let content = dockerfile_for(&stack(Language::Rust, "demo"), "demo");
        assert!(content.contains("FROM rust:"));
        assert!(content.contains("cargo build --release"));
    }

    #[test]
    fn sanitizes_image_name() {
        assert_eq!(sanitize("Meu App/Web!"), "meu-app-web");
        assert_eq!(sanitize("___"), "devforge-app");
    }

    #[test]
    fn single_dockerfile_when_single_stack() {
        let steps = steps_for(&[stack(Language::Rust, "demo")], Path::new("/p"));
        // 1 check + 1 Dockerfile + 1 build.
        assert_eq!(steps.len(), 3);
        assert!(steps.iter().any(|s| s.detail.ends_with("/p/Dockerfile")));
    }
}
