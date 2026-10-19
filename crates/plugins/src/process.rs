//! Plugin that executes commands on the operating system.
//!
//! Uses `std::process::Command`. Nothing here decides **what** to run: the `core`
//! builds the [`CommandSpec`] and the presentation layer asks permission first.

use zavik_shared::Error;
use zavik_sdk::{Capability, CommandOutput, CommandRunner, CommandSpec};

/// Real implementation via `std::process::Command`.
#[derive(Debug, Default)]
pub struct LocalCommandRunner;

impl Capability for LocalCommandRunner {
    fn name(&self) -> &'static str {
        "command_runner"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_command_runner(&self) -> Option<&dyn CommandRunner> {
        Some(self)
    }
}

impl CommandRunner for LocalCommandRunner {
    fn run(&self, command: &CommandSpec) -> Result<CommandOutput, Error> {
        let output = std::process::Command::new(&command.program)
            .args(&command.args)
            .current_dir(&command.working_dir)
            .output()
            .map_err(|e| {
                Error::IoError(format!(
                    "não foi possível executar '{}': {e}",
                    command.display()
                ))
            })?;

        Ok(CommandOutput {
            success: output.status.success(),
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    /// Runs with **inherited stdio**: the child process talks straight to the
    /// user's terminal. Needed for commands that prompt for a password on the
    /// spot (e.g.: `sudo apt install`, `brew install`). No shell — still argv by argv.
    fn run_interactive(&self, command: &CommandSpec) -> Result<Option<i32>, Error> {
        let status = std::process::Command::new(&command.program)
            .args(&command.args)
            .current_dir(&command.working_dir)
            .status()
            .map_err(|e| {
                Error::IoError(format!(
                    "não foi possível executar '{}': {e}",
                    command.display()
                ))
            })?;
        Ok(status.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zavik_sdk::CommandSpec;
    use std::path::PathBuf;

    #[cfg(unix)]
    #[test]
    fn run_interactive_runs_and_returns_code() {
        let runner = LocalCommandRunner;
        let spec = CommandSpec::new("true", Vec::<String>::new(), PathBuf::from("/"));
        assert_eq!(runner.run_interactive(&spec).unwrap(), Some(0));
    }

    #[test]
    fn run_interactive_missing_program_errors() {
        let runner = LocalCommandRunner;
        let spec = CommandSpec::new(
            "comando-que-nao-existe-xyz",
            Vec::<String>::new(),
            PathBuf::from("/"),
        );
        assert!(runner.run_interactive(&spec).is_err());
    }
}
