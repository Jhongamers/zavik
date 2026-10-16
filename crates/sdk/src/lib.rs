//! Crate `devforge-sdk` — the **contracts** for capabilities.
//!
//! This crate is what a third-party developer imports to create a plugin
//! (ADR 0003). It depends only on `devforge-shared`/`devforge-events`, and
//! never on the Kernel, the database, or Tauri.
//!
//! Besides capabilities, the SDK defines presentation ports (e.g.
//! [`PermissionGate`]) that `core` uses without knowing the concrete UI.

use devforge_shared::{
    AnalysisReport, Context, Error, PluginId, ProjectStack, TrustReport,
};
use std::any::Any;
use std::path::{Path, PathBuf};

/// Base contract for **any** plugin.
///
/// For the Kernel to perform the "safe cast" described in `arquiteture.md`,
/// each plugin exposes here the capabilities it implements via `as_*` methods.
pub trait Capability: Any {
    /// Unique name used to register the plugin in the Kernel.
    fn name(&self) -> &'static str;

    /// Access to the concrete type (for downcasting/diagnostics).
    fn as_any(&self) -> &dyn Any;

    /// Returns `Some(self)` if this plugin implements `FileSystemAccess`.
    fn as_file_system(&self) -> Option<&dyn FileSystemAccess> {
        None
    }

    /// Returns `Some(self)` if this plugin implements `ProjectDetector`.
    fn as_detector(&self) -> Option<&dyn ProjectDetector> {
        None
    }

    /// Returns `Some(self)` if this plugin implements `CommandRunner`.
    fn as_command_runner(&self) -> Option<&dyn CommandRunner> {
        None
    }

    /// Returns `Some(self)` if this plugin implements `ProjectAnalyzer`.
    fn as_analyzer(&self) -> Option<&dyn ProjectAnalyzer> {
        None
    }
}

/// Context handed to the plugin in `on_load`.
pub struct PluginContext {
    pub id: PluginId,
    pub context: Context,
}

// ---------------------------------------------------------------------------
// Capability: file system
// ---------------------------------------------------------------------------

/// File system access capability.
pub trait FileSystemAccess: Capability {
    /// Checks whether a path exists and is a file.
    fn check_file_exists(&self, path: &Path) -> Result<bool, Error>;

    /// Reads a file as text.
    fn read_to_string(&self, path: &Path) -> Result<String, Error>;

    /// Writes text to a file (creates/overwrites).
    fn write_string(&self, path: &Path, contents: &str) -> Result<(), Error>;
}

// ---------------------------------------------------------------------------
// Capability: stack detection
// ---------------------------------------------------------------------------

/// Project stack detection capability (language, framework and
/// dependencies). Implemented by specialized plugins.
pub trait ProjectDetector: Capability {
    /// Analyzes `context.project_root` and returns **all** recognized stacks
    /// (a project may have more than one, e.g. Tauri = Rust + Node).
    fn detect(&self, context: &Context) -> Result<Vec<ProjectStack>, Error>;
}

// ---------------------------------------------------------------------------
// Capability: command execution
// ---------------------------------------------------------------------------

/// A command to be executed, already "assembled".
#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    /// Working directory of the command.
    pub working_dir: PathBuf,
}

impl CommandSpec {
    pub fn new(
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
        working_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            working_dir: working_dir.into(),
        }
    }

    /// Human-readable representation (e.g. `cargo build --release`).
    pub fn display(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }
}

/// Result of executing a command.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Capability that runs commands on the system.
pub trait CommandRunner: Capability {
    fn run(&self, command: &CommandSpec) -> Result<CommandOutput, Error>;

    /// Runs **interactively** (inherited stdio), for commands that prompt for a
    /// password on the spot (e.g. `sudo`, `brew`). Returns the exit code.
    /// The default is "not supported" — only those with a real terminal implement it.
    fn run_interactive(&self, command: &CommandSpec) -> Result<Option<i32>, Error> {
        let _ = command;
        Err(Error::IoError(
            "execução interativa não suportada por este runner".to_string(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Capability: dependency analysis
// ---------------------------------------------------------------------------

/// Analysis options.
#[derive(Debug, Clone, Copy, Default)]
pub struct AnalysisOptions {
    /// Query registries (crates.io, npm, PyPI) to find new versions.
    /// Requires the network.
    pub check_latest: bool,
}

/// Capability that analyzes dependencies for risks and updates.
pub trait ProjectAnalyzer: Capability {
    fn analyze(
        &self,
        context: &Context,
        stacks: &[ProjectStack],
        options: &AnalysisOptions,
    ) -> Result<AnalysisReport, Error>;

    /// Trust check **before** installing: finds things that can
    /// execute arbitrary code (e.g. `postinstall`) or that do not match the
    /// registry (possible typo/fraud). It is the "search" that stops DevForge from
    /// trusting the manifest blindly — it protects when an AI drives the tool.
    fn verify_trust(
        &self,
        context: &Context,
        stacks: &[ProjectStack],
    ) -> Result<TrustReport, Error>;
}

// ---------------------------------------------------------------------------
// Presentation port: permissions
// ---------------------------------------------------------------------------

/// Permission request shown to the user before a sensitive action.
#[derive(Debug, Clone)]
pub struct PermissionRequest {
    /// Short title of the action (e.g. "Install dependencies").
    pub title: String,
    /// What will actually be done (e.g. `npm install` or the file path).
    pub detail: String,
    /// Why we are asking for permission.
    pub reason: String,
    /// If `true`, this is a code-execution / supply-chain risk. The implementation
    /// **must not** cover it with "yes to all" nor with `--yes`: it requires an
    /// explicit, individual confirmation.
    pub critical: bool,
}

/// The user's decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Approved,
    Denied,
}

/// Port that `core` uses to request permission. The concrete implementation
/// (terminal prompt, UI dialog, etc.) lives in the presentation layer.
pub trait PermissionGate: Send + Sync {
    fn confirm(&self, request: &PermissionRequest) -> Decision;
}
