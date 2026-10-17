//! Integration tests for `devforge audit` on real project shapes — born
//! from dogfooding DevForge with itself (docs/dogfooding.md):
//!
//! - a virtual workspace root must audit its **members' dependencies**
//!   (used to see 0 packages and approve nothing);
//! - 0 packages = "SEM DADOS" in human output, never a confident
//!   "APROVADO" (anti-false-positive), while the machine contract
//!   (`packages_audited`, `passed`, exit code) stays untouched.

#![cfg(unix)]

use std::process::Command;
use tempfile::TempDir;

fn devforge() -> Command {
    Command::new(env!("CARGO_BIN_EXE_devforge"))
}

fn workspace_fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.dependencies]\nserde = \"1.0.0\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("crates/app")).unwrap();
    std::fs::write(
        dir.path().join("crates/app/Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = { workspace = true }\n",
    )
    .unwrap();
    dir
}

#[test]
fn workspace_root_audits_member_dependencies_not_zero() {
    let dir = workspace_fixture();
    let output = devforge()
        .args(["audit", "--json"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "audit sai 0 sem --fail-on");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON puro");
    assert_eq!(json["command"], "audit");
    assert!(
        json["packages_audited"].as_u64().unwrap_or(0) >= 1,
        "raiz de workspace audita os membros: {json}"
    );
}

#[test]
fn workspace_root_human_audit_reports_real_packages() {
    let dir = workspace_fixture();
    let output = devforge().arg("audit").arg(dir.path()).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Pacotes auditados: 1"),
        "audit real na raiz do workspace: {stdout}"
    );
    assert!(!stdout.contains("SEM DADOS"), "não é mais um audit vazio");
}

#[test]
fn zero_packages_is_no_data_in_human_and_zero_in_json() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name":"sem-deps","version":"1.0.0"}"#,
    )
    .unwrap();

    let human = devforge().arg("audit").arg(dir.path()).output().unwrap();
    assert!(human.status.success());
    let stdout = String::from_utf8_lossy(&human.stdout);
    assert!(
        stdout.contains("0 pacotes auditados"),
        "avisa que auditou nada: {stdout}"
    );
    assert!(
        stdout.contains("SEM DADOS") && !stdout.contains("APROVADO"),
        "zero pacotes nunca vira aprovação: {stdout}"
    );

    // Machine contract untouched: JSON still reports the numbers as-is.
    let json_out = devforge()
        .args(["audit", "--json"])
        .arg(dir.path())
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&json_out.stdout).expect("JSON puro");
    assert_eq!(json["packages_audited"], 0);
    assert!(json["passed"].is_boolean(), "passed segue booleano (contrato)");
}

#[test]
fn audit_human_output_translates_to_english() {
    // i18n phase 2: the wedge command body must follow `--lang`.
    let dir = workspace_fixture();
    let output = devforge()
        .args(["audit", "--lang", "en"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Packages audited: 1"),
        "corpo do audit em EN: {stdout}"
    );
    assert!(
        stdout.contains("Verdict: APPROVED ✅"),
        "veredito traduzido: {stdout}"
    );
    assert!(
        !stdout.contains("Pacotes auditados"),
        "sem pt vazando no --lang en: {stdout}"
    );

    // Zero packages stays NO DATA in English too — never an approval.
    let empty = TempDir::new().unwrap();
    std::fs::write(
        empty.path().join("package.json"),
        r#"{"name":"sem-deps","version":"1.0.0"}"#,
    )
    .unwrap();
    let zero = devforge()
        .args(["audit", "--lang", "en"])
        .arg(empty.path())
        .output()
        .unwrap();
    assert!(zero.status.success());
    let stdout = String::from_utf8_lossy(&zero.stdout);
    assert!(stdout.contains("Verdict: NO DATA"), "no-data em EN: {stdout}");
    assert!(
        !stdout.contains("APPROVED"),
        "zero pacotes nunca vira aprovação: {stdout}"
    );
}
