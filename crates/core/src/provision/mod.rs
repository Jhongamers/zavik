//! Use case **Prepare project to run** (with and without Docker).
//!
//! The flow is:
//! 1. [`Provisioner::build_plan`] builds a [`PreparePlan`] from the detected
//!    stacks (pure, without touching the environment).
//! 2. The presentation shows the plan.
//! 3. [`Provisioner::execute`] runs it step by step, **asking for permission**
//!    before each sensitive action via [`PermissionGate`].
//!
//! The `core` executes nothing directly: it uses the capabilities
//! `CommandRunner`/`FileSystemAccess` and the `PermissionGate` port.

use devforge_shared::{Error, ProjectStack};
use devforge_sdk::{
    CommandRunner, CommandSpec, Decision, FileSystemAccess, PermissionGate, PermissionRequest,
};
use std::path::{Path, PathBuf};

mod docker;
mod local;

/// Preparation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrepareMode {
    /// Installs the dependencies on the current machine.
    Local,
    /// Generates the `Dockerfile` and builds the image.
    Docker,
}

/// Concrete action of a step.
#[derive(Debug, Clone)]
pub enum ProvisionAction {
    RunCommand(CommandSpec),
    WriteFile { path: PathBuf, contents: String },
}

/// A step of the preparation plan.
#[derive(Debug, Clone)]
pub struct ProvisionStep {
    /// Short title (e.g. "Instalar dependências (npm)").
    pub title: String,
    /// What will be done (command or file path).
    pub detail: String,
    /// Justification shown when asking for permission.
    pub reason: String,
    /// Whether it requires user confirmation before running.
    pub requires_permission: bool,
    pub action: ProvisionAction,
}

/// Complete plan, displayable before any execution.
#[derive(Debug, Default)]
pub struct PreparePlan {
    pub steps: Vec<ProvisionStep>,
    /// How to run the project once prepared.
    pub run_hints: Vec<String>,
}

/// Execution options.
#[derive(Debug, Default, Clone, Copy)]
pub struct ExecuteOptions {
    /// If `true`, runs nothing: only returns what *would* be done.
    pub dry_run: bool,
    /// Allows overwriting existing files (e.g. the `Dockerfile`).
    pub force: bool,
}

/// Result of the execution.
#[derive(Debug, Default)]
pub struct PrepareReport {
    /// Steps that would be run (`dry_run` mode).
    pub planned: Vec<String>,
    /// Steps successfully completed.
    pub executed: Vec<String>,
    /// Steps refused or skipped (e.g. file already exists).
    pub skipped: Vec<String>,
    /// Filled in if a step failed (execution stops at the first error).
    pub failed: Option<String>,
}

/// Project preparation engine.
pub struct Provisioner;

impl Provisioner {
    /// Builds the plan. Does not touch the user's environment.
    pub fn build_plan(mode: PrepareMode, stacks: &[ProjectStack], root: &Path) -> PreparePlan {
        let mut plan = PreparePlan::default();
        match mode {
            PrepareMode::Local => {
                for stack in stacks {
                    plan.steps.extend(local::steps_for(stack, root));
                    plan.run_hints.extend(local::run_hints_for(stack));
                }
            }
            PrepareMode::Docker => {
                plan.steps.extend(docker::steps_for(stacks, root));
                plan.run_hints.extend(docker::run_hints_for(stacks, root));
            }
        }
        plan
    }

    /// Runs the plan, asking for permission before each sensitive action.
    pub fn execute(
        &self,
        plan: &PreparePlan,
        runner: &dyn CommandRunner,
        files: &dyn FileSystemAccess,
        gate: &dyn PermissionGate,
        options: &ExecuteOptions,
    ) -> Result<PrepareReport, Error> {
        let mut report = PrepareReport::default();

        for step in &plan.steps {
            if options.dry_run {
                report.planned.push(step.title.clone());
                continue;
            }

            // 1. Permission.
            if step.requires_permission
                && gate.confirm(&PermissionRequest {
                    title: step.title.clone(),
                    detail: step.detail.clone(),
                    reason: step.reason.clone(),
                    critical: false,
                }) == Decision::Denied
            {
                println!("   ↳ recusado pelo usuário, pulando.");
                report.skipped.push(step.title.clone());
                continue;
            }

            // 2. Execution.
            match &step.action {
                ProvisionAction::RunCommand(command) => {
                    println!("   $ {}", command.display());
                    match runner.run(command) {
                        Ok(output) if output.success => {
                            report.executed.push(step.title.clone());
                        }
                        Ok(output) => {
                            if !output.stderr.trim().is_empty() {
                                eprintln!("   stderr: {}", output.stderr.trim());
                            }
                            let reason = format!(
                                "'{}' falhou (código {:?})",
                                command.display(),
                                output.code
                            );
                            report.failed = Some(reason);
                            return Ok(report);
                        }
                        Err(error) => {
                            report.failed = Some(error.to_string());
                            return Ok(report);
                        }
                    }
                }
                ProvisionAction::WriteFile { path, contents } => {
                    // Never overwrites an existing file without `--force`.
                    if !options.force && files.check_file_exists(path)? {
                        println!("   ↳ arquivo já existe, pulando: {}", path.display());
                        report
                            .skipped
                            .push(format!("{} (já existe)", step.title));
                        continue;
                    }
                    files.write_string(path, contents)?;
                    report.executed.push(format!("{} -> {}", step.title, path.display()));
                }
            }
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use devforge_shared::{Dependency, DependencyKind, Language};
    use devforge_sdk::{
        Capability, CommandOutput, CommandRunner, CommandSpec, Decision, FileSystemAccess,
        PermissionGate, PermissionRequest,
    };
    use std::sync::Mutex;

    // --- test doubles -------------------------------------------------------

    #[derive(Default)]
    struct FakeRunner {
        ran: Mutex<Vec<String>>,
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
            self.ran.lock().unwrap().push(command.display());
            Ok(CommandOutput {
                success: true,
                code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })
        }
    }

    #[derive(Default)]
    struct FakeFs {
        existing: Vec<PathBuf>,
        written: Mutex<Vec<PathBuf>>,
    }

    impl Capability for FakeFs {
        fn name(&self) -> &'static str {
            "fake_fs"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_file_system(&self) -> Option<&dyn FileSystemAccess> {
            Some(self)
        }
    }

    impl FileSystemAccess for FakeFs {
        fn check_file_exists(&self, path: &Path) -> Result<bool, Error> {
            Ok(self.existing.contains(&path.to_path_buf()))
        }
        fn read_to_string(&self, _path: &Path) -> Result<String, Error> {
            Ok(String::new())
        }
        fn write_string(&self, path: &Path, _contents: &str) -> Result<(), Error> {
            self.written.lock().unwrap().push(path.to_path_buf());
            Ok(())
        }
    }

    struct Allow;
    impl PermissionGate for Allow {
        fn confirm(&self, _request: &PermissionRequest) -> Decision {
            Decision::Approved
        }
    }

    struct Deny;
    impl PermissionGate for Deny {
        fn confirm(&self, _request: &PermissionRequest) -> Decision {
            Decision::Denied
        }
    }

    fn rust_stack() -> ProjectStack {
        ProjectStack::new(Language::Rust, PathBuf::from("/p/Cargo.toml"), Some("cargo".into()))
            .with_name(Some("demo".into()))
            .with_dependencies(vec![Dependency::new("serde", None, DependencyKind::Runtime)])
    }

    // --- tests --------------------------------------------------------------

    #[test]
    fn rust_local_plan_includes_cargo() {
        let plan = Provisioner::build_plan(PrepareMode::Local, &[rust_stack()], Path::new("/p"));
        assert!(plan
            .steps
            .iter()
            .any(|s| s.detail.contains("cargo build")));
        assert!(plan.run_hints.iter().any(|h| h.contains("cargo run")));
    }

    #[test]
    fn executes_when_approved() {
        let plan = Provisioner::build_plan(PrepareMode::Local, &[rust_stack()], Path::new("/p"));
        let runner = FakeRunner::default();
        let fs = FakeFs::default();
        let report = Provisioner::execute(
            &Provisioner,
            &plan,
            &runner,
            &fs,
            &Allow,
            &ExecuteOptions::default(),
        )
        .unwrap();

        assert!(report.failed.is_none());
        assert!(!report.executed.is_empty());
        assert!(runner.ran.lock().unwrap().iter().any(|c| c.contains("cargo")));
    }

    #[test]
    fn skips_when_denied() {
        let plan = Provisioner::build_plan(PrepareMode::Local, &[rust_stack()], Path::new("/p"));
        let runner = FakeRunner::default();
        let fs = FakeFs::default();
        let report = Provisioner::execute(
            &Provisioner,
            &plan,
            &runner,
            &fs,
            &Deny,
            &ExecuteOptions::default(),
        )
        .unwrap();

        assert!(report.executed.is_empty());
        assert_eq!(report.skipped.len(), plan.steps.len());
        assert!(runner.ran.lock().unwrap().is_empty());
    }

    #[test]
    fn dry_run_executes_nothing() {
        let plan = Provisioner::build_plan(PrepareMode::Local, &[rust_stack()], Path::new("/p"));
        let runner = FakeRunner::default();
        let fs = FakeFs::default();
        let report = Provisioner::execute(
            &Provisioner,
            &plan,
            &runner,
            &fs,
            &Allow,
            &ExecuteOptions {
                dry_run: true,
                force: false,
            },
        )
        .unwrap();

        assert_eq!(report.planned.len(), plan.steps.len());
        assert!(report.executed.is_empty());
        assert!(runner.ran.lock().unwrap().is_empty());
    }

    #[test]
    fn docker_does_not_overwrite_existing_dockerfile() {
        let plan = Provisioner::build_plan(PrepareMode::Docker, &[rust_stack()], Path::new("/p"));
        let runner = FakeRunner::default();
        let mut fs = FakeFs::default();
        fs.existing.push(PathBuf::from("/p/Dockerfile"));

        let report = Provisioner::execute(
            &Provisioner,
            &plan,
            &runner,
            &fs,
            &Allow,
            &ExecuteOptions::default(),
        )
        .unwrap();

        // The existing Dockerfile was skipped (not written).
        assert!(fs.written.lock().unwrap().is_empty());
        assert!(report.skipped.iter().any(|s| s.contains("já existe")));
    }
}
