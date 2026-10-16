//! Integration tests for `devforge up` (real binary).
//!
//! These cover the pipeline paths that do NOT execute project code (plan,
//! `--until audit`, denials), so the permission gates behave deterministically
//! in a non-interactive test runner and the JSONL contract stays locked.

#![cfg(unix)]

use std::process::Command;
use tempfile::TempDir;

fn devforge() -> Command {
    Command::new(env!("CARGO_BIN_EXE_devforge"))
}

fn fixture_npm() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name":"demo","scripts":{"start":"node server.js"},"dependencies":{}}"#,
    )
    .unwrap();
    dir
}

/// Parses stdout as JSON Lines: every non-empty line must be valid JSON.
fn json_lines(stdout: &[u8]) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("linha não é JSON: {line}\n{e}")))
        .collect()
}

#[test]
fn dry_run_json_lists_full_plan() {
    let dir = fixture_npm();
    let output = devforge()
        .args(["up", dir.path().to_str().unwrap(), "--dry-run", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success(), "dry-run deve sair 0");
    let lines = json_lines(&output.stdout);
    assert_eq!(lines.len(), 1, "dry-run deve emitir um único objeto");
    let plan = &lines[0];
    assert_eq!(plan["schema"], 1);
    assert_eq!(plan["command"], "up");
    assert_eq!(plan["status"], "planned");
    let names: Vec<&str> = plan["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["audit", "prepare", "run"], "path não clona");
    assert!(plan["steps"]
        .as_array()
        .unwrap()
        .iter()
        .all(|step| step["status"] == "planned"));
}

#[test]
fn url_dry_run_includes_clone_step() {
    let output = devforge()
        .args(["up", "https://example.com/x/y.git", "--dry-run", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let lines = json_lines(&output.stdout);
    let plan = &lines[0];
    let names: Vec<&str> = plan["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["clone", "audit", "prepare", "run"]);
    assert_eq!(plan["target"], "https://example.com/x/y.git");
}

#[test]
fn until_audit_runs_audit_only_and_summarizes() {
    let dir = fixture_npm();
    let output = devforge()
        .args(["up", dir.path().to_str().unwrap(), "--until", "audit", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success(), "só audit deve sair 0");
    let lines = json_lines(&output.stdout);
    assert_eq!(lines.len(), 2, "esperava linha do audit + linha final do up");

    assert_eq!(lines[0]["command"], "audit", "primeira linha = resultado do audit");

    let summary = &lines[1];
    assert_eq!(summary["command"], "up");
    assert_eq!(summary["status"], "ok");
    assert_eq!(summary["exit_code"], 0);
    let states: Vec<(&str, &str)> = summary["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| (step["name"].as_str().unwrap(), step["status"].as_str().unwrap()))
        .collect();
    assert_eq!(
        states,
        [("audit", "ok"), ("prepare", "skipped"), ("run", "skipped")]
    );
}

#[test]
fn human_non_tty_denies_and_executes_nothing() {
    let dir = fixture_npm();
    // stdin is null in `output()` → the interactive gate denies (EOF): the
    // pipeline must stop and run NOTHING of the project.
    let output = devforge()
        .args(["up", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1), "não-interativo nega por padrão");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("Pipeline concluído"),
        "nunca conclui sem executar as etapas: {stdout}"
    );
    assert!(!dir.path().join("node_modules").exists(), "nada foi instalado");
}

#[test]
fn nonexistent_path_fails_honest() {
    let output = devforge()
        .args(["up", "/tmp/devforge-path-that-does-not-exist-xyz", "--json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("caminho não existe"), "stderr: {stderr}");
}

#[test]
fn invalid_until_is_rejected() {
    let dir = fixture_npm();
    let output = devforge()
        .args(["up", dir.path().to_str().unwrap(), "--until", "banana", "--json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--until"), "stderr: {stderr}");
}

#[test]
fn url_with_skip_clone_is_rejected() {
    let output = devforge()
        .args(["up", "https://example.com/x/y.git", "--skip", "clone", "--dry-run", "--json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--skip clone"), "stderr: {stderr}");
}
