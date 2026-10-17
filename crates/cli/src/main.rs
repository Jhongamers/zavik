//! DevForge agent entry point (CLI).
//!
//! Commands:
//!     devforge [path]                          # detects the stack (default)
//!     devforge detect [path]
//!     devforge analyze [path] [flags]          # analyzes dependencies (lockfile, versions)
//!     devforge context [path] [flags]          # briefing: runbook/entrypoints/risks (good for AI)
//!     devforge diagnose [flags]                # summarizes an error: root cause + fix
//!     devforge env [path] [flags]              # environment fingerprint: tool versions + what's missing
//!     devforge audit [path] [flags]            # supply-chain audit (--fail-on gates CI)
//!     devforge prepare [path] [flags]          # installs dependencies (no Docker)
//!     devforge docker  [path] [flags]          # generates Dockerfile + builds the image
//!
//! Flags:
//!     --dry-run   shows the plan without executing anything
//!     --yes, -y   approves everything automatically (without asking)
//!     --force     allows overwriting existing files (e.g., Dockerfile)
//!     --online    (analyze) queries registries to find new versions
//!     --json      structured output (pure JSON on stdout) for AIs
//!     --all       descends into subfolders looking for projects (monorepo/nested)

mod dataset;
mod detached;
mod license;
mod output;
mod permission;
mod up;

use devforge_core::provision::{ExecuteOptions, PrepareMode, PrepareReport, Provisioner};
use devforge_core::toolchain::{
    env_snapshot, install_command, install_hint, missing_tools, MissingTool, ToolStatus,
};
use devforge_core::WorkflowEngine;
use devforge_kernel::Kernel;
use devforge_plugins::{
    diagnose, extract_briefing, npm_audit_signals, parse_health, resolve_run, supervise,
    DependencyAnalyzer, LocalCommandRunner, LocalFileSystem, NpmAuditSignal, RunCandidate, RunPlan,
    RunResolution, RunState, StackDetector, SuperviseOptions, SupervisedOutcome,
};
use devforge_shared::{
    AnalysisReport, AuditPolicy, AuditReport, AuditSeverity, Context, Language, PackageOrigin,
    PackageSnapshot, ProjectStack, RiskFactor, Severity, Suppression, TrustReport,
};
use devforge_sdk::{AnalysisOptions, CommandSpec, Decision, PermissionGate, PermissionRequest, PluginContext};
use permission::{AutoApprovePermissionGate, InteractivePermissionGate};
use serde_json::json;
use detached::spawn_detached;
use up::UpStep;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    // `--lang` is pulled before dispatch so it reaches EVERY parser —
    // including `license`/`data`, which parse their own flags. Values after
    // `--` belong to the child command and are left alone.
    let lang_flag = extract_lang(&mut args)?;
    devforge_shared::i18n::set_lang(devforge_shared::i18n::resolve(lang_flag.as_deref())?);

    let (command, rest): (&str, &[String]) = match args.first().map(String::as_str) {
        Some("prepare") => ("prepare", &args[1..]),
        Some("docker") => ("docker", &args[1..]),
        Some("analyze") => ("analyze", &args[1..]),
        Some("detect") => ("detect", &args[1..]),
        Some("context") => ("context", &args[1..]),
        Some("diagnose") => ("diagnose", &args[1..]),
        Some("env") => ("env", &args[1..]),
        Some("audit") => ("audit", &args[1..]),
        Some("run") => ("run", &args[1..]),
        Some("up") => ("up", &args[1..]),
        Some("license") => ("license", &args[1..]),
        Some("data") => ("data", &args[1..]),
        Some("help") | Some("--help") | Some("-h") => ("help", &[]),
        Some("--version") | Some("-V") => ("version", &[]),
        _ => ("detect", &args[..]),
    };

    if command == "help" {
        print_help();
        return Ok(());
    }
    if command == "version" {
        println!("devforge {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    // The license needs neither kernel nor banner — it is purely local verification.
    if command == "license" {
        return run_license(rest);
    }
    // Same for `data`: `status` is a local read (free); `update` is the one
    // network command and the only license-gated one (docs/design-data.md).
    if command == "data" {
        return run_data(rest);
    }

    let (path, options) = parse_options(rest)?;
    // `up` also accepts a git URL as target: the project root only exists after
    // the clone step, so until then we work from the current directory.
    let up_url: Option<String> = if command == "up" {
        path.as_ref()
            .map(|target| target.to_string_lossy().into_owned())
            .filter(|target| up::is_git_url(target))
    } else {
        None
    };
    let project_root = match &path {
        Some(path) if up_url.is_none() => path.clone(),
        _ => env::current_dir()?,
    };
    let context = Context {
        project_root: project_root.clone(),
    };

    if !options.json && !options.sarif {
        banner(&format!("DEVFORGE — {command}"), &project_root);
    }
    let kernel = build_kernel(&context, options.all)?;

    match command {
        "detect" => run_detect(&kernel, &context, &options)?,
        "analyze" => run_analyze(&kernel, &context, &options)?,
        "context" => run_context(&kernel, &context, &options)?,
        "diagnose" => run_diagnose(&options)?,
        "audit" => run_audit(&kernel, &context, &options)?,
        "env" => run_env(&kernel, &context, &options)?,
        "run" => run_run(&kernel, &context, &options)?,
        "up" => run_up(up_url.as_deref(), &kernel, &context, &options)?,
        "prepare" => {
            run_prepare(&kernel, &context, PrepareMode::Local, &options)?;
        }
        "docker" => {
            run_prepare(&kernel, &context, PrepareMode::Docker, &options)?;
        }
        _ => unreachable!("comando tratado acima"),
    }

    Ok(())
}

/// Configures and initializes the Kernel with the V1 plugins.
fn build_kernel(
    context: &Context,
    recursive: bool,
) -> Result<Kernel, Box<dyn std::error::Error>> {
    let mut kernel = Kernel::new();

    // The presentation layer just listens to domain events.
    kernel
        .event_bus()
        .subscribe(|event| eprintln!("   [logger] evento de domínio: '{}'", event.event_type));

    let plugin_context = PluginContext {
        id: "system".to_string(),
        context: context.clone(),
    };
    kernel.load_plugin(Box::new(LocalFileSystem), &plugin_context)?;
    kernel.load_plugin(Box::new(StackDetector { recursive }), &plugin_context)?;
    kernel.load_plugin(Box::new(DependencyAnalyzer), &plugin_context)?;
    kernel.load_plugin(Box::new(LocalCommandRunner), &plugin_context)?;

    kernel.run_initialization();
    Ok(kernel)
}

fn run_detect(
    kernel: &Kernel,
    context: &Context,
    options: &Options,
) -> Result<(), Box<dyn std::error::Error>> {
    if options.json {
        let stacks = kernel.detect_stacks(context)?;
        output::detect(context, &stacks);
        return Ok(());
    }
    // Demonstrates the WorkflowEngine (the original use case).
    section("Workflow de detecção (dependency_scan)");
    let detector = kernel
        .detector()
        .ok_or("plugin 'stack_detector' não registrado")?;
    let steps = vec!["step_1_setup".to_string(), "dependency_scan".to_string()];
    let report = WorkflowEngine::new().run_workflow(&steps, context, detector)?;
    print_stacks(&report.detected_stacks);
    Ok(())
}

/// Read-only environment fingerprint: OS/arch + each detected stack's tools
/// with presence and version. No permission gate (nothing executes beyond
/// `--version` probes of tools already on the PATH) — pure agent ground truth
/// for "does this machine even work?" (ROADMAP 5.4).
fn run_env(
    kernel: &Kernel,
    context: &Context,
    options: &Options,
) -> Result<(), Box<dyn std::error::Error>> {
    let stacks = kernel.detect_stacks(context)?;
    let Some(runner) = kernel.command_runner() else {
        return Err("plugin 'local_command_runner' não registrado".into());
    };
    let tools = env_snapshot(runner, &stacks, &context.project_root)?;

    if options.json {
        output::env(&stacks, &tools);
        return Ok(());
    }

    use devforge_shared::i18n::{t, t_fmt};
    section("AMBIENTE");
    println!(
        "  os: {} ({})\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    for stack in &stacks {
        let lang_tools: Vec<&ToolStatus> = tools
            .iter()
            .filter(|tool| tool.language == stack.language)
            .collect();
        if lang_tools.is_empty() {
            continue;
        }
        println!("  {}:", stack.language);
        for tool in lang_tools {
            if tool.present {
                println!(
                    "    • {} ✅ {}",
                    tool.tool,
                    tool.version.as_deref().unwrap_or(t("(versão não lida)"))
                );
            } else {
                println!(
                    "{}",
                    t_fmt(
                        "    • {} ❌ fora do PATH — sugestão: {}",
                        &[&tool.tool, &install_hint(&tool.tool)]
                    )
                );
            }
        }
    }
    if stacks.is_empty() {
        println!("{}", t("  nenhum stack detectado nesta pasta."));
    }
    println!(
        "{}",
        t("\n(leitura apenas: checa o PATH e roda `<tool> --version` de ferramentas")
    );
    println!(
        "{}",
        t("já instaladas; não instala nada e nunca lê arquivos .env)")
    );
    Ok(())
}

fn run_analyze(
    kernel: &Kernel,
    context: &Context,
    options: &Options,
) -> Result<(), Box<dyn std::error::Error>> {
    let stacks = kernel.detect_stacks(context)?;
    let analyzer = kernel
        .analyzer()
        .ok_or("plugin 'dependency_analyzer' não registrado")?;
    let analysis_options = AnalysisOptions {
        check_latest: options.online,
    };
    let report = analyzer.analyze(context, &stacks, &analysis_options)?;

    if options.json {
        output::analyze(context, &stacks, &report);
        return Ok(());
    }

    print_stacks(&stacks);
    if stacks.is_empty() {
        return Ok(());
    }
    if options.online {
        println!("\nConsultando registries (crates.io / npm / PyPI)… isso pode demorar.");
    }
    print_findings(&report);
    Ok(())
}

fn run_context(
    kernel: &Kernel,
    context: &Context,
    options: &Options,
) -> Result<(), Box<dyn std::error::Error>> {
    let stacks = kernel.detect_stacks(context)?;
    let trust = match kernel.analyzer() {
        Some(analyzer) => analyzer.verify_trust(context, &stacks)?,
        None => TrustReport::default(),
    };
    let briefing = extract_briefing(&context.project_root, &stacks);

    if options.json {
        output::context(context, &stacks, &trust, &briefing);
        return Ok(());
    }

    print_stacks(&stacks);
    if stacks.is_empty() {
        println!("Nenhum stack reconhecido.");
        return Ok(());
    }

    let rb = &briefing.runbook;
    section("COMO RODAR (runbook)");
    print_bucket("setup", &rb.setup);
    print_bucket("build", &rb.build);
    print_bucket("test", &rb.test);
    print_bucket("run", &rb.run);
    print_bucket("lint", &rb.lint);

    if !briefing.entrypoints.is_empty() {
        section("ENTRADAS");
        for entry in &briefing.entrypoints {
            println!("  • {entry}");
        }
    }

    if !briefing.key_files.is_empty() {
        section("ARQUIVOS-CHAVE");
        for key in &briefing.key_files {
            println!("  [{}] {}", key.kind, key.path);
        }
    }

    if !trust.is_empty() {
        section("CONFIANÇA (resumo)");
        for risk in trust.sorted() {
            println!("  [{}] {}", risk.severity.as_str(), risk.message);
        }
    }
    Ok(())
}

fn print_bucket(label: &str, cmds: &[String]) {
    if cmds.is_empty() {
        return;
    }
    println!("  {label}:");
    for cmd in cmds {
        println!("     $ {cmd}");
    }
}

fn run_diagnose(options: &Options) -> Result<(), Box<dyn std::error::Error>> {
    use devforge_shared::i18n::Lang;
    let text = read_error_input(options)?;
    // JSON prose is always English (dogfooding #6); human output follows
    // the current language. The texts are composed ONCE, in that language.
    let target = if options.json {
        Lang::En
    } else {
        devforge_shared::i18n::lang()
    };
    let diagnosis = devforge_plugins::diagnose_in(target, &text);

    if options.json {
        output::diagnose(&diagnosis);
        return Ok(());
    }

    section("DIAGNÓSTICO");
    print_diagnosis(&diagnosis);
    Ok(())
}

/// Human-readable printing of a [`devforge_shared::Diagnosis`] (reused by `run`).
/// The body texts are already in the current language (composed at
/// construction); only the labels translate here.
fn print_diagnosis(diagnosis: &devforge_shared::Diagnosis) {
    use devforge_shared::i18n::{t, t_fmt};
    println!("{}", diagnosis.summary);
    for cause in &diagnosis.causes {
        println!("\n• {}", cause.cause);
        if let Some(note) = &cause.note {
            println!("{}", t_fmt("  nota: {}", &[note]));
        }
        println!("{}", t_fmt("  conserto: {}", &[&cause.fix]));
        if !cause.evidence.is_empty() {
            println!("{}", t("  provas:"));
            for line in &cause.evidence {
                println!("    {line}");
            }
        }
    }
    if diagnosis.causes.is_empty() && !diagnosis.top_errors.is_empty() {
        println!("\n{}", t("Linhas com mais cara de erro:"));
        for line in &diagnosis.top_errors {
            println!("  {line}");
        }
    }
}

/// Where the error text comes from: `--message`, `--input <file>`, or stdin
/// (e.g., `meu_build 2>&1 | devforge diagnose`).
fn read_error_input(options: &Options) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(message) = &options.message {
        return Ok(message.clone());
    }
    if let Some(path) = &options.input {
        return Ok(std::fs::read_to_string(path)?);
    }
    use std::io::{IsTerminal, Read};
    // Only read stdin if it is NOT a terminal (otherwise it would block waiting for input).
    if std::io::stdin().is_terminal() {
        return Err(
            devforge_shared::i18n::t(
                "cole a saída do erro, ou use --message \"...\", --input <arquivo> \
                 (ou pipe: `meu_comando 2>&1 | devforge diagnose`)",
            )
            .into(),
        );
    }
    let mut buffer = String::new();
    std::io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer)
}

// ---------------------------------------------------------------------------
// `audit` — supply-chain audit (Socket/Snyk/OWASP pattern)
// ---------------------------------------------------------------------------

fn run_audit(
    kernel: &Kernel,
    context: &Context,
    options: &Options,
) -> Result<(), Box<dyn std::error::Error>> {
    let stacks = kernel.detect_stacks(context)?;
    if stacks.is_empty() {
        return Err("nenhuma stack detectada pra auditar".into());
    }

    // Policy + suppressions (versioned TOML) + override via --fail-on.
    let mut policy = load_policy(options.policy.as_deref())?;
    if let Some(fail_on) = &options.fail_on {
        policy.fail_on = AuditSeverity::parse(fail_on).ok_or_else(|| {
            format!("--fail-on inválido: {fail_on} (use low|medium|high|critical)")
        })?;
    }
    let suppressions = load_suppressions(options.ignore.as_deref())?;

    // Online enrichment (npm): install hook + downloads/week.
    let mut signals: HashMap<String, NpmAuditSignal> = HashMap::new();
    if options.online {
        let npm_deps: Vec<(String, Option<String>)> = stacks
            .iter()
            .filter(|s| matches!(s.language, Language::JavaScript | Language::TypeScript))
            .flat_map(|s| s.dependencies.iter())
            .filter(|d| matches!(classify_origin(d.version.as_deref()), PackageOrigin::Registry))
            .map(|d| (d.name.clone(), d.version.clone()))
            .collect();
        signals = npm_audit_signals(&npm_deps);
    }

    let snapshots = build_snapshots(&stacks, &signals);
    let report =
        devforge_plugins::audit_packages(&snapshots, &policy, &suppressions, &today_iso());

    // Pro: a valid signed LOCAL dataset adds an advisory section. Absent →
    // output stays byte-identical to the free tier (design-data §6 invariant).
    // Installed-but-invalid is a loud warning on stderr (never a silent pass).
    let installed = match dataset::load_installed() {
        Ok(installed) => installed,
        Err(error) => {
            eprintln!(
                "{}",
                devforge_shared::i18n::t_fmt(
                    "(dataset instalado INVÁLIDO — ignorado: {})",
                    &[&error.to_string()],
                )
            );
            None
        }
    };
    let dataset_report = installed
        .as_ref()
        .map(|ds| dataset::build_report(&stacks, ds, &today_iso()));

    if options.sarif {
        let sarif = build_sarif(&report, &stacks, &context.project_root);
        println!("{}", serde_json::to_string_pretty(&sarif)?);
    } else if options.json {
        output::audit(&report, dataset_report.as_ref());
    } else {
        print_audit(&report, options.online, dataset_report.as_ref());
    }

    if !report.passed {
        // Exit code that gates CI (Snyk/cargo-audit pattern).
        std::process::exit(1);
    }
    Ok(())
}

fn build_snapshots(
    stacks: &[ProjectStack],
    signals: &HashMap<String, NpmAuditSignal>,
) -> Vec<PackageSnapshot> {
    let mut out = Vec::new();
    for stack in stacks {
        for dep in &stack.dependencies {
            let origin = classify_origin(dep.version.as_deref());
            let mut snapshot = PackageSnapshot {
                name: dep.name.clone(),
                version: dep.version.clone(),
                origin,
                install_script: None,
                downloads_last_week: None,
                pinned: is_pinned(dep.version.as_deref()),
                // Lockfile integrity is not yet verified (future work).
                has_integrity: true,
                direct: true,
            };
            if let Some(signal) = signals.get(&dep.name.to_lowercase()) {
                snapshot.install_script = signal.install_script.clone();
                snapshot.downloads_last_week = signal.downloads_last_week;
            }
            out.push(snapshot);
        }
    }
    out
}

/// Classifies the origin from the version requirement.
fn classify_origin(version: Option<&str>) -> PackageOrigin {
    let Some(version) = version else {
        return PackageOrigin::Registry;
    };
    let lower = version.to_ascii_lowercase();
    let looks_git = lower.starts_with("git")
        || lower.contains("github.com")
        || lower.contains("gitlab.com")
        || lower.contains("bitbucket.org")
        || lower.contains("git+");
    if looks_git {
        return PackageOrigin::Git(version.to_string());
    }
    if lower.starts_with("file:") {
        return PackageOrigin::File;
    }
    if lower.starts_with("link:")
        || lower.starts_with("portal:")
        || lower.starts_with("./")
        || lower.starts_with("../")
    {
        return PackageOrigin::Path;
    }
    PackageOrigin::Registry
}

/// Exactly pinned version (`1.2.3`, `=1.2.3`), no range/wildcard.
fn is_pinned(version: Option<&str>) -> bool {
    let Some(version) = version else {
        return false;
    };
    let bare = version.trim().trim_start_matches('=');
    !bare.is_empty() && bare.chars().all(|c| c.is_ascii_digit() || c == '.')
}

fn load_policy(path: Option<&Path>) -> Result<AuditPolicy, Box<dyn std::error::Error>> {
    let Some(path) = path else {
        return Ok(AuditPolicy::default());
    };
    let text = std::fs::read_to_string(path)?;
    let value: toml::Value = text.parse()?;
    let mut policy = AuditPolicy::default();
    if let Some(fail_on) = value.get("fail_on").and_then(|v| v.as_str()) {
        policy.fail_on = AuditSeverity::parse(fail_on)
            .ok_or_else(|| format!("fail_on inválido no policy: {fail_on}"))?;
    }
    if let Some(list) = value.get("allowlist").and_then(|v| v.as_array()) {
        policy.allowlist = list
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect();
    }
    Ok(policy)
}

fn load_suppressions(path: Option<&Path>) -> Result<Vec<Suppression>, Box<dyn std::error::Error>> {
    let Some(path) = path else {
        return Ok(Vec::new());
    };
    let text = std::fs::read_to_string(path)?;
    let value: toml::Value = text.parse()?;
    let mut out = Vec::new();
    if let Some(entries) = value.get("ignore").and_then(|v| v.as_array()) {
        for entry in entries {
            let package = entry
                .get("package")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let factor_id = entry.get("factor").and_then(|v| v.as_str()).unwrap_or_default();
            let reason = entry
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let expires = entry
                .get("expires")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let Some(factor) = RiskFactor::from_id(factor_id) else {
                return Err(format!("factor desconhecido na supressão: {factor_id}").into());
            };
            // Expiration is MANDATORY — an ignore never becomes an eternal shield.
            if expires.is_empty() {
                return Err(format!(
                    "supressão de '{package}' sem 'expires' (expiração é obrigatória)"
                )
                .into());
            }
            out.push(Suppression {
                package,
                factor,
                reason,
                expires,
            });
        }
    }
    Ok(out)
}

fn print_audit(report: &AuditReport, online: bool, dataset: Option<&dataset::DatasetReport>) {
    use devforge_shared::i18n::{t, t_fmt};

    section("AUDITORIA DE SUPPLY-CHAIN");
    let packages = report.packages_audited.to_string();
    println!("{}", t_fmt("Pacotes auditados: {}", &[&packages]));
    if report.packages_audited == 0 {
        // Anti-false-positive: approving ZERO packages certifies nothing.
        println!("\n{}", t("⚠ 0 pacotes auditados — não há o que atestar aqui"));
        println!(
            "{}",
            t("   (manifesto sem dependências declaradas — ex.: projeto novo, ou aponte")
        );
        println!(
            "{}",
            t("    o DevForge num crate/membro específico). O veredito NÃO atesta nada.")
        );
    }
    let score = report.score.to_string();
    println!("{}", t_fmt("Score (pior pacote): {}/100", &[&score]));
    if !online {
        println!(
            "{}",
            t("(offline: hooks/downloads de registry não consultados — use --online)")
        );
    }

    if report.findings.is_empty() {
        println!("\n{}", t("Nenhuma achação acima do limiar. ✅"));
    }
    for finding in &report.findings {
        let severity = finding.severity.as_str().to_uppercase();
        let finding_score = finding.score.to_string();
        println!(
            "\n{}",
            t_fmt(
                "• {} — {}  (score {}, confiança {})",
                &[
                    &finding.package,
                    &severity,
                    &finding_score,
                    finding.confidence.as_str(),
                ],
            )
        );
        let factors = finding
            .factors
            .iter()
            .map(|f| f.id())
            .collect::<Vec<_>>()
            .join(", ");
        println!("  {}", t_fmt("fatores: {}", &[&factors]));
        for evidence in &finding.evidence {
            println!("    · {evidence}");
        }
    }

    if !report.suppressed.is_empty() {
        let active = report.suppressed.len().to_string();
        println!("\n{}", t_fmt("Supressões ativas: {}", &[&active]));
        for s in &report.suppressed {
            let expires = s.expires.as_str();
            println!(
                "{}",
                t_fmt("  - {} ({}) expira {}", &[s.package.as_str(), s.factor.id(), expires])
            );
        }
    }
    if !report.expired_suppressions.is_empty() {
        let expired = report.expired_suppressions.len().to_string();
        println!(
            "\n{}",
            t_fmt("⚠ Supressões EXPIRADAS (re-triagem necessária): {}", &[&expired])
        );
        for s in &report.expired_suppressions {
            let expires = s.expires.as_str();
            println!(
                "{}",
                t_fmt("  - {} ({}) expirou {}", &[s.package.as_str(), s.factor.id(), expires])
            );
        }
    }

    if let Some(dataset) = dataset {
        print_dataset_section(dataset);
    }

    if report.packages_audited == 0 {
        println!(
            "\n{}",
            t("Veredito: SEM DADOS ⚠️  (0 pacotes — nada foi atestado)")
        );
    } else {
        let verdict = if report.passed {
            t("APROVADO ✅")
        } else {
            t("REPROVADO ❌")
        };
        println!("\n{}", t_fmt("Veredito: {}", &[verdict]));
    }
}

/// Pro section of `audit`: findings from the signed local dataset. Advisory
/// only — never touches score, `passed` or `--fail-on` (design-data §2.1).
fn print_dataset_section(report: &dataset::DatasetReport) {
    use devforge_shared::i18n::{t, t_fmt};
    section(t("DADOS FRESCOS (Pro — dataset assinado)"));
    println!(
        "{}",
        t_fmt(
            "dataset {} de {} (idade: {} dia(s); {} advisories na janela, {} na kill list)",
            &[
                &report.version.to_string(),
                &report.generated,
                &report.age_days.to_string(),
                &report.advisories_total.to_string(),
                &report.bad_names_total.to_string(),
            ]
        )
    );
    if report.stale {
        println!(
            "{}",
            t_fmt(
                "⚠ atrasado (>{} dias) — rode: devforge data update",
                &[&dataset::STALE_AFTER_DAYS.to_string()]
            )
        );
    }

    if report.findings.is_empty() {
        println!(
            "{}",
            t("\nNenhum achado novo nas dependências deste projeto.")
        );
        return;
    }
    let (bad, advisories): (Vec<_>, Vec<_>) =
        report.findings.iter().partition(|f| f.kind == "bad_name");
    if !bad.is_empty() {
        println!(
            "{}",
            t("\n🛑 Nome malicioso/typosquat presente nas dependências:")
        );
        for finding in bad {
            println!(
                "   • {} ({}) — {}",
                finding.package, finding.ecosystem, finding.detail
            );
            if let Some(url) = &finding.url {
                println!("     ↳ {url}");
            }
        }
    }
    if !advisories.is_empty() {
        println!(
            "{}",
            t("\n⚠ Possível exposição (casamento só por nome — confira sua versão):")
        );
        for finding in advisories {
            println!(
                "   • {} ({}) — {}",
                finding.package, finding.ecosystem, finding.detail
            );
            if let Some(url) = &finding.url {
                println!("     ↳ {url}");
            }
        }
    }
    println!(
        "{}",
        t("\n(seção informativa: não altera o score nem o --fail-on do audit)")
    );
}

/// `devforge run` — resolves how the project starts and runs (foreground, v1).
///
/// Ladder: declared (scripts.start/Procfile/Makefile/compose) > convention >
/// ask. Readiness = port heuristic (stated as a heuristic). Early crash
/// becomes a `diagnose` input. Execution is a critical risk: a human decides.
fn run_run(
    kernel: &Kernel,
    context: &Context,
    options: &Options,
) -> Result<(), Box<dyn std::error::Error>> {
    use devforge_shared::i18n::{t_fmt_in, translate, Lang};
    let health = options.health.as_deref().map(parse_health).transpose()?;
    let timeout_secs = options.timeout.unwrap_or(60);
    // One target language per invocation: JSON prose is always English,
    // human output follows --lang (docs/design-i18n.md). Strings shared by
    // both paths compose here, in that language; human-only messages use
    // `lang` too (identical to the current language when not JSON).
    let lang = if options.json {
        Lang::En
    } else {
        devforge_shared::i18n::lang()
    };

    let mut plan = match resolve_run(&context.project_root) {
        RunResolution::Ready(plan) => plan,
        RunResolution::NotFound => {
            let mut languages: Vec<String> = kernel
                .detect_stacks(context)?
                .iter()
                .map(|stack| stack.language.to_string())
                .collect();
            languages.sort();
            languages.dedup();
            let languages = languages.join(", ");
            if options.json {
                println!(
                    "{}",
                    json!({"schema": 1, "command": "run", "status": "not_found"})
                );
            } else {
                println!(
                    "{}",
                    translate(lang, "Não achei como iniciar este projeto.")
                );
                if !languages.is_empty() {
                    println!(
                        "{}",
                        t_fmt_in(lang, "   linguagens detectadas: {}", &[&languages])
                    );
                }
                println!(
                    "{}",
                    translate(
                        lang,
                        "   declare um start (ex.: package.json scripts.start, Procfile, Makefile com alvo 'run')"
                    )
                );
            }
            std::process::exit(1);
        }
        RunResolution::Ambiguous(candidates) => {
            if options.json {
                let list: Vec<serde_json::Value> = candidates
                    .iter()
                    .map(|item| {
                        json!({
                            "label": item.label,
                            "command": format!("{} {}", item.command, item.args.join(" ")).trim(),
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    json!({"schema": 1, "command": "run", "status": "ambiguous", "candidates": list})
                );
                std::process::exit(1);
            }
            match pick_start_candidate(&candidates) {
                Some(chosen) => chosen,
                None => {
                    println!("{}", translate(lang, "   (cancelado — nada foi executado)"));
                    std::process::exit(1);
                }
            }
        }
    };

    if !options.extra_args.is_empty() {
        plan.args.extend(options.extra_args.iter().cloned());
    }
    let display = format!("{} {}", plan.command, plan.args.join(" ")).trim().to_string();

    if options.dry_run {
        if options.json {
            println!(
                "{}",
                json!({"schema": 1, "command": "run", "status": "planned",
                       "resolved": display, "source": plan.source})
            );
        } else {
            println!("{}", translate(lang, "\nPlano de execução:"));
            println!("{}", t_fmt_in(lang, "   comando : {}", &[&display]));
            println!("{}", t_fmt_in(lang, "   origem  : {}", &[&plan.source]));
            println!("{}", translate(lang, "   (dry-run: nada foi executado)"));
        }
        return Ok(());
    }

    // Code execution risk: individual human confirmation; --yes does not cover it.
    let gate: Box<dyn PermissionGate> = if options.yes {
        Box::new(AutoApprovePermissionGate)
    } else {
        Box::new(InteractivePermissionGate::new())
    };
    let decision = gate.confirm(&PermissionRequest {
        title: translate(lang, "Executar o projeto").to_string(),
        detail: display.clone(),
        reason: translate(
            lang,
            "roda código do projeto local com as credenciais do seu usuário",
        )
        .to_string(),
        critical: true,
    });
    if decision == Decision::Denied {
        if options.json {
            println!(
                "{}",
                json!({"schema": 1, "command": "run", "status": "denied"})
            );
        } else {
            println!(
                "{}",
                translate(
                    lang,
                    "   ✗ negado (nada foi executado). --yes não cobre execução de código."
                )
            );
        }
        std::process::exit(1);
    }

    // Detached mode: start in the background and return the PID immediately
    // (an AI must never wait). Honest limitation: no readiness supervision
    // here — the log goes to `.devforge/run.log`; `run` without `--detach`
    // keeps the health check + crash→diagnose.
    if options.detach {
        let log_dir = context.project_root.join(".devforge");
        let log_path = log_dir.join("run.log");
        // These error strings can land in JSON (`error`) too: compose them
        // in the target language, not the current one.
        let spawn_result = std::fs::create_dir_all(&log_dir)
            .map_err(|error| {
                t_fmt_in(
                    lang,
                    "não preparei o diretório de log {}: {}",
                    &[&log_dir.display().to_string(), &error.to_string()],
                )
            })
            .and_then(|_| {
                spawn_detached(&plan.command, &plan.args, &context.project_root, &log_path).map_err(
                    |error| {
                        t_fmt_in(
                            lang,
                            "não foi possível executar '{}': {}",
                            &[&display, &error.to_string()],
                        )
                    },
                )
            });
        match spawn_result {
            Ok(pid) => {
                if options.json {
                    println!(
                        "{}",
                        json!({"schema": 1, "command": "run", "status": "detached",
                               "pid": pid, "resolved": display, "source": plan.source,
                               "log": log_path.to_string_lossy(),
                               "stop_command": format!("kill {pid}")})
                    );
                } else {
                    println!(
                        "{}",
                        t_fmt_in(lang, "\n▶ rodando em background: {}", &[&display])
                    );
                    println!("   pid  : {pid}");
                    println!(
                        "{}",
                        t_fmt_in(
                            lang,
                            "   log  : {}   (tail -f pra acompanhar)",
                            &[&log_path.display().to_string()]
                        )
                    );
                    println!(
                        "{}",
                        t_fmt_in(
                            lang,
                            "   parar: kill {}   (se precisar: kill -9 {})",
                            &[&pid.to_string(), &pid.to_string()]
                        )
                    );
                    println!(
                        "{}",
                        translate(
                            lang,
                            "\n   (detached não espera ficar pronto nem supervisiona — pra isso"
                        )
                    );
                    println!(
                        "{}",
                        translate(
                            lang,
                            "    rode `devforge run` sem --detach, com health check e diagnose)"
                        )
                    );
                    #[cfg(windows)]
                    println!(
                        "{}",
                        translate(
                            lang,
                            "    no Windows o processo continua ligado ao console (limitação conhecida)"
                        )
                    );
                }
                return Ok(());
            }
            Err(message) => {
                let diagnosis = diagnose(&message);
                if options.json {
                    println!(
                        "{}",
                        json!({"schema": 1, "command": "run", "status": "failed", "error": message})
                    );
                } else {
                    println!("❌ {message}");
                    print_diagnosis(&diagnosis);
                }
                std::process::exit(1);
            }
        }
    }

    let mut command = std::process::Command::new(&plan.command);
    command
        .args(&plan.args)
        .current_dir(&context.project_root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    // Own process group: SIGTERM/SIGKILL reaches grandchildren (npm → node) and the
    // terminal's Ctrl-C does not need to reach the child — our handler forwards it.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    let child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let message = t_fmt_in(
                lang,
                "não foi possível executar '{}': {}",
                &[&display, &error.to_string()],
            );
            let diagnosis = diagnose(&message);
            if options.json {
                println!(
                    "{}",
                    json!({"schema": 1, "command": "run", "status": "failed", "error": message})
                );
            } else {
                println!("❌ {message}");
                print_diagnosis(&diagnosis);
            }
            std::process::exit(1);
        }
    };

    let stop = Arc::new(AtomicBool::new(false));
    {
        let flag = stop.clone();
        let _ = ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst));
    }

    if !options.json {
        println!("{}", t_fmt_in(lang, "\n▶ rodando: {}", &[&display]));
        println!(
            "{}\n",
            translate(lang, "   (Ctrl-C encerra: SIGTERM → espera → SIGKILL)")
        );
    }

    let supervise_options = SuperviseOptions {
        timeout: std::time::Duration::from_secs(timeout_secs),
        health,
        candidates: vec![3000, 4000, 5173, 8000, 8080],
        stream: !options.json,
        ..SuperviseOptions::default()
    };
    let outcome = supervise(child, &supervise_options, Some(stop))?;
    let seconds = outcome.duration_ms as f64 / 1000.0;

    match outcome.state {
        RunState::Exited if outcome.exit_code == Some(0) => {
            if options.json {
                println!("{}", json!({"schema": 1, "command": "run", "status": "exited", "exit_code": 0,
                                      "ready": outcome.ready, "port": outcome.port,
                                      "duration_ms": outcome.duration_ms}));
            } else {
                // No format specs in the catalog: compose the number outside.
                let note = readiness_note(&outcome);
                println!(
                    "{}",
                    t_fmt_in(
                        lang,
                        "\n✨ terminou normalmente em {}s{}",
                        &[&format!("{seconds:.1}"), &note]
                    )
                );
            }
        }
        RunState::Exited => {
            let combined = combine_output(&outcome.tail_stdout, &outcome.tail_stderr);
            // JSON prose is always English; human output follows the current
            // language (the diagnosis is composed once, in that language).
            let target = if options.json {
                devforge_shared::i18n::Lang::En
            } else {
                devforge_shared::i18n::lang()
            };
            let diagnosis = devforge_plugins::diagnose_in(target, &combined);
            if options.json {
                let causes: Vec<serde_json::Value> = diagnosis
                    .causes
                    .iter()
                    .map(|cause| {
                        json!({"id": cause.id,
                               "cause": devforge_shared::i18n::t_en(&cause.cause),
                               "fix": devforge_shared::i18n::t_en(&cause.fix)})
                    })
                    .collect();
                println!(
                    "{}",
                    json!({"schema": 1, "command": "run", "status": "failed",
                           "exit_code": outcome.exit_code, "duration_ms": outcome.duration_ms,
                           "summary": devforge_shared::i18n::t_en(&diagnosis.summary),
                           "causes": causes})
                );
            } else {
                // {:?} and {:.1} compose OUTSIDE the template (catalog rule).
                let code = format!("{:?}", outcome.exit_code);
                let note = readiness_note(&outcome);
                println!(
                    "{}",
                    t_fmt_in(
                        lang,
                        "\n❌ o projeto saiu com código {} após {}s{}",
                        &[&code, &format!("{seconds:.1}"), &note]
                    )
                );
                print_diagnosis(&diagnosis);
            }
            std::process::exit(outcome.exit_code.unwrap_or(1));
        }
        RunState::TimedOut => {
            if options.json {
                println!(
                    "{}",
                    json!({"schema": 1, "command": "run", "status": "timeout", "ready": false,
                           "duration_ms": outcome.duration_ms, "tail": outcome.tail_stderr})
                );
            } else {
                println!(
                    "{}",
                    t_fmt_in(
                        lang,
                        "\n⏱  não ficou \"pronto\" em {}s — processo derrubado.",
                        &[&timeout_secs.to_string()]
                    )
                );
                println!("{}", translate(lang, "   últimas linhas:"));
                let lines: Vec<&String> = outcome
                    .tail_stderr
                    .iter()
                    .chain(outcome.tail_stdout.iter())
                    .collect();
                for line in lines.iter().rev().take(8).rev() {
                    println!("     {line}");
                }
                println!(
                    "{}",
                    translate(
                        lang,
                        "   (prontidão = heurística de porta; use --health se o serviço é lento)"
                    )
                );
            }
            std::process::exit(1);
        }
        RunState::Stopped => {
            if options.json {
                println!(
                    "{}",
                    json!({"schema": 1, "command": "run", "status": "stopped", "ready": outcome.ready,
                           "port": outcome.port, "duration_ms": outcome.duration_ms})
                );
            } else {
                println!(
                    "{}",
                    t_fmt_in(
                        lang,
                        "\n■ interrompido{} em {}s",
                        &[&readiness_note(&outcome), &format!("{seconds:.1}")]
                    )
                );
            }
            std::process::exit(130);
        }
    }

    Ok(())
}

fn readiness_note(outcome: &SupervisedOutcome) -> String {
    use devforge_shared::i18n::t_fmt;
    match (outcome.ready, outcome.port) {
        (true, Some(port)) => t_fmt(" (pronto na porta {})", &[&port.to_string()]),
        _ => String::new(),
    }
}

fn combine_output(stdout: &[String], stderr: &[String]) -> String {
    format!("{}\n{}", stdout.join("\n"), stderr.join("\n"))
}

/// Interactive choice among ambiguous entrypoints — we never decide on our own.
fn pick_start_candidate(candidates: &[RunCandidate]) -> Option<RunPlan> {
    use devforge_shared::i18n::t;
    println!("{}", t("   Vários pontos de entrada — escolha um:"));
    for (index, item) in candidates.iter().enumerate() {
        // The list line has no translatable words (numbers + labels + command).
        println!(
            "   {}) {}  →  {} {}",
            index + 1,
            item.label,
            item.command,
            item.args.join(" ")
        );
    }
    loop {
        print!("{}", t("   número (vazio = cancelar): "));
        let _ = std::io::Write::flush(&mut std::io::stdout());

        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(0) => return None,
            Ok(_) => {}
            Err(_) => return None,
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Ok(number) = trimmed.parse::<usize>() {
            if let Some(item) = candidates.get(number.saturating_sub(1)) {
                return Some(RunPlan {
                    command: item.command.clone(),
                    args: item.args.clone(),
                    source: item.label.clone(),
                });
            }
        }
        println!("{}", t("   opção inválida."));
    }
}

/// `devforge license status|install|remove` — offline license (Ed25519).
/// 100% local verification: no network calls.
/// `devforge data status|update` — the Pro risk dataset.
///
/// `status` is always free and never touches the network; `update` is the
/// only network command and the only one gated by a valid license.
/// Refusals print their JSON line on stdout and exit 1 (design §4.2).
fn run_data(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use devforge_shared::i18n::{t, t_fmt};
    let mut as_json = false;
    let mut url: Option<String> = None;
    let mut positional: Vec<&String> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        if arg == "--json" {
            as_json = true;
        } else if arg == "--url" {
            index += 1;
            let value = args
                .get(index)
                .ok_or(t("--url exige um valor (<https://...> ou file://<caminho>)"))?;
            url = Some(value.clone());
        } else if arg.starts_with("--") {
            return Err(
                t_fmt(
                    "opção desconhecida: {} (uso: devforge data status|update [--json] [--url <url>])",
                    &[arg],
                )
                .into(),
            );
        } else {
            positional.push(&args[index]);
        }
        index += 1;
    }

    match positional.first().map(|s| s.as_str()).unwrap_or("status") {
        "status" => data_status(as_json),
        "update" => data_update(url, as_json),
        other => Err(t_fmt("subcomando desconhecido: {} (use: status | update)", &[other]).into()),
    }
}

/// `data status` — local, free, offline. Distinguishes absent / invalid /
/// installed (an installed-but-invalid dataset is never silently ignored).
fn data_status(as_json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use devforge_shared::i18n::{t, t_fmt, Lang};
    let file = dataset::dataset_file();
    match dataset::load_installed() {
        Ok(Some(ds)) => {
            let age = dataset::age_days(&ds.generated, &crate::today_iso()).unwrap_or(-1);
            let stale = age >= 0 && dataset::is_stale(age);
            if as_json {
                println!(
                    "{}",
                    json!({"schema": 1, "command": "data", "sub": "status",
                           "installed": true, "valid": true,
                           "file": file.display().to_string(),
                           "version": ds.version, "generated": ds.generated,
                           "age_days": age, "stale": stale,
                           "advisories": ds.advisories.len(),
                           "bad_names": ds.bad_names.len()})
                );
            } else {
                println!("{}", t("dataset instalado ✅"));
                println!(
                    "{}",
                    t_fmt(
                        "   versão:  {} (gerado em {})",
                        &[&ds.version.to_string(), &ds.generated.to_string()],
                    )
                );
                println!(
                    "{}",
                    t_fmt(
                        "   idade:   {} dia(s) — {} advisories na janela + {} na kill list",
                        &[
                            &age.to_string(),
                            &ds.advisories.len().to_string(),
                            &ds.bad_names.len().to_string(),
                        ],
                    )
                );
                println!("   path:    {}", file.display());
                if stale {
                    println!(
                        "\n{}",
                        t_fmt(
                            "⚠ atrasado (>{} dias) — rode: devforge data update",
                            &[&dataset::STALE_AFTER_DAYS.to_string()],
                        )
                    );
                } else {
                    println!("{}", t("\n(fresco)"));
                }
            }
            Ok(())
        }
        Ok(None) => {
            if as_json {
                println!(
                    "{}",
                    json!({"schema": 1, "command": "data", "sub": "status",
                           "installed": false,
                           "file": file.display().to_string(),
                           "tier": "free",
                           "hint": devforge_shared::i18n::t_en(
                               "audit segue offline com o motor embutido; dados frescos são Pro (devforge data update)"
                           )})
                );
            } else {
                println!(
                    "{}",
                    t_fmt(
                        "nenhum dataset instalado ({})",
                        &[&file.display().to_string()]
                    )
                );
                println!(
                    "{}",
                    t("   grátis: o audit usa o motor offline — nada muda no tier gratuito.")
                );
                println!(
                    "{}",
                    t("   Pro:    devforge data update (dados de risco frescos, exige licença).")
                );
            }
            Ok(())
        }
        Err(error) => {
            if as_json {
                println!(
                    "{}",
                    json!({"schema": 1, "command": "data", "sub": "status",
                           "installed": true, "valid": false,
                           "file": file.display().to_string(),
                           "error": error.message_in(Lang::En)})
                );
            } else {
                println!(
                    "{}",
                    t_fmt("dataset instalado, mas INVÁLIDO: {}", &[&error.to_string()])
                );
                println!(
                    "{}",
                    t("   nada é usado sem assinatura válida — rode devforge data update.")
                );
            }
            Ok(())
        }
    }
}

/// `data update` — license gate FIRST (no network for unlicensed installs),
/// then download → verify → atomic install. Refused = previous stays intact.
fn data_update(
    url: Option<String>,
    as_json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use devforge_shared::i18n::{t, t_fmt, translate, Lang};
    // Composed strings can land in JSON (`error`): build them in the target
    // language — JSON prose is always English (docs/design-i18n.md).
    let lang = if as_json {
        Lang::En
    } else {
        devforge_shared::i18n::lang()
    };
    let license_problem: Option<(&'static str, String)> = match license::load_installed() {
        None => Some((
            "no_license",
            translate(lang, "nenhuma licença instalada").to_string(),
        )),
        Some(token) => license::verify_token(&token)
            .err()
            .map(|error| ("license_invalid", error.message_in(lang))),
    };
    if let Some((status, error)) = license_problem {
        if as_json {
            println!(
                "{}",
                json!({"schema": 1, "command": "data", "sub": "update",
                       "status": status, "error": error})
            );
        } else {
            println!(
                "{}",
                t_fmt("⛔ Recurso Pro (dataset de risco fresco): {}.", &[&error])
            );
            println!(
                "{}",
                t("   Instale sua licença:  devforge license install <token>")
            );
            println!("{}", t("   (confira:             devforge license status)"));
            println!(
                "{}",
                t("   Sem licença o audit segue grátis e offline — o tier gratuito não muda.")
            );
        }
        std::process::exit(1);
    }

    let target = url.or_else(|| {
        std::env::var("DF_DATASET_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
    });
    let target = match target {
        Some(target) => target,
        None if !dataset::DEFAULT_DATASET_URL.is_empty() => {
            dataset::DEFAULT_DATASET_URL.to_string()
        }
        None => {
            return Err(
                t("URL do dataset não configurada nesta build (use --url <url> ou env DF_DATASET_URL)")
                    .into(),
            )
        }
    };

    if !as_json {
        println!("{}", t_fmt("baixando dataset de {} ...", &[&target]));
    }
    let bytes = fetch_dataset(&target, dataset::MAX_DATASET_BYTES)?;

    match dataset::install(&bytes) {
        Ok(ds) => {
            if as_json {
                println!(
                    "{}",
                    json!({"schema": 1, "command": "data", "sub": "update",
                           "status": "updated",
                           "version": ds.version, "generated": ds.generated,
                           "advisories": ds.advisories.len(),
                           "bad_names": ds.bad_names.len(),
                           "file": dataset::dataset_file().display().to_string()})
                );
            } else {
                println!("{}", t("dataset instalado ✅"));
                println!(
                    "{}",
                    t_fmt(
                        "   versão:  {} (gerado em {})",
                        &[&ds.version.to_string(), &ds.generated.to_string()],
                    )
                );
                println!(
                    "{}",
                    t_fmt(
                        "   dados:   {} advisories na janela + {} nomes na kill list",
                        &[
                            &ds.advisories.len().to_string(),
                            &ds.bad_names.len().to_string(),
                        ],
                    )
                );
                println!("   path:    {}", dataset::dataset_file().display());
                println!(
                    "{}",
                    t("\nO audit desta máquina já usa os dados frescos (offline até o próximo update).")
                );
            }
            Ok(())
        }
        Err(error) => {
            if as_json {
                println!(
                    "{}",
                    json!({"schema": 1, "command": "data", "sub": "update",
                           "status": "refused", "error": error.message_in(lang)})
                );
            } else {
                println!(
                    "{}",
                    t_fmt("❌ dataset recusado: {}", &[&error.to_string()])
                );
                println!(
                    "{}",
                    t("   (nada foi instalado — o dataset anterior, se houver, permanece)")
                );
            }
            std::process::exit(1);
        }
    }
}

/// Fetches the dataset bytes: `file://`/local for tests and air-gapped
/// mirrors, HTTPS for the Release. Size-capped (`MAX + 1` to detect overflow).
fn fetch_dataset(target: &str, max_bytes: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use devforge_shared::i18n::t_fmt;
    use std::io::Read;
    if let Some(path) = target.strip_prefix("file://") {
        return std::fs::read(path).map_err(|error| {
            t_fmt("não li o dataset local {}: {}", &[path, &error.to_string()]).into()
        });
    }
    if !target.starts_with("https://") && !target.starts_with("http://") {
        return Err(t_fmt(
            "origem não suportada (use https:// ou file://): {}",
            &[target],
        )
        .into());
    }
    let response = ureq::get(target)
        .timeout(std::time::Duration::from_secs(60))
        .call()
        .map_err(|error| t_fmt("falha ao baixar de {}: {}", &[target, &error.to_string()]))?;
    let mut buffer = Vec::new();
    response
        .into_reader()
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut buffer)
        .map_err(|error| t_fmt("falha ao ler a resposta de {}: {}", &[target, &error.to_string()]))?;
    if buffer.len() > max_bytes {
        return Err(
            t_fmt("resposta acima de {} bytes — recusada", &[&max_bytes.to_string()]).into(),
        );
    }
    Ok(buffer)
}

fn run_license(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use devforge_shared::i18n::{t, t_fmt, Lang};
    let as_json = args.iter().any(|a| a == "--json");
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let sub = positional.first().map(|s| s.as_str()).unwrap_or("status");

    match sub {
        "status" => {
            let file = license::license_file();
            match license::load_installed() {
                None => {
                    if as_json {
                        println!(
                            "{}",
                            json!({"schema": 1, "installed": false, "file": file.display().to_string()})
                        );
                    } else {
                        println!(
                            "{}",
                            t_fmt("nenhuma licença instalada ({})", &[&file.display().to_string()])
                        );
                    }
                }
                Some(token) => match license::verify_token(&token) {
                    Ok(p) => {
                        if as_json {
                            println!(
                                "{}",
                                json!({"schema": 1, "installed": true, "valid": true,
                                       "plan": p.plan, "holder": p.holder,
                                       "issued": p.issued, "expires": p.expires})
                            );
                        } else {
                            println!("{}", t("licença VÁLIDA"));
                            println!("{}", t_fmt("   plano:    {}", &[&p.plan]));
                            println!("{}", t_fmt("   titular:  {}", &[&p.holder]));
                            println!("{}", t_fmt("   emitida:  {}", &[&p.issued]));
                            println!("{}", t_fmt("   expira:   {}", &[&p.expires]));
                        }
                    }
                    Err(error) => {
                        if as_json {
                            println!(
                                "{}",
                                json!({"schema": 1, "installed": true, "valid": false,
                                       "error": error.message_in(Lang::En)})
                            );
                        } else {
                            println!(
                                "{}",
                                t_fmt("licença instalada, mas INVÁLIDA: {}", &[&error.to_string()])
                            );
                        }
                    }
                },
            }
        }

        "install" => {
            let arg = positional
                .get(1)
                .ok_or(t("uso: devforge license install <chave|@arquivo>"))?;
            let token = if let Some(path) = arg.strip_prefix('@') {
                std::fs::read_to_string(path)?
            } else {
                (*arg).clone()
            };
            let token = token.trim();

            // Verify BEFORE writing — junk never enters the vault.
            let payload = license::verify_token(token)
                .map_err(|error| t_fmt("licença rejeitada: {}", &[&error.to_string()]))?;
            let path = license::install(token)?;
            if as_json {
                println!(
                    "{}",
                    json!({"schema": 1, "installed": true, "valid": true, "file": path.display().to_string(),
                           "plan": payload.plan, "holder": payload.holder, "expires": payload.expires})
                );
            } else {
                println!(
                    "{}",
                    t_fmt(
                        "   ✓ licença instalada: {} (plano {}, expira {})",
                        &[&path.display().to_string(), &payload.plan, &payload.expires,],
                    )
                );
            }
        }

        "remove" => {
            let removed = license::remove();
            if as_json {
                println!("{}", json!({"schema": 1, "removed": removed}));
            } else if removed {
                println!("{}", t("   ✓ licença removida."));
            } else {
                println!("{}", t("   (não havia licença instalada)"));
            }
        }

        other => {
            return Err(
                t_fmt("subcomando desconhecido: {} (status|install|remove)", &[other]).into(),
            )
        }
    }

    Ok(())
}

/// Builds the SARIF 2.1.0 document from the report (read by GitHub Code
/// Scanning — annotates the PR inline). Line lookup is "lazy": only here, with
/// simple `contains` (no regex, no new dependency, read-only).
fn build_sarif(
    report: &AuditReport,
    stacks: &[ProjectStack],
    project_root: &Path,
) -> serde_json::Value {
    // package → manifest path (relative to the root).
    let mut manifest_of: HashMap<String, PathBuf> = HashMap::new();
    for stack in stacks {
        let relative = stack
            .manifest
            .strip_prefix(project_root)
            .unwrap_or(&stack.manifest)
            .to_path_buf();
        for dep in &stack.dependencies {
            manifest_of.insert(dep.name.clone(), relative.clone());
        }
    }

    // Rules (distinct factors) — for GitHub to group/deduplicate.
    let mut rules: Vec<serde_json::Value> = Vec::new();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for finding in &report.findings {
        for factor in &finding.factors {
            if seen.insert(factor.id()) {
                rules.push(json!({
                    "id": factor.id(),
                    "name": factor.id(),
                    "shortDescription": { "text": factor_label(factor) },
                }));
            }
        }
    }

    let results: Vec<serde_json::Value> = report
        .findings
        .iter()
        .map(|finding| {
            let manifest = manifest_of.get(&finding.package);
            let uri = manifest
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let start_line = manifest.and_then(|p| find_dep_line(&project_root.join(p), &finding.package));

            let mut location = json!({
                "physicalLocation": { "artifactLocation": { "uri": uri } }
            });
            if let Some(line) = start_line {
                location["physicalLocation"]["region"] = json!({ "startLine": line });
            }

            // SARIF is machine output: always English (docs/design-i18n.md).
            let message = devforge_shared::i18n::t_fmt_in(
                devforge_shared::i18n::Lang::En,
                "{} — score {} (confiança {}). Fatores: {}. {}",
                &[
                    &finding.package,
                    &finding.score.to_string(),
                    finding.confidence.as_str(),
                    &finding
                        .factors
                        .iter()
                        .map(|f| f.id())
                        .collect::<Vec<_>>()
                        .join(", "),
                    &finding.evidence.join(" | "),
                ],
            );

            json!({
                "ruleId": finding.factors.first().map(|f| f.id()).unwrap_or("supply-chain"),
                "level": sarif_level(finding.severity),
                "message": { "text": message },
                "locations": [ location ],
            })
        })
        .collect();

    json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "devforge",
                "version": env!("CARGO_PKG_VERSION"),
                "rules": rules,
            }},
            "results": results,
        }],
    })
}

fn sarif_level(severity: AuditSeverity) -> &'static str {
    match severity {
        AuditSeverity::Low => "note",
        AuditSeverity::Medium => "warning",
        AuditSeverity::High | AuditSeverity::Critical => "error",
    }
}

/// SARIF short descriptions: machine output → always English
/// (docs/design-i18n.md). Never used by human output.
fn factor_label(factor: &RiskFactor) -> &'static str {
    use devforge_shared::i18n::t_en;
    match factor {
        RiskFactor::InstallScript => t_en("Script de instalação (executa código no install)"),
        RiskFactor::NetworkAccess => t_en("Acesso à rede no install"),
        RiskFactor::ShellAccess => t_en("Acesso à shell no install"),
        RiskFactor::FilesystemAccess => t_en("Escrita em filesystem sensível no install"),
        RiskFactor::EnvAccess => t_en("Leitura de variáveis de ambiente no install"),
        RiskFactor::Obfuscation => t_en("Código ofuscado no install"),
        RiskFactor::NonRegistry => t_en("Dependência fora do registro (git/path/file)"),
        RiskFactor::LowDownloads => t_en("Pacote com baixa exposição (downloads)"),
        RiskFactor::Unpinned => t_en("Versão não fixada (range/coringa)"),
        RiskFactor::NoIntegrity => t_en("Sem hash de integridade"),
    }
}

/// Finds the line (1-based) of the dependency declaration in the manifest.
fn find_dep_line(path: &Path, package: &str) -> Option<u32> {
    let text = std::fs::read_to_string(path).ok()?;
    let ext = path.extension().and_then(|e| e.to_str());
    let is_json = ext == Some("json");
    let is_toml = ext == Some("toml");

    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        let json_key = format!("\"{package}\":");
        let toml_key = format!("{package} =");
        let hit = if is_json {
            trimmed.contains(&json_key)
        } else if is_toml {
            trimmed.starts_with(&toml_key)
        } else {
            trimmed.contains(&json_key) || trimmed.starts_with(&toml_key)
        };
        if hit {
            return Some(index as u32 + 1);
        }
    }
    None
}

/// Current date in ISO `YYYY-MM-DD` (no date dep; civil-days algorithm).
fn today_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil_from_days(secs.div_euclid(86_400))
}

/// Howard Hinnant's `civil_from_days` — converts days since epoch to a civil date.
fn civil_from_days(z: i64) -> String {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }
    format!("{year:04}-{month:02}-{day:02}")
}

fn print_findings(report: &AnalysisReport) {
    section("ANÁLISE DE DEPENDÊNCIAS");
    if report.is_empty() {
        println!("✅ Nada a reportar: dependências e lockfiles parecem saudáveis.");
        return;
    }

    for finding in report.sorted() {
        let icon = match finding.severity {
            Severity::Critical => "🛑",
            Severity::Warning => "⚠️ ",
            Severity::Info => "ℹ️ ",
        };
        println!("{icon} {}", finding.message);
    }

    println!(
        "\nResumo: {} crítico(s), {} aviso(s), {} informação(ões).",
        report.count(Severity::Critical),
        report.count(Severity::Warning),
        report.count(Severity::Info)
    );
}

/// Trust pre-flight: runs before `prepare`/`docker` so we don't install
/// code without the user knowing. Returns `false` if any risk was declined.
///
/// Each code execution risk requires **individual** confirmation — "yes
/// to all" and `--yes` do NOT apply here (protects the case of an AI driving).
fn pre_flight_trust(
    kernel: &Kernel,
    context: &Context,
    stacks: &[ProjectStack],
    gate: &dyn PermissionGate,
    require_confirmation: bool,
) -> bool {
    let Some(analyzer) = kernel.analyzer() else {
        return true;
    };
    let report = match analyzer.verify_trust(context, stacks) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("   (verificação de confiança falhou, seguindo): {error}");
            return true;
        }
    };

    if report.is_empty() {
        println!("\n✅ Verificação de confiança: nenhum risco detectado.");
        return true;
    }

    section("VERIFICAÇÃO DE CONFIANÇA (pré-instalação)");
    println!("O DevForge não confia cegamente no manifest. Riscos encontrados:\n");
    for risk in report.sorted() {
        let icon = match risk.severity {
            Severity::Critical => "🛑",
            Severity::Warning => "⚠️ ",
            Severity::Info => "ℹ️ ",
        };
        println!("{icon} {}", risk.message);
        if !risk.detail.trim().is_empty() {
            println!("   ↳ {}", risk.detail);
        }
    }

    let blocking = report.blocking();
    if blocking.is_empty() {
        println!("\n(Só avisos informativos — seguindo.)");
        return true;
    }

    if !require_confirmation {
        println!(
            "\n(--dry-run: nada será executado, então os riscos acima só foram exibidos.)"
        );
        return true;
    }

    println!(
        "\nOs {} risco(s) abaixo exigem confirmação EXPLÍCITA cada um \
         ('sim para todas' e --yes NÃO valem aqui).",
        blocking.len()
    );
    for risk in &blocking {
        let decision = gate.confirm(&PermissionRequest {
            title: format!("Aceitar risco: {}", risk.message),
            detail: risk.detail.clone(),
            reason: risk.message.clone(),
            critical: true,
        });
        if decision == Decision::Denied {
            return false;
        }
    }
    true
}

/// Tools missing from the PATH (local mode only; in Docker the toolchain lives
/// in the container, so we don't check here).
fn local_missing_tools(
    kernel: &Kernel,
    mode: PrepareMode,
    context: &Context,
    stacks: &[ProjectStack],
) -> Result<Vec<MissingTool>, Box<dyn std::error::Error>> {
    if mode != PrepareMode::Local {
        return Ok(Vec::new());
    }
    let Some(runner) = kernel.command_runner() else {
        return Ok(Vec::new());
    };
    Ok(missing_tools(runner, stacks, &context.project_root)?)
}

fn print_missing_tools(missing: &[MissingTool]) {
    section("FERRAMENTAS FALTANDO");
    println!("Estas ferramentas não estão no PATH e serão necessárias pra rodar o projeto:\n");
    for m in missing {
        println!("  • {} ({})", m.tool, m.language);
        println!("      sugestão: {}", m.hint);
    }
    println!("\nInstale manualmente e rode de novo. As sugestões são por gerenciador");
    println!("de pacotes — sua máquina pode variar.");
}

/// Phase 2: offers to install, one by one, the missing tools that have a
/// **curated** command for the current OS. Each requires **critical** confirmation (neither
/// "yes to all" nor `--yes` cover it). Runs with inherited stdio, so a
/// `sudo`/`brew` that asks for a password works right away. Tools without a
/// curated command stay as suggestion only ([`print_missing_tools`]).
fn offer_toolchain_install(
    kernel: &Kernel,
    context: &Context,
    gate: &dyn PermissionGate,
    missing: &[MissingTool],
) {
    let Some(runner) = kernel.command_runner() else {
        return;
    };
    // Only offers what has a curated command for the current OS; the rest stays a suggestion.
    let plans: Vec<(&MissingTool, CommandSpec)> = missing
        .iter()
        .filter_map(|entry| {
            let (program, args) = install_command(&entry.tool)?;
            Some((entry, CommandSpec::new(program, args, &context.project_root)))
        })
        .collect();
    if plans.is_empty() {
        return;
    }

    section("INSTALAR TOOLCHAIN (com sua permissão)");
    for (entry, spec) in plans {
        let decision = gate.confirm(&PermissionRequest {
            title: format!("Instalar {} ({})", entry.tool, entry.language),
            detail: format!(
                "Vou rodar (sem shell, argumento a argumento):\n  {}\n\
                 Isso altera seu sistema e pode pedir sua senha. Recuse se não confiar.",
                spec.display()
            ),
            reason: format!("{} não está no PATH", entry.tool),
            critical: true,
        });
        if decision != Decision::Approved {
            println!(
                "   {} — deixado de lado (você optou por não instalar).",
                entry.tool
            );
            continue;
        }
        println!("\n▶ Instalando {} — siga o prompt se ele pedir senha...", entry.tool);
        match runner.run_interactive(&spec) {
            Ok(Some(0)) | Ok(None) => println!("   ✓ {} instalado.", entry.tool),
            Ok(Some(code)) => eprintln!("   ✗ '{}' terminou com exit {code}.", spec.display()),
            Err(error) => eprintln!("   ✗ não consegui rodar: {error}"),
        }
    }
}

/// `devforge up` — composed pipeline: [secure clone?] → audit → prepare → run.
///
/// Every step is literally the isolated command (same code, same gates, same
/// output); composing never weakens security. A [`PrepareOutcome::Aborted`]
/// (trust risk / missing toolchain / failed install) stops the pipeline
/// BEFORE `run` — never execute a project after an abort.
fn run_up(
    url: Option<&str>,
    kernel: &Kernel,
    context: &Context,
    options: &Options,
) -> Result<(), Box<dyn std::error::Error>> {
    if url.is_none() && !context.project_root.exists() {
        return Err(format!("caminho não existe: {}", context.project_root.display()).into());
    }
    let target = match url {
        Some(url) => url.to_string(),
        None => context.project_root.display().to_string(),
    };
    let skip: Vec<&str> = options
        .skip
        .as_deref()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let plan = up::up_plan(url.is_some(), options.until.as_deref(), &skip)?;

    // Everything the pipeline could run (for reporting; flags cut this list).
    let mut all_steps = vec![UpStep::Audit, UpStep::Prepare, UpStep::Run];
    if url.is_some() {
        all_steps.insert(0, UpStep::Clone);
    }

    if options.dry_run {
        if options.json {
            let steps: Vec<serde_json::Value> = all_steps
                .iter()
                .map(|step| {
                    json!({"name": step.name(), "status": if plan.contains(step) { "planned" } else { "skipped" }})
                })
                .collect();
            println!(
                "{}",
                json!({"schema": 1, "command": "up", "status": "planned",
                       "target": target, "steps": steps})
            );
        } else {
            section("PLANO DO UP");
            println!("Alvo: {target}\n");
            for (index, step) in all_steps.iter().enumerate() {
                let suffix = if plan.contains(step) {
                    ""
                } else {
                    "   (pulado por flag)"
                };
                println!("{:>2}. {} — {}{}", index + 1, step.name(), step.description(), suffix);
            }
            println!("\n(--dry-run: nada foi executado.)");
        }
        return Ok(());
    }

    let mut statuses: Vec<(&'static str, &'static str)> = Vec::new();
    let mut active_context = context.clone();
    let mut cloned_kernel: Option<Kernel> = None;

    for (position, step) in plan.iter().enumerate() {
        if !options.json {
            section(&format!(
                "UP — ETAPA {} DE {}: {}",
                position + 1,
                plan.len(),
                step.name().to_uppercase()
            ));
        }
        match step {
            UpStep::Clone => {
                let url = url.ok_or("clone sem URL (inconsistência interna do plano)")?;
                let destination = env::current_dir()?.join(up::repo_name(url)?);
                if let Err(error) = clone_repo(url, &destination) {
                    let diagnosis = diagnose(&error);
                    if options.json {
                        println!(
                            "{}",
                            json!({"schema": 1, "command": "up", "status": "failed",
                                   "step": "clone", "error": error})
                        );
                    } else {
                        println!("❌ {error}");
                        print_diagnosis(&diagnosis);
                    }
                    std::process::exit(1);
                }
                active_context = Context {
                    project_root: destination,
                };
                if !options.json {
                    println!("✅ clonado em {}", active_context.project_root.display());
                }
                cloned_kernel = Some(build_kernel(&active_context, options.all)?);
                statuses.push(("clone", "ok"));
            }
            UpStep::Audit => {
                run_audit(cloned_kernel.as_ref().unwrap_or(kernel), &active_context, options)?;
                statuses.push(("audit", "ok"));
            }
            UpStep::Prepare => {
                let outcome = run_prepare(
                    cloned_kernel.as_ref().unwrap_or(kernel),
                    &active_context,
                    PrepareMode::Local,
                    options,
                )?;
                if outcome == PrepareOutcome::Aborted {
                    // Abort (trust/toolchain/install): NEVER reach `run`.
                    statuses.push(("prepare", "aborted"));
                    emit_up_summary(options, &all_steps, &statuses, "aborted", 1);
                    std::process::exit(1);
                }
                statuses.push(("prepare", "ok"));
            }
            UpStep::Run => {
                run_run(cloned_kernel.as_ref().unwrap_or(kernel), &active_context, options)?;
                statuses.push(("run", "ok"));
            }
        }
    }

    emit_up_summary(options, &all_steps, &statuses, "ok", 0);
    Ok(())
}

/// Final report: one JSON line (JSONL) or a human section. Steps cut by
/// `--until`/`--skip` are reported as `skipped`.
fn emit_up_summary(
    options: &Options,
    all_steps: &[UpStep],
    statuses: &[(&'static str, &'static str)],
    status: &str,
    exit_code: i32,
) {
    if options.json {
        let steps: Vec<serde_json::Value> = all_steps
            .iter()
            .map(|step| {
                let state = statuses
                    .iter()
                    .find(|(name, _)| *name == step.name())
                    .map(|(_, state)| *state)
                    .unwrap_or("skipped");
                json!({"name": step.name(), "status": state})
            })
            .collect();
        println!(
            "{}",
            json!({"schema": 1, "command": "up", "status": status,
                   "steps": steps, "exit_code": exit_code})
        );
    } else {
        section("RESULTADO DO UP");
        for step in all_steps {
            match statuses.iter().find(|(name, _)| *name == step.name()) {
                Some((_, "ok")) => println!("✅ {}", step.name()),
                Some((_, state)) => println!("⏹️  {} ({state})", step.name()),
                None => println!("⏭️  {} (pulado)", step.name()),
            }
        }
        if status == "ok" {
            println!("\n✨ Pipeline concluído.");
        } else {
            println!("\n⏹️ Pipeline interrompido — nada além do ponto de aborto foi executado.");
        }
    }
}

/// Secure shallow clone: argv only (no shell), no submodules, `--depth 1`,
/// defense-in-depth against template hooks (`core.hooksPath=/dev/null` —
/// hooks/config don't travel with a clone anyway; submodules/LFS would).
/// git's progress goes to stderr, so `--json` stdout stays clean JSONL.
fn clone_repo(url: &str, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err(format!(
            "o destino já existe: {} — nada foi clonado (remova ou use outro alvo)",
            destination.display()
        ));
    }
    let status = std::process::Command::new("git")
        .args(["-c", "core.hooksPath=/dev/null", "clone", "--depth", "1", url])
        .arg(destination)
        .status()
        .map_err(|error| format!("não consegui executar git: {error}"))?;
    if !status.success() {
        return Err(format!("git clone falhou ({status})"));
    }
    Ok(())
}

/// Whether `prepare` finished its job. `Aborted` means the pipeline must
/// STOP: trust risk, missing toolchain after refusal, or failed install —
/// never continue to `run` in any of those cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrepareOutcome {
    Completed,
    Aborted,
}

fn run_prepare(
    kernel: &Kernel,
    context: &Context,
    mode: PrepareMode,
    options: &Options,
) -> Result<PrepareOutcome, Box<dyn std::error::Error>> {
    let stacks = kernel.detect_stacks(context)?;

    // `--json` mode: delivers the plan + trust check + toolchain
    // structured and does NOT execute (execution is interactive/human). stdout = pure JSON.
    if options.json {
        let trust = match kernel.analyzer() {
            Some(analyzer) => analyzer.verify_trust(context, &stacks)?,
            None => TrustReport::default(),
        };
        let plan = Provisioner::build_plan(mode, &stacks, &context.project_root);
        let missing_tools = local_missing_tools(kernel, mode, context, &stacks)?;
        output::prepare_plan(
            context,
            command_name(mode),
            mode_str(mode),
            &stacks,
            &trust,
            &plan,
            &missing_tools,
        );
        // `--json` mode delivers the plan only (no execution happens here).
        return Ok(PrepareOutcome::Completed);
    }

    if stacks.is_empty() {
        println!("Nenhum stack reconhecido — nada a preparar.");
        return Ok(PrepareOutcome::Completed);
    }
    print_stacks(&stacks);

    let auto = AutoApprovePermissionGate;
    let interactive = InteractivePermissionGate::new();
    let gate: &dyn PermissionGate = if options.yes { &auto } else { &interactive };

    // Toolchain: in local mode, checks what is missing from the PATH (Phase 1). If
    // possible, offers to install with explicit permission (Phase 2, curated). Without
    // the toolchain prepare cannot run — but with --dry-run we still show the plan.
    if mode == PrepareMode::Local {
        let missing = local_missing_tools(kernel, mode, context, &stacks)?;
        if !missing.is_empty() {
            if options.dry_run {
                print_missing_tools(&missing);
            } else {
                offer_toolchain_install(kernel, context, gate, &missing);
                // Re-checks honestly: the user may have declined or the
                // installation may have failed.
                let still = local_missing_tools(kernel, mode, context, &stacks)?;
                if !still.is_empty() {
                    print_missing_tools(&still);
                    // Without the toolchain prepare cannot run: abort (the `up`
                    // pipeline must not continue to `run` unprepared).
                    return Ok(PrepareOutcome::Aborted);
                }
                println!("\n✅ Toolchain instalada — seguindo.");
            }
        }
    }

    // Trust pre-flight: does NOT install/build anything before checking for
    // arbitrary code scripts and packages that don't match the registry.
    if !pre_flight_trust(kernel, context, &stacks, gate, !options.dry_run) {
        println!("\n⏹️  Abortado por risco de segurança. Nada foi instalado nem buildado.");
        // Security abort: the `up` pipeline stops here — never reach `run`.
        return Ok(PrepareOutcome::Aborted);
    }

    let plan = Provisioner::build_plan(mode, &stacks, &context.project_root);

    section(&format!("PLANO ({})", mode_label(mode)));
    for (index, step) in plan.steps.iter().enumerate() {
        println!("{:>2}. {}", index + 1, step.title);
        println!("      {}", step.detail);
    }
    if !plan.run_hints.is_empty() {
        println!("\nPara rodar depois de preparado:");
        for hint in &plan.run_hints {
            println!("   $ {hint}");
        }
    }

    if options.dry_run {
        println!("\n(--dry-run: nada foi executado.)");
        return Ok(PrepareOutcome::Completed);
    }

    let files = kernel
        .file_system()
        .ok_or("plugin 'file_system_access' não registrado")?;
    let runner = kernel
        .command_runner()
        .ok_or("plugin 'command_runner' não registrado")?;

    section(if options.yes {
        "EXECUÇÃO (aprovação automática)"
    } else {
        "EXECUÇÃO (você será perguntado a cada passo)"
    });

    let execute_options = ExecuteOptions {
        dry_run: false,
        force: options.force,
    };
    let report = Provisioner.execute(&plan, runner, files, gate, &execute_options)?;
    print_execution(&report);
    // A failed execution is NOT a completed prepare: the `up` pipeline stops
    // instead of running a project that wasn't prepared.
    if report.failed.is_some() {
        return Ok(PrepareOutcome::Aborted);
    }
    Ok(PrepareOutcome::Completed)
}

fn print_stacks(stacks: &[ProjectStack]) {
    section("STACKS DETECTADOS");
    if stacks.is_empty() {
        println!("Nenhum manifesto reconhecido neste diretório.");
        return;
    }
    for stack in stacks {
        println!(
            "\n• Linguagem : {}   |   Gerenciador: {}   |   Nome: {}",
            stack.language,
            stack.package_manager.as_deref().unwrap_or("-"),
            stack.name.as_deref().unwrap_or("-")
        );
        println!("  Manifesto : {}", stack.manifest.display());
        if !stack.frameworks.is_empty() {
            println!("  Frameworks: {}", stack.frameworks.join(", "));
        }
        println!("  Dependências: {}", stack.dependencies.len());
    }
}

fn print_execution(report: &PrepareReport) {
    section("RESULTADO");
    for title in &report.executed {
        println!("✅ {title}");
    }
    for title in &report.skipped {
        println!("⏭️  {title}");
    }
    if let Some(error) = &report.failed {
        println!("\n❌ A execução parou: {error}");
        println!("   Corrija o problema e rode novamente.");
    } else if report.executed.is_empty() && report.skipped.is_empty() {
        println!("Nada foi executado.");
    } else {
        println!("\n✨ Concluído: {} passo(s) executado(s), {} pulado(s).", report.executed.len(), report.skipped.len());
    }
}

fn mode_label(mode: PrepareMode) -> &'static str {
    match mode {
        PrepareMode::Local => "sem Docker",
        PrepareMode::Docker => "com Docker",
    }
}

/// Command name for the JSON (`prepare`/`docker`).
fn command_name(mode: PrepareMode) -> &'static str {
    match mode {
        PrepareMode::Local => "prepare",
        PrepareMode::Docker => "docker",
    }
}

/// Stable code mode for the JSON (`local`/`docker`).
fn mode_str(mode: PrepareMode) -> &'static str {
    match mode {
        PrepareMode::Local => "local",
        PrepareMode::Docker => "docker",
    }
}

#[derive(Default)]
struct Options {
    dry_run: bool,
    yes: bool,
    force: bool,
    online: bool,
    json: bool,
    all: bool,
    /// (`diagnose`) inline error message.
    message: Option<String>,
    /// (`diagnose`) file with the error output.
    input: Option<PathBuf>,
    /// (`audit`) minimum severity that fails the build (`low|medium|high|critical`).
    fail_on: Option<String>,
    /// (`audit`) policy file (TOML).
    policy: Option<PathBuf>,
    /// (`audit`) suppressions/expiration file (TOML).
    ignore: Option<PathBuf>,
    /// (`audit`) emits results in SARIF 2.1.0 (GitHub Code Scanning).
    sarif: bool,
    /// (`run`) seconds spent waiting for the project to become ready (readiness).
    timeout: Option<u64>,
    /// (`run`) explicit health check: `tcp:<port>` | `http://...`.
    health: Option<String>,
    /// (`run`) arguments after `--`, passed through to the start command.
    extra_args: Vec<String>,
    /// (`run`) start in the background and return the PID immediately — no
    /// readiness supervision; the log goes to `.devforge/run.log`.
    detach: bool,
    /// (`up`) stop after this step (`clone|audit|prepare|run`).
    until: Option<String>,
    /// (`up`) comma-separated steps to skip (`clone,audit,prepare,run`).
    skip: Option<String>,
}

/// Flags of `parse_options` whose NEXT argument is a value — kept in sync
/// with the match there, so `--lang` extraction never eats a value by
/// mistake (e.g. `--message "--lang"`).
const VALUE_FLAGS: &[&str] = &[
    "--message",
    "--input",
    "--fail-on",
    "--policy",
    "--ignore",
    "--timeout",
    "--health",
    "--until",
    "--skip",
    "--url",
];

/// Pulls `--lang <pt-BR|en>` (or `--lang=...`) out of `args` BEFORE dispatch,
/// so every parser honors it — including `license`/`data`, which parse their
/// own flags. Stops at `--`: what follows belongs to the child command
/// (e.g. `run . -- --lang ...` passes it through untouched).
fn extract_lang(args: &mut Vec<String>) -> Result<Option<String>, String> {
    let mut lang: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].clone();
        if arg == "--" {
            break;
        }
        if VALUE_FLAGS.contains(&arg.as_str()) {
            index += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--lang=") {
            lang = Some(value.to_string());
            args.remove(index);
            continue;
        }
        if arg == "--lang" {
            let value = args
                .get(index + 1)
                .ok_or("--lang exige um valor (pt-BR | en)")?
                .clone();
            lang = Some(value);
            args.drain(index..=index + 1);
            continue;
        }
        index += 1;
    }
    Ok(lang)
}

fn parse_options(args: &[String]) -> Result<(Option<PathBuf>, Options), String> {
    let mut path = None;
    let mut options = Options::default();
    let mut i = 0;

    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--dry-run" => options.dry_run = true,
            "--yes" | "-y" => options.yes = true,
            "--force" => options.force = true,
            "--online" => options.online = true,
            "--json" => options.json = true,
            "--detach" => options.detach = true,
            "--all" => options.all = true,
            "--sarif" => options.sarif = true,
            // Flags that consume the next argument as their value.
            "--message" | "--input" | "--fail-on" | "--policy" | "--ignore" | "--timeout"
            | "--health" | "--until" | "--skip" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| format!("{arg} exige um valor"))?;
                match arg {
                    "--message" => options.message = Some(value.clone()),
                    "--input" => options.input = Some(PathBuf::from(value)),
                    "--fail-on" => options.fail_on = Some(value.clone()),
                    "--policy" => options.policy = Some(PathBuf::from(value)),
                    "--ignore" => options.ignore = Some(PathBuf::from(value)),
                    "--timeout" => {
                        options.timeout = Some(
                            value
                                .parse()
                                .map_err(|_| format!("--timeout exige um número de segundos: {value}"))?,
                        );
                    }
                    "--health" => options.health = Some(value.clone()),
                    "--until" => options.until = Some(value.clone()),
                    "--skip" => options.skip = Some(value.clone()),
                    _ => unreachable!("flag de valor tratada acima"),
                }
                i += 1;
            }
            // `--` separates flags from the arguments passed through to the command (run).
            "--" => {
                options.extra_args.extend(args[i + 1..].iter().cloned());
                break;
            }
            other if other.starts_with('-') => {
                return Err(format!("opção desconhecida: {other}"));
            }
            other => {
                if path.is_some() {
                    return Err(format!("caminho informado mais de uma vez: {other}"));
                }
                path = Some(PathBuf::from(other));
            }
        }
        i += 1;
    }

    Ok((path, options))
}

fn print_help() {
    use devforge_shared::i18n::{t, HELP_PT};
    println!("{}", t(HELP_PT));
}

fn banner(title: &str, project_root: &std::path::Path) {
    use devforge_shared::i18n::t_fmt;
    println!("=============================================");
    println!("✨ {title} ✨");
    println!(
        "{}",
        t_fmt("   projeto: {}", &[&project_root.display().to_string()])
    );
    println!("=============================================");
}

fn section(title: &str) {
    println!("\n---------------------------------------------");
    println!("{}", devforge_shared::i18n::t(title));
    println!("---------------------------------------------");
}
