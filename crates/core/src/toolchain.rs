//! Toolchain check: are the tools the project needs on the PATH?
//!
//! **Phase 1** (conservative): it only **detects** what is missing and
//! **suggests** the install command for the current OS family. It installs
//! nothing (pure read). The idea is that the user doesn't discover "node not
//! found" only when `npm install` fails.
//!
//! Auto-install (with sudo/package manager) is Phase 2 — much more
//! unpredictable per distro/OS — and is deliberately left out here.
//!
//! Honest limitation: the suggestion is a reasonable guess (apt/brew/winget),
//! not a guarantee. Each machine may differ.

use devforge_sdk::{CommandRunner, CommandSpec};
use devforge_shared::{Error, Language, ProjectStack};
use std::collections::HashSet;
use std::path::Path;

/// Missing tool, with the install suggestion for its system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingTool {
    pub tool: String,
    pub language: Language,
    pub hint: String,
}

/// Checks whether the tools required by the stacks are on the PATH.
///
/// `docker` is not checked here: in Docker mode the toolchain lives inside the
/// container.
pub fn missing_tools(
    runner: &dyn CommandRunner,
    stacks: &[ProjectStack],
    root: &Path,
) -> Result<Vec<MissingTool>, Error> {
    let mut missing = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for stack in stacks {
        for tool in required_tools(stack) {
            if !seen.insert(tool.clone()) {
                continue;
            }
            if !on_path(runner, root, &tool) {
                missing.push(MissingTool {
                    language: stack.language,
                    hint: install_hint(&tool),
                    tool,
                });
            }
        }
    }
    Ok(missing)
}

/// Queries the PATH using `which` (unix) or `where` (windows) — no shell.
fn on_path(runner: &dyn CommandRunner, root: &Path, tool: &str) -> bool {
    let lookup = if cfg!(windows) { "where" } else { "which" };
    let spec = CommandSpec::new(lookup, [tool], root);
    matches!(runner.run(&spec), Ok(output) if output.success)
}

/// Tools a stack needs to run locally.
fn required_tools(stack: &ProjectStack) -> Vec<String> {
    let pm = stack.package_manager.as_deref().unwrap_or("");
    match stack.language {
        Language::Rust => vec!["cargo".into()],
        Language::JavaScript | Language::TypeScript => {
            let pm_tool = match pm {
                "yarn" => "yarn",
                "pnpm" => "pnpm",
                "bun" => "bun",
                _ => "npm",
            };
            vec!["node".into(), pm_tool.into()]
        }
        Language::Python => vec!["python3".into()],
        Language::Go => vec!["go".into()],
        Language::Java | Language::Kotlin => vec!["java".into()],
        Language::Php => vec!["php".into(), "composer".into()],
        Language::Ruby => vec!["ruby".into()],
        Language::CSharp => vec!["dotnet".into()],
        Language::Dart => {
            if stack.frameworks.iter().any(|f| f == "Flutter") {
                vec!["flutter".into()]
            } else {
                vec!["dart".into()]
            }
        }
        Language::Elixir => vec!["elixir".into()],
        Language::Swift => vec!["swift".into()],
        Language::C | Language::Cpp => vec!["cc".into()],
        Language::Unknown => vec![],
    }
}

// --- per-OS suggestion ------------------------------------------------------

fn install_hint(tool: &str) -> String {
    match std::env::consts::OS {
        "macos" => format!("brew install {}", brew_formula(tool)),
        "windows" => format!("winget install {}", winget_id(tool)),
        _ => match linux_install_cmd() {
            Some(cmd) => format!("{cmd} {}", linux_pkg(tool)),
            None => format!("instale '{}' (pacote sugerido: {})", tool, linux_pkg(tool)),
        },
    }
}

fn brew_formula(tool: &str) -> &'static str {
    match tool {
        "node" | "npm" => "node",
        "cargo" => "rust",
        "python3" => "python",
        "java" => "temurin",
        "dotnet" => "--cask dotnet",
        "cc" => "gcc",
        _ => match tool {
            "yarn" => "yarn",
            "pnpm" => "pnpm",
            "bun" => "bun",
            "go" => "go",
            "php" => "php",
            "composer" => "composer",
            "ruby" => "ruby",
            "dart" => "dart",
            "flutter" => "flutter",
            "elixir" => "elixir",
            "swift" => "swift",
            "mvn" => "maven",
            "gradle" => "gradle",
            "cmake" => "cmake",
            _ => "node",
        },
    }
}

fn winget_id(tool: &str) -> &'static str {
    match tool {
        "node" | "npm" => "OpenJS.NodeJS",
        "cargo" => "Rustlang.Rustup",
        "python3" => "Python.Python.3.12",
        "java" => "EclipseAdoptium.Temurin.21.JDK",
        "go" => "GoLang.Go",
        "dotnet" => "Microsoft.DotNet.SDK.8",
        _ => "OpenJS.NodeJS", // best-effort; the warning already says it's a suggestion
    }
}

/// Detects the Linux package manager by the presence of its binary.
fn linux_install_cmd() -> Option<&'static str> {
    for (path, cmd) in [
        ("/usr/bin/apt-get", "sudo apt install"),
        ("/bin/apt-get", "sudo apt install"),
        ("/usr/bin/dnf", "sudo dnf install"),
        ("/usr/bin/pacman", "sudo pacman -S"),
        ("/usr/bin/zypper", "sudo zypper install"),
    ] {
        if Path::new(path).exists() {
            return Some(cmd);
        }
    }
    None
}

fn linux_pkg(tool: &str) -> &'static str {
    match tool {
        "node" | "npm" => "nodejs",
        "python3" => "python3",
        "java" => "default-jdk",
        "go" => "golang-go",
        "dotnet" => "dotnet-sdk-8.0",
        "cc" => "build-essential",
        _ => match tool {
            "cargo" => "cargo",
            "php" => "php",
            "composer" => "composer",
            "ruby" => "ruby",
            "dart" => "dart",
            "flutter" => "flutter",
            "elixir" => "elixir",
            "yarn" => "yarn",
            "pnpm" => "pnpm",
            "bun" => "bun",
            _ => "nodejs",
        },
    }
}

// --- auto-install (Phase 2) -------------------------------------------------

/// Tools for which we offer **automatic** (curated) installation. The others
/// only get a suggestion — avoids guessing the wrong command/version (e.g.:
/// Rust goes through rustup, a `curl | sh`; dotnet/java have too many
/// variants).
const AUTO_INSTALLABLE: &[&str] = &["node", "npm", "go", "python3", "java", "php", "ruby"];

/// Builds the command (program + args, **no shell**) to install `tool` on the
/// current OS. `None` if the tool is not curated or the package manager does
/// not exist.
pub fn install_command(tool: &str) -> Option<(String, Vec<String>)> {
    if !AUTO_INSTALLABLE.contains(&tool) {
        return None;
    }
    match std::env::consts::OS {
        "macos" => bin_on_path("brew").then(|| {
            (
                "brew".to_string(),
                vec!["install".to_string(), brew_formula(tool).to_string()],
            )
        }),
        "windows" => bin_on_path("winget").then(|| {
            (
                "winget".to_string(),
                vec![
                    "install".to_string(),
                    winget_id(tool).to_string(),
                    "--accept-package-agreements".to_string(),
                    "--accept-source-agreements".to_string(),
                ],
            )
        }),
        _ => {
            let (program, mut args) = linux_pm_argv()?;
            args.push(linux_pkg(tool).to_string());
            Some((program, args))
        }
    }
}

/// Program on the PATH? (scans the `PATH` dirs; without invoking a shell.)
fn bin_on_path(bin: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    for dir in std::env::split_paths(&paths) {
        if dir.join(bin).is_file() {
            return true;
        }
        if cfg!(windows) {
            for ext in ["exe", "cmd", "bat"] {
                if dir.join(format!("{bin}.{ext}")).is_file() {
                    return true;
                }
            }
        }
    }
    false
}

/// Linux package manager -> (program, initial args).
fn linux_pm_argv() -> Option<(String, Vec<String>)> {
    for (bin, program, args) in [
        ("/usr/bin/apt-get", "sudo", vec!["apt", "install", "-y"]),
        ("/bin/apt-get", "sudo", vec!["apt", "install", "-y"]),
        ("/usr/bin/dnf", "sudo", vec!["dnf", "install", "-y"]),
        ("/usr/bin/pacman", "sudo", vec!["pacman", "-S", "--noconfirm"]),
        ("/usr/bin/zypper", "sudo", vec!["zypper", "install", "-y"]),
    ] {
        if Path::new(bin).exists() {
            return Some((
                program.to_string(),
                args.into_iter().map(String::from).collect(),
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use devforge_sdk::{Capability, CommandOutput, CommandSpec};
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// Fake runner: finds only the tools in `present` to be present.
    struct FakeRunner {
        present: Vec<String>,
        asked: Mutex<Vec<String>>,
    }

    impl Capability for FakeRunner {
        fn name(&self) -> &'static str {
            "fake_runner"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_command_runner(&self) -> Option<&dyn CommandRunner> {
            Some(self)
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, command: &CommandSpec) -> Result<CommandOutput, Error> {
            let tool = command.args.first().cloned().unwrap_or_default();
            self.asked.lock().unwrap().push(tool.clone());
            let success = self.present.contains(&tool);
            Ok(CommandOutput {
                success,
                code: Some(if success { 0 } else { 1 }),
                stdout: String::new(),
                stderr: String::new(),
            })
        }
    }

    fn node_stack() -> ProjectStack {
        ProjectStack::new(
            Language::JavaScript,
            PathBuf::from("/p/package.json"),
            Some("npm".into()),
        )
    }

    #[test]
    fn reports_missing_tools() {
        // `node` present, `npm` missing.
        let runner = FakeRunner {
            present: vec!["node".into()],
            asked: Mutex::new(Vec::new()),
        };
        let missing = missing_tools(&runner, &[node_stack()], Path::new("/p")).unwrap();
        let tools: Vec<_> = missing.iter().map(|m| m.tool.as_str()).collect();
        assert!(tools.contains(&"npm"), "npm devia faltar: {tools:?}");
        assert!(!tools.contains(&"node"), "node tava presente");
        assert!(!missing[0].hint.is_empty());
    }

    #[test]
    fn nothing_missing_when_everything_present() {
        let runner = FakeRunner {
            present: vec!["node".into(), "npm".into()],
            asked: Mutex::new(Vec::new()),
        };
        let missing = missing_tools(&runner, &[node_stack()], Path::new("/p")).unwrap();
        assert!(missing.is_empty());
    }
}
