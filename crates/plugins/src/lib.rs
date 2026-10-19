//! Crate `zavik-plugins` — **concrete** implementations of the capabilities.
//!
//! In V1 the plugins are static (linked into the binary), but they already
//! implement the same contracts as `zavik-sdk`, so they can migrate to
//! dynamic plugins (`.dll`/`.so`) without changing the Kernel.
//!
//! - [`LocalFileSystem`]: implements [`zavik_sdk::FileSystemAccess`].
//! - [`StackDetector`]: implements [`zavik_sdk::ProjectDetector`].
//! - [`DependencyAnalyzer`]: implements [`zavik_sdk::ProjectAnalyzer`].

mod analyzer;
mod audit;
mod briefing;
mod diagnose;
mod detector;
mod filesystem;
mod process;
mod runner;

pub use analyzer::{npm_audit_signals, DependencyAnalyzer, NpmAuditSignal};
pub use audit::{audit_packages, classify_hook, HookAnalysis, HookVerdict};
pub use briefing::extract_briefing;
pub use diagnose::{diagnose, diagnose_in};
pub use detector::StackDetector;
pub use filesystem::LocalFileSystem;
pub use process::LocalCommandRunner;
pub use runner::{
    parse_health, resolve_run, supervise, HealthCheck, RunCandidate, RunPlan, RunResolution,
    RunState, SuperviseOptions, SupervisedOutcome,
};
