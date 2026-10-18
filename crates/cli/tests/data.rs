//! Integration tests for `devforge data` (real binary).
//!
//! Covers everything reachable WITHOUT a real license key (the happy path —
//! valid license + valid signature — is the manual E2E with the production
//! keys, see docs/design-data.md §6): the gate denials, the honest status
//! states, and the free-tier invariant: no dataset → `audit` output is
//! byte-identical (no dataset keys leak into stdout).

#![cfg(unix)]

use std::process::Command;
use tempfile::TempDir;

fn devforge() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_devforge"));
    // Locale must not flip the post-flip default (en) on any machine.
    cmd.env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("LANGUAGE")
        .env_remove("LANG");
    cmd
}

/// Isolated XDG root: no real license/dataset of the machine can leak in,
/// and nothing the test writes can leak out.
fn isolated_xdg() -> TempDir {
    TempDir::new().unwrap()
}

fn with_xdg<'a>(cmd: &'a mut Command, xdg: &'a TempDir) -> &'a mut Command {
    cmd.env("XDG_CONFIG_HOME", xdg.path())
}

fn devforge_dir(xdg: &TempDir) -> std::path::PathBuf {
    xdg.path().join("devforge")
}

/// One JSON object on stdout (commands here emit a single line).
fn single_json(stdout: &[u8]) -> serde_json::Value {
    let text = String::from_utf8_lossy(stdout);
    let line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_else(|| panic!("stdout vazio: {text}"));
    serde_json::from_str(line).unwrap_or_else(|e| panic!("stdout não é JSON: {line}\n{e}"))
}

// --- data status (free, offline) ------------------------------------------

#[test]
fn status_without_dataset_is_free_and_offline() {
    let xdg = isolated_xdg();
    let output = {
        let mut cmd = devforge();
        with_xdg(&mut cmd, &xdg);
        cmd.args(["data", "status", "--json"]).output().unwrap()
    };
    assert!(output.status.success(), "status é livre: deve sair 0");
    let body = single_json(&output.stdout);
    assert_eq!(body["command"], "data");
    assert_eq!(body["sub"], "status");
    assert_eq!(body["installed"], false);
    assert_eq!(body["tier"], "free");
    assert!(body["hint"].as_str().unwrap().contains("Pro"));
}

#[test]
fn status_human_explains_the_free_tier_honestly() {
    let xdg = isolated_xdg();
    let output = {
        let mut cmd = devforge();
        with_xdg(&mut cmd, &xdg);
        cmd.args(["data", "status"]).output().unwrap()
    };
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("no dataset installed"), "{text}");
    assert!(text.contains("free:"), "deve dizer o que é grátis: {text}");
    assert!(text.contains("Pro"), "deve dizer o que é Pro: {text}");
}

#[test]
fn status_with_corrupt_dataset_reports_invalid_never_silently() {
    let xdg = isolated_xdg();
    std::fs::create_dir_all(devforge_dir(&xdg)).unwrap();
    std::fs::write(devforge_dir(&xdg).join("dataset.json"), b"lixo sem assinatura").unwrap();

    let output = {
        let mut cmd = devforge();
        with_xdg(&mut cmd, &xdg);
        cmd.args(["data", "status", "--json"]).output().unwrap()
    };
    assert!(output.status.success(), "status é diagnóstico: sai 0");
    let body = single_json(&output.stdout);
    assert_eq!(body["installed"], true, "o arquivo existe...");
    assert_eq!(body["valid"], false, "...mas não é usável");
    // JSON prose is ALWAYS English, even under the default pt language.
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("invalid dataset format"),
        "{body}"
    );
}

// --- data update: the license gate (first, before any network) ------------

#[test]
fn update_without_license_denies_with_json_line_and_exit_1() {
    let xdg = isolated_xdg();
    let output = {
        let mut cmd = devforge();
        with_xdg(&mut cmd, &xdg);
        cmd.args(["data", "update", "--json"]).output().unwrap()
    };
    assert_eq!(output.status.code(), Some(1), "sem licença = exit 1");
    let body = single_json(&output.stdout);
    assert_eq!(body["status"], "no_license");
    // JSON prose is ALWAYS English, even under the default pt language.
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("no license installed"),
        "{body}"
    );
}

#[test]
fn update_without_license_denies_in_human_message_that_keeps_free_intact() {
    let xdg = isolated_xdg();
    let output = {
        let mut cmd = devforge();
        with_xdg(&mut cmd, &xdg);
        cmd.args(["data", "update"]).output().unwrap()
    };
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Pro"), "{text}");
    assert!(text.contains("license install"), "deve dizer como instalar: {text}");
    assert!(text.contains("free tier"), "deve prometer que o tier grátis não muda: {text}");
    let out = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        out.matches("baixando").count() + out.matches("downloading").count(),
        0,
        "nunca toca a rede sem licença"
    );
}

#[test]
fn update_with_garbage_license_denies_as_invalid_not_as_missing() {
    let xdg = isolated_xdg();
    std::fs::create_dir_all(devforge_dir(&xdg)).unwrap();
    std::fs::write(devforge_dir(&xdg).join("license.key"), "token-forjado").unwrap();

    let output = {
        let mut cmd = devforge();
        with_xdg(&mut cmd, &xdg);
        cmd.args(["data", "update", "--json"]).output().unwrap()
    };
    assert_eq!(output.status.code(), Some(1));
    let body = single_json(&output.stdout);
    assert_eq!(body["status"], "license_invalid", "{body}");
}

// --- argument parsing -------------------------------------------------------

#[test]
fn unknown_subcommand_and_flag_are_rejected() {
    let xdg = isolated_xdg();

    let unknown = {
        let mut cmd = devforge();
        with_xdg(&mut cmd, &xdg);
        cmd.args(["data", "banana"]).output().unwrap()
    };
    assert_eq!(unknown.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&unknown.stderr).contains("status"),
        "erro deve ensinar os subcomandos: {:?}",
        String::from_utf8_lossy(&unknown.stderr)
    );

    let unknown_flag = {
        let mut cmd = devforge();
        with_xdg(&mut cmd, &xdg);
        cmd.args(["data", "status", "--bogus"]).output().unwrap()
    };
    assert_eq!(unknown_flag.status.code(), Some(1));

    let url_without_value = {
        let mut cmd = devforge();
        with_xdg(&mut cmd, &xdg);
        cmd.args(["data", "update", "--url"]).output().unwrap()
    };
    assert_eq!(url_without_value.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&url_without_value.stderr).contains("requires a value"),
        "{:?}",
        String::from_utf8_lossy(&url_without_value.stderr)
    );
}

// --- free-tier invariant: no dataset → audit output unchanged --------------

fn fixture_npm() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name":"demo","scripts":{"start":"node server.js"},"dependencies":{}}"#,
    )
    .unwrap();
    dir
}

#[test]
fn audit_without_dataset_has_no_dataset_keys() {
    let xdg = isolated_xdg();
    let project = fixture_npm();
    let output = {
        let mut cmd = devforge();
        with_xdg(&mut cmd, &xdg);
        cmd.args(["audit", project.path().to_str().unwrap(), "--json"])
            .output()
            .unwrap()
    };
    assert!(output.status.success(), "audit grátis deve sair 0 no fixture");
    let body = single_json(&output.stdout);
    assert_eq!(body["command"], "audit");
    assert!(body.get("score").is_some(), "contrato base intacto");
    assert!(body.get("dataset").is_none(), "sem dataset ⇒ sem chave dataset");
    assert!(
        body.get("dataset_findings").is_none(),
        "sem dataset ⇒ sem dataset_findings (invariante §6)"
    );
}

#[test]
fn audit_with_corrupt_dataset_warns_on_stderr_and_keeps_stdout_clean() {
    let xdg = isolated_xdg();
    std::fs::create_dir_all(devforge_dir(&xdg)).unwrap();
    std::fs::write(devforge_dir(&xdg).join("dataset.json"), b"adulterado").unwrap();

    let project = fixture_npm();
    let output = {
        let mut cmd = devforge();
        with_xdg(&mut cmd, &xdg);
        cmd.args(["audit", project.path().to_str().unwrap(), "--json"])
            .output()
            .unwrap()
    };
    assert!(output.status.success(), "dataset inválido não quebra o audit");
    let body = single_json(&output.stdout);
    assert!(body.get("dataset").is_none(), "inválido nunca vira seção");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("INVALID"),
        "anomalia é barulhenta no stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}
