//! Crate `zavik-shared` — the **domain** layer.
//!
//! Contains only universal types, with no business logic and no dependency on
//! external frameworks. It is the innermost crate in the architecture (Clean
//! Architecture): every other crate may depend on it, but it depends on
//! nobody.

use std::path::PathBuf;

/// Human-language selection for CLI messages (pt-BR + en). Machine output
/// (JSON/SARIF/exit codes) never goes through it — see module docs.
pub mod i18n;

/// Identifier of a plugin loaded in the Kernel (e.g. `"stack_detector"`).
pub type PluginId = String;

/// Execution context injected into plugins and use cases.
#[derive(Debug, Clone)]
pub struct Context {
    /// Root of the project being analyzed.
    pub project_root: PathBuf,
}

/// The single error type of the domain.
///
/// We encapsulate infrastructure messages in a `String` so that the domain does
/// not need to depend on external crates (serde, std::io, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    IoError(String),
    SerializationError(String),
    Unknown(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::i18n::t_fmt;
        match self {
            Error::IoError(m) => f.write_str(&t_fmt("erro de I/O: {}", &[m])),
            Error::SerializationError(m) => {
                f.write_str(&t_fmt("erro de desserialização: {}", &[m]))
            }
            Error::Unknown(m) => f.write_str(&t_fmt("erro desconhecido: {}", &[m])),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IoError(e.to_string())
    }
}

/// Programming language detected in a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    JavaScript,
    TypeScript,
    Python,
    Go,
    Java,
    Kotlin,
    Php,
    Ruby,
    CSharp,
    Dart,
    Swift,
    C,
    Cpp,
    Elixir,
    Unknown,
}

impl Language {
    /// Human-readable label for display.
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Rust => "Rust",
            Language::JavaScript => "JavaScript",
            Language::TypeScript => "TypeScript",
            Language::Python => "Python",
            Language::Go => "Go",
            Language::Java => "Java",
            Language::Kotlin => "Kotlin",
            Language::Php => "PHP",
            Language::Ruby => "Ruby",
            Language::CSharp => "C#",
            Language::Dart => "Dart",
            Language::Swift => "Swift",
            Language::C => "C",
            Language::Cpp => "C++",
            Language::Elixir => "Elixir",
            Language::Unknown => "Desconhecida",
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Category of a dependency within a manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    Runtime,
    Development,
    Build,
    Peer,
    Unknown,
}

impl DependencyKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DependencyKind::Runtime => "runtime",
            DependencyKind::Development => "dev",
            DependencyKind::Build => "build",
            DependencyKind::Peer => "peer",
            DependencyKind::Unknown => "?",
        }
    }
}

/// A dependency declared in a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub name: String,
    pub version: Option<String>,
    pub kind: DependencyKind,
}

impl Dependency {
    pub fn new(name: impl Into<String>, version: Option<String>, kind: DependencyKind) -> Self {
        Self {
            name: name.into(),
            version,
            kind,
        }
    }
}

/// Result of analyzing a "stack": a language plus the manifest that proves it,
/// along with the framework(s) and dependencies found.
///
/// A single project can yield several `ProjectStack`s (e.g. a Tauri app has
/// both a `Cargo.toml` and a `package.json`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectStack {
    pub language: Language,
    /// Project/package name, when declared in the manifest (e.g. `"meu-app"`).
    /// Used to name Docker images and binaries.
    pub name: Option<String>,
    pub frameworks: Vec<String>,
    pub package_manager: Option<String>,
    /// Path to the manifest that originated the detection (e.g. `package.json`).
    pub manifest: PathBuf,
    pub dependencies: Vec<Dependency>,
}

impl ProjectStack {
    pub fn new(language: Language, manifest: PathBuf, package_manager: Option<String>) -> Self {
        Self {
            language,
            name: None,
            frameworks: Vec::new(),
            package_manager,
            manifest,
            dependencies: Vec::new(),
        }
    }

    pub fn with_name(mut self, name: Option<String>) -> Self {
        self.name = name;
        self
    }

    pub fn with_frameworks(mut self, frameworks: Vec<String>) -> Self {
        self.frameworks = frameworks;
        self
    }

    pub fn with_dependencies(mut self, dependencies: Vec<Dependency>) -> Self {
        self.dependencies = dependencies;
        self
    }
}

/// Severity of an analysis finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Critical => "crítico",
            Severity::Warning => "aviso",
            Severity::Info => "info",
        }
    }

    /// The higher the value, the more severe (used to order the display).
    pub fn rank(&self) -> u8 {
        match self {
            Severity::Info => 0,
            Severity::Warning => 1,
            Severity::Critical => 2,
        }
    }
}

/// An issue found by the analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    /// Related package, when applicable.
    pub package: Option<String>,
    pub message: String,
}

impl Finding {
    pub fn new(severity: Severity, package: Option<String>, message: impl Into<String>) -> Self {
        Self {
            severity,
            package,
            message: message.into(),
        }
    }

    pub fn critical(package: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Critical, Some(package.into()), message)
    }

    pub fn warning(package: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, Some(package.into()), message)
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(Severity::Info, None, message)
    }
}

/// Complete result of an analysis.
#[derive(Debug, Default, Clone)]
pub struct AnalysisReport {
    pub findings: Vec<Finding>,
}

impl AnalysisReport {
    pub fn push(&mut self, finding: Finding) {
        self.findings.push(finding);
    }

    pub fn len(&self) -> usize {
        self.findings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }

    pub fn count(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }

    /// Findings ordered from most to least severe.
    pub fn sorted(&self) -> Vec<&Finding> {
        let mut findings: Vec<&Finding> = self.findings.iter().collect();
        findings.sort_by_key(|finding| std::cmp::Reverse(finding.severity.rank()));
        findings
    }
}

/// A trust/supply-chain risk detected **before** installing.
///
/// Represents something that can execute arbitrary code (e.g. `postinstall`) or
/// that does not match the registry (possible typo/fraud). Each risk requires
/// explicit confirmation — it is never covered by "yes to all".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustRisk {
    pub severity: Severity,
    /// Related package/origin, when applicable.
    pub package: Option<String>,
    /// A sentence explaining the risk.
    pub message: String,
    /// The concrete code/action that would run (e.g. the `postinstall` content).
    pub detail: String,
}

impl TrustRisk {
    pub fn new(
        severity: Severity,
        package: Option<String>,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            package,
            message: message.into(),
            detail: detail.into(),
        }
    }

    pub fn critical(
        package: impl Into<String>,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(Severity::Critical, Some(package.into()), message, detail)
    }

    pub fn warning(
        package: impl Into<String>,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(Severity::Warning, Some(package.into()), message, detail)
    }

    pub fn info(
        package: impl Into<String>,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(Severity::Info, Some(package.into()), message, detail)
    }
}

/// Result of the trust check (pre-installation).
#[derive(Debug, Default, Clone)]
pub struct TrustReport {
    pub risks: Vec<TrustRisk>,
}

impl TrustReport {
    pub fn push(&mut self, risk: TrustRisk) {
        self.risks.push(risk);
    }

    pub fn is_empty(&self) -> bool {
        self.risks.is_empty()
    }

    /// Risks that require confirmation (everything that is not merely informative).
    pub fn blocking(&self) -> Vec<&TrustRisk> {
        self.risks
            .iter()
            .filter(|r| r.severity != Severity::Info)
            .collect()
    }

    /// Risks ordered from most to least severe.
    pub fn sorted(&self) -> Vec<&TrustRisk> {
        let mut risks: Vec<&TrustRisk> = self.risks.iter().collect();
        risks.sort_by_key(|risk| std::cmp::Reverse(risk.severity.rank()));
        risks
    }
}

// ---------------------------------------------------------------------------
// Briefing (the `context` command) — saves an AI the exploration work
// ---------------------------------------------------------------------------

/// Ready-made commands for working on the project, inferred from manifests/CI.
/// What an AI would spend time reading the README/Makefile/package.json to discover.
#[derive(Debug, Default, Clone)]
pub struct Runbook {
    /// Install dependencies (e.g. `npm install`, `cargo fetch`).
    pub setup: Vec<String>,
    /// Compile/build.
    pub build: Vec<String>,
    /// Run the tests.
    pub test: Vec<String>,
    /// Run the project.
    pub run: Vec<String>,
    /// Lint/format.
    pub lint: Vec<String>,
}

/// An important project file, along with the role it plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyFile {
    /// Role: `manifest`, `lockfile`, `ci`, `docker`, `env`, `readme`, ...
    pub kind: String,
    pub path: String,
}

/// Compact briefing: how to build/test/run + where the entrypoints live.
#[derive(Debug, Default, Clone)]
pub struct Briefing {
    pub runbook: Runbook,
    /// Entrypoint files (e.g. `src/main.rs`, `index.js`, `cmd/api/main.go`).
    pub entrypoints: Vec<String>,
    pub key_files: Vec<KeyFile>,
}

// ---------------------------------------------------------------------------
// Error diagnosis (the `diagnose` command)
// ---------------------------------------------------------------------------

/// A probable cause of failure, with the fix and the lines that revealed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cause {
    /// Stable rule identifier (e.g. `missing-lib`).
    pub id: String,
    /// The REAL cause (not the symptom the error points at).
    pub cause: String,
    /// Next step / suggested fix.
    pub fix: String,
    /// When the literal error misleads: explains that it is only a symptom.
    pub note: Option<String>,
    /// Output lines that matched the rule (the "proof").
    pub evidence: Vec<String>,
}

/// Diagnosis of a failed command's output.
#[derive(Debug, Default, Clone)]
pub struct Diagnosis {
    /// Short summary of what is known.
    pub summary: String,
    /// Probable causes, from most to least likely.
    pub causes: Vec<Cause>,
    /// Fallback: lines that look like errors (when no rule matches).
    pub top_errors: Vec<String>,
}

// ---------------------------------------------------------------------------
// Supply-chain audit (`audit`) — consolidated standard
// (Socket.dev risk factors + Snyk/cargo-audit policy + OWASP suppression)
// ---------------------------------------------------------------------------

/// Severity in the CVSS/Snyk model (Low < Medium < High < Critical). Kept
/// separate from the 3-level [`Severity`] so the existing analysis does not break.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuditSeverity {
    Low,
    Medium,
    /// Conservative default (fails `high` and `critical` in CI).
    #[default]
    High,
    Critical,
}

impl AuditSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditSeverity::Low => "low",
            AuditSeverity::Medium => "medium",
            AuditSeverity::High => "high",
            AuditSeverity::Critical => "critical",
        }
    }
    pub fn rank(&self) -> u8 {
        match self {
            AuditSeverity::Low => 0,
            AuditSeverity::Medium => 1,
            AuditSeverity::High => 2,
            AuditSeverity::Critical => 3,
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "low" => Some(AuditSeverity::Low),
            "medium" | "med" => Some(AuditSeverity::Medium),
            "high" => Some(AuditSeverity::High),
            "critical" => Some(AuditSeverity::Critical),
            _ => None,
        }
    }
}

/// Behavioral risk factors (Socket.dev standard). **No single one is
/// a verdict** — severity comes from the confluence (sum of weights), and only a
/// score ≥ the threshold becomes a finding (so `^4.18` or a benign
/// `postinstall` do not turn into alerts for no reason).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiskFactor {
    InstallScript,
    NetworkAccess,
    ShellAccess,
    FilesystemAccess,
    EnvAccess,
    Obfuscation,
    NonRegistry,
    LowDownloads,
    Unpinned,
    NoIntegrity,
}

impl RiskFactor {
    pub fn id(&self) -> &'static str {
        match self {
            RiskFactor::InstallScript => "install-script",
            RiskFactor::NetworkAccess => "network-access",
            RiskFactor::ShellAccess => "shell-access",
            RiskFactor::FilesystemAccess => "filesystem-access",
            RiskFactor::EnvAccess => "env-access",
            RiskFactor::Obfuscation => "obfuscation",
            RiskFactor::NonRegistry => "non-registry",
            RiskFactor::LowDownloads => "low-downloads",
            RiskFactor::Unpinned => "unpinned",
            RiskFactor::NoIntegrity => "no-integrity",
        }
    }

    pub fn from_id(text: &str) -> Option<Self> {
        Some(match text.trim().to_ascii_lowercase().as_str() {
            "install-script" => RiskFactor::InstallScript,
            "network-access" => RiskFactor::NetworkAccess,
            "shell-access" => RiskFactor::ShellAccess,
            "filesystem-access" => RiskFactor::FilesystemAccess,
            "env-access" => RiskFactor::EnvAccess,
            "obfuscation" => RiskFactor::Obfuscation,
            "non-registry" => RiskFactor::NonRegistry,
            "low-downloads" => RiskFactor::LowDownloads,
            "unpinned" => RiskFactor::Unpinned,
            "no-integrity" => RiskFactor::NoIntegrity,
            _ => return None,
        })
    }
}

/// Provenance of a package.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PackageOrigin {
    #[default]
    Registry,
    Git(String),
    Path,
    File,
}

/// Confidence in the finding (a function of the confluence of factors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    Low,
    Medium,
    High,
}

impl Confidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Confidence::Low => "low",
            Confidence::Medium => "medium",
            Confidence::High => "high",
        }
    }

    /// 1 factor → low; 2 → medium; 3+ → high.
    pub fn from_factor_count(count: usize) -> Self {
        match count {
            0..=1 => Confidence::Low,
            2 => Confidence::Medium,
            _ => Confidence::High,
        }
    }
}

/// A package "frozen" for auditing. The engine is a pure function over these
/// snapshots; the CLI populates them from detection (and from the registry, if online).
#[derive(Debug, Clone, Default)]
pub struct PackageSnapshot {
    pub name: String,
    pub version: Option<String>,
    pub origin: PackageOrigin,
    /// Install script text (e.g. npm's `postinstall`), if known.
    pub install_script: Option<String>,
    pub downloads_last_week: Option<u64>,
    pub pinned: bool,
    pub has_integrity: bool,
    pub direct: bool,
}

/// False-positive suppression with a reason and **mandatory expiry** (the Snyk
/// `.snyk`/OWASP suppression standard). Requiring an expiry keeps an ignore from becoming a
/// permanent shield for a compromised package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suppression {
    pub package: String,
    pub factor: RiskFactor,
    pub reason: String,
    /// ISO date `YYYY-MM-DD` — lexicographic comparison suffices.
    pub expires: String,
}

/// Audit policy (tunable per team). Loaded from TOML, versioned.
#[derive(Debug, Clone, Default)]
pub struct AuditPolicy {
    /// Minimum severity that fails the CI (exit 1). Default: `high`.
    pub fail_on: AuditSeverity,
    /// Expected internal packages outside the registry (they must not become findings).
    pub allowlist: Vec<String>,
}

/// An audit finding (one per package, aggregating the factors).
#[derive(Debug, Clone)]
pub struct AuditFinding {
    pub package: String,
    pub factors: Vec<RiskFactor>,
    pub severity: AuditSeverity,
    /// Score 0–100 (sum of weights, capped).
    pub score: u8,
    pub confidence: Confidence,
    /// The evidence ("hook: curl|sh", "downloads=8/week").
    pub evidence: Vec<String>,
}

/// Complete audit report.
#[derive(Debug, Clone, Default)]
pub struct AuditReport {
    /// Worst score across packages (aggregate posture).
    pub score: u8,
    pub findings: Vec<AuditFinding>,
    /// Active suppressions applied.
    pub suppressed: Vec<Suppression>,
    /// Suppressions that have EXPIRED (they must be re-triaged).
    pub expired_suppressions: Vec<Suppression>,
    pub fail_on: AuditSeverity,
    pub passed: bool,
    pub packages_audited: usize,
}
