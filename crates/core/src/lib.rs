//! Crate `devforge-core` — the **use cases**.
//!
//! Contains the `WorkflowEngine`, which runs a workflow in the
//! **Workflow -> Step -> Action** model (ADR 0004), with retries and
//! exponential backoff for transient failures.
//!
//! The `core` knows neither the Kernel nor the UI: it receives a `Context` and
//! the capabilities (via `devforge-sdk`) it needs.

use devforge_shared::{Context, Error, ProjectStack};
use devforge_sdk::ProjectDetector;
use std::time::Duration;

pub mod provision;
pub mod toolchain;

/// Name of the step that triggers dependency/framework detection.
pub const STEP_DEPENDENCY_SCAN: &str = "dependency_scan";

/// Engine configuration.
pub struct WorkflowEngine {
    max_retries: u32,
    initial_backoff_secs: f64,
}

impl Default for WorkflowEngine {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff_secs: 1.0,
        }
    }
}

/// Final report of a workflow execution.
#[derive(Debug, Default)]
pub struct WorkflowReport {
    /// Steps executed, in order.
    pub executed_steps: Vec<String>,
    /// Detected stacks (filled in by the [`STEP_DEPENDENCY_SCAN`] step).
    pub detected_stacks: Vec<ProjectStack>,
}

impl WorkflowEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs the sequence of steps with retries.
    ///
    /// On a transient error, it retries honoring an exponential backoff; if all
    /// attempts fail, it returns the final error.
    pub fn run_workflow(
        &self,
        steps: &[String],
        context: &Context,
        detector: &dyn ProjectDetector,
    ) -> Result<WorkflowReport, String> {
        use devforge_shared::i18n::t_fmt;
        let steps_dbg = format!("{steps:?}");
        for attempt in 1..=self.max_retries {
            println!(
                "{}",
                t_fmt(
                    "CORE: tentativa {}/{} para os passos {}",
                    &[
                        &attempt.to_string(),
                        &self.max_retries.to_string(),
                        &steps_dbg,
                    ]
                )
            );

            match self.run_attempt(steps, context, detector) {
                Ok(report) => {
                    println!(
                        "{}",
                        t_fmt(
                            "CORE: workflow concluído na tentativa {}.",
                            &[&attempt.to_string()]
                        )
                    );
                    return Ok(report);
                }
                Err(error) if attempt < self.max_retries => {
                    let delay = self.initial_backoff_secs * 2f64.powi((attempt - 1) as i32);
                    eprintln!(
                        "{}",
                        t_fmt(
                            "CORE: falha transitória ('{}'). Retentando em {}s (backoff exponencial)...",
                            &[&error, &format!("{delay:.2}")]
                        )
                    );
                    std::thread::sleep(Duration::from_secs_f64(delay));
                }
                Err(error) => {
                    return Err(t_fmt(
                        "workflow falhou após {} tentativas. Erro final: {}",
                        &[&self.max_retries.to_string(), &error],
                    ));
                }
            }
        }
        unreachable!("o loop cobre todas as tentativas e sempre retorna")
    }

    /// A single pass through the steps, with no retry.
    fn run_attempt(
        &self,
        steps: &[String],
        context: &Context,
        detector: &dyn ProjectDetector,
    ) -> Result<WorkflowReport, String> {
        let mut report = WorkflowReport::default();

        for step in steps {
            println!(
                "{}",
                devforge_shared::i18n::t_fmt("CORE: -> executando passo '{}'", &[step])
            );
            if step == STEP_DEPENDENCY_SCAN {
                let stacks = detector.detect(context).map_err(|e: Error| e.to_string())?;
                report.detected_stacks = stacks;
            }
            report.executed_steps.push(step.clone());
        }

        Ok(report)
    }
}
