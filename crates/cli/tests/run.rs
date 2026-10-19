//! Integration tests for `zavik run` (real binary).
//!
//! Cover the paths that do NOT execute code (dry-run, ambiguity, not
//! found) — so the permission gate is never hit in tests and the `--json`
//! contract stays locked.

#![cfg(unix)]

use std::process::Command;
use tempfile::TempDir;

fn zavik() -> Command {
    Command::new(env!("CARGO_BIN_EXE_zavik"))
}

#[test]
fn dry_run_resolves_npm_start_and_returns_json_plan() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name":"demo","scripts":{"start":"node server.js"}}"#,
    )
    .unwrap();

    let output = zavik()
        .args(["run", dir.path().to_str().unwrap(), "--dry-run", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success(), "dry-run deve sair 0");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout deve ser JSON puro");
    assert_eq!(json["schema"], 1);
    assert_eq!(json["command"], "run");
    assert_eq!(json["status"], "planned");
    assert_eq!(json["resolved"], "npm run start");
    assert_eq!(json["source"], "package.json scripts.start");
}

#[test]
fn human_dry_run_shows_command_and_source() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("Makefile"), "run:\n\t./app\n").unwrap();

    let output = zavik()
        .args(["run", dir.path().to_str().unwrap(), "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("make run"), "deveria mostrar o comando: {stdout}");
    assert!(stdout.contains("Makefile"), "deveria mostrar a fonte: {stdout}");
}

#[test]
fn ambiguous_procfile_lists_candidates_and_does_not_execute() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("Procfile"), "worker: node w.js\nclock: node c.js\n").unwrap();

    let output = zavik()
        .args(["run", dir.path().to_str().unwrap(), "--json"])
        .output()
        .unwrap();

    // No interaction → denies by default (exit 1) and returns the candidates.
    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout deve ser JSON puro");
    assert_eq!(json["status"], "ambiguous");
    assert_eq!(json["candidates"].as_array().unwrap().len(), 2);
}

#[test]
fn project_without_entrypoint_is_not_found() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("README.md"), "oi").unwrap();

    let output = zavik()
        .args(["run", dir.path().to_str().unwrap(), "--json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout deve ser JSON puro");
    assert_eq!(json["status"], "not_found");
}

#[test]
fn flags_after_separator_go_to_the_command() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"scripts":{"start":"node server.js"}}"#,
    )
    .unwrap();

    let output = zavik()
        .args([
            "run",
            dir.path().to_str().unwrap(),
            "--dry-run",
            "--json",
            "--",
            "--port",
            "3001",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["resolved"], "npm run start --port 3001");
}
