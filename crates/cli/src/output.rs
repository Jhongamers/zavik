//! Structured output (`--json`) for AI orchestration.
//!
//! Reason for existing: when the one calling the CLI is an **agent**, text for
//! humans (emoji, sentences) forces the model to "interpret", spending tokens. A
//! compact and stable JSON delivers the fact ready-made. That is why the output here is
//! **compact (single line)** and goes only to stdout — logs stay on stderr.
//!
//! Contract: every document has `"schema": 1`. If we break compatibility,
//! we bump that number.

use devforge_core::provision::PreparePlan;
use devforge_core::toolchain::{MissingTool, ToolStatus};
use devforge_shared::i18n::t_en;
use devforge_shared::{
    AnalysisReport, AuditReport, Briefing, Context, Diagnosis, ProjectStack, Severity, TrustReport,
};
use serde_json::{json, Value};

/// Severity in stable code (English) for machine consumption.
fn severity_str(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "critical",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

fn stack_to_json(stack: &ProjectStack) -> Value {
    json!({
        "language": stack.language.to_string(),
        "package_manager": stack.package_manager,
        "name": stack.name,
        "manifest": stack.manifest.display().to_string(),
        "frameworks": stack.frameworks,
        "dependencies": stack.dependencies.iter().map(|d| json!({
            "name": d.name,
            "version": d.version,
            "kind": format!("{:?}", d.kind),
        })).collect::<Vec<_>>(),
    })
}

/// Emits the JSON document on stdout (compact, single line).
fn emit(value: &Value) {
    // `to_string` (not pretty) on purpose: fewer tokens for the agent.
    println!("{value}");
}

/// `devforge detect --json`
pub fn detect(context: &Context, stacks: &[ProjectStack]) {
    emit(&json!({
        "schema": 1,
        "command": "detect",
        "root": context.project_root.display().to_string(),
        "stacks": stacks.iter().map(stack_to_json).collect::<Vec<_>>(),
    }));
}

/// `devforge analyze [--online] --json`
pub fn analyze(context: &Context, stacks: &[ProjectStack], report: &AnalysisReport) {
    emit(&json!({
        "schema": 1,
        "command": "analyze",
        "root": context.project_root.display().to_string(),
        "stacks": stacks.iter().map(stack_to_json).collect::<Vec<_>>(),
        "findings": report.findings.iter().map(|f| json!({
            "severity": severity_str(f.severity),
            "package": f.package,
            "message": f.message,
        })).collect::<Vec<_>>(),
        "summary": {
            "critical": report.count(Severity::Critical),
            "warning": report.count(Severity::Warning),
            "info": report.count(Severity::Info),
        },
    }));
}

/// `devforge context --json` — full briefing in a single call: stack,
/// trust, runbook (setup/build/test/run/lint), entrypoints and key files.
/// Made for an AI to "understand" the project in one shot, without exploration
/// (glob/grep/reading the README), saving tokens.
pub fn context(
    context: &Context,
    stacks: &[ProjectStack],
    trust: &TrustReport,
    briefing: &Briefing,
) {
    let rb = &briefing.runbook;
    emit(&json!({
        "schema": 1,
        "command": "context",
        "root": context.project_root.display().to_string(),
        "stacks": stacks.iter().map(stack_to_json).collect::<Vec<_>>(),
        "trust": {
            "risks": trust.risks.iter().map(|r| json!({
                "severity": severity_str(r.severity),
                "package": r.package,
                "message": r.message,
            })).collect::<Vec<_>>(),
            "blocking": trust.blocking().len(),
        },
        "runbook": {
            "setup": rb.setup,
            "build": rb.build,
            "test": rb.test,
            "run": rb.run,
            "lint": rb.lint,
        },
        "entrypoints": briefing.entrypoints,
        "key_files": briefing.key_files.iter().map(|k| json!({
            "kind": k.kind,
            "path": k.path,
        })).collect::<Vec<_>>(),
    }));
}

/// `devforge diagnose --json` — probable cause + fix for an error, so the AI
/// does not have to read a giant stack trace (nor guess that the literal error is
/// just a symptom).
pub fn diagnose(diagnosis: &Diagnosis) {
    emit(&json!({
        "schema": 1,
        "command": "diagnose",
        // Prose inside JSON is always English (t_en) — independent of the
        // language the diagnosis was built with (docs/design-i18n.md).
        // `id` and `evidence`/`top_errors` are machine facts: never touched.
        "summary": t_en(&diagnosis.summary),
        "causes": diagnosis.causes.iter().map(|c| json!({
            "id": c.id,
            "cause": t_en(&c.cause),
            "fix": t_en(&c.fix),
            "note": c.note.as_deref().map(t_en),
            "evidence": c.evidence,
        })).collect::<Vec<_>>(),
        "top_errors": diagnosis.top_errors,
    }));
}

/// `devforge audit --json` — supply-chain report (schema 1). One finding
/// per package, with factors, score, confidence and the evidence; plus the CI verdict.
///
/// `dataset` = Pro: when a valid signed local dataset exists, its advisory
/// section is appended (`dataset` + `dataset_findings`). Absent (free tier)
/// → keys are omitted entirely: output stays byte-identical (design §6).
pub fn audit(report: &AuditReport, dataset: Option<&crate::dataset::DatasetReport>) {
    let suppression_json = |s: &devforge_shared::Suppression| {
        json!({
            "package": s.package,
            "factor": s.factor.id(),
            "reason": s.reason,
            "expires": s.expires,
        })
    };
    let mut value = json!({
        "schema": 1,
        "command": "audit",
        "score": report.score,
        "passed": report.passed,
        "fail_on": report.fail_on.as_str(),
        "packages_audited": report.packages_audited,
        "findings": report.findings.iter().map(|f| json!({
            "package": f.package,
            "severity": f.severity.as_str(),
            "score": f.score,
            "confidence": f.confidence.as_str(),
            "factors": f.factors.iter().map(|x| x.id()).collect::<Vec<_>>(),
            "evidence": f.evidence,
        })).collect::<Vec<_>>(),
        "suppressed": report.suppressed.iter().map(suppression_json).collect::<Vec<_>>(),
        "expired_suppressions": report.expired_suppressions.iter().map(suppression_json).collect::<Vec<_>>(),
    });
    if let Some(dataset) = dataset {
        value["dataset"] = json!({
            "version": dataset.version,
            "generated": dataset.generated,
            "age_days": dataset.age_days,
            "stale": dataset.stale,
            "advisories": dataset.advisories_total,
            "bad_names": dataset.bad_names_total,
        });
        value["dataset_findings"] = json!(
            dataset.findings.iter().map(|f| json!({
                "package": f.package,
                "ecosystem": f.ecosystem,
                "kind": f.kind,
                "severity": f.severity,
                "id": f.id,
                "detail": f.detail,
                "url": f.url,
                "source": "dataset",
            })).collect::<Vec<_>>()
        );
    }
    emit(&value);
}

/// `devforge prepare|docker --json` — delivers the **plan**, the **trust
/// check** and the **missing tools** structured, without executing anything
/// (execution is human/interactive).
pub fn prepare_plan(
    context: &Context,
    command: &str,
    mode: &str,
    stacks: &[ProjectStack],
    trust: &TrustReport,
    plan: &PreparePlan,
    missing_tools: &[MissingTool],
) {
    emit(&json!({
        "schema": 1,
        "command": command,
        "mode": mode,
        "root": context.project_root.display().to_string(),
        "stacks": stacks.iter().map(stack_to_json).collect::<Vec<_>>(),
        "toolchain": {
            "missing": missing_tools.iter().map(|m| json!({
                "tool": m.tool,
                "language": m.language.to_string(),
                "hint": m.hint,
            })).collect::<Vec<_>>(),
        },
        "trust": {
            "risks": trust.risks.iter().map(|r| json!({
                "severity": severity_str(r.severity),
                "package": r.package,
                "message": r.message,
                "detail": r.detail,
            })).collect::<Vec<_>>(),
            "blocking": trust.blocking().len(),
        },
        "plan": {
            "steps": plan.steps.iter().map(|s| json!({
                "title": s.title,
                "detail": s.detail,
                "reason": s.reason,
                "requires_permission": s.requires_permission,
            })).collect::<Vec<_>>(),
            "run_hints": plan.run_hints,
        },
    }));
}

/// `devforge env --json`: environment fingerprint for agents — OS/arch plus
/// each stack's tools with presence + version, and a flat `missing` list.
pub fn env(stacks: &[ProjectStack], tools: &[ToolStatus]) {
    let stacks_json: Vec<Value> = stacks
        .iter()
        .filter_map(|stack| {
            let lang_tools: Vec<Value> = tools
                .iter()
                .filter(|tool| tool.language == stack.language)
                .map(|tool| {
                    json!({
                        "name": tool.tool,
                        "present": tool.present,
                        "version": tool.version,
                    })
                })
                .collect();
            (!lang_tools.is_empty()).then(|| {
                json!({
                    "language": stack.language.to_string(),
                    "tools": lang_tools,
                })
            })
        })
        .collect();
    let missing: Vec<&str> = tools
        .iter()
        .filter(|tool| !tool.present)
        .map(|tool| tool.tool.as_str())
        .collect();
    println!(
        "{}",
        json!({
            "schema": 1,
            "command": "env",
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "stacks": stacks_json,
            "missing": missing,
        })
    );
}
