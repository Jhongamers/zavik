//! Integration tests for `zavik run --detach` (real binary).
//!
//! The spawn mechanics themselves are unit-tested in `src/detached.rs`; here
//! we lock the **gate invariant**: detach executes code, so a non-interactive
//! session (tests, CI, agents without a human) is denied — `--yes` does NOT
//! cover it (same rule as foreground `run`).

#![cfg(unix)]

use std::process::Command;
use tempfile::TempDir;

fn zavik() -> Command {
    Command::new(env!("CARGO_BIN_EXE_zavik"))
}

/// Project with a resolvable start command (never executed: gate denies).
fn project_with_start() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name":"detach-fixture","scripts":{"start":"node server.js"}}"#,
    )
    .unwrap();
    dir
}

#[test]
fn detach_without_a_human_is_denied_even_with_yes() {
    let dir = project_with_start();
    let output = zavik()
        .args([
            "run",
            dir.path().to_str().unwrap(),
            "--detach",
            "--yes",
            "--json",
        ])
        .output()
        .unwrap();

    // Critical risk: --yes does not cover code execution.
    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout deve ser JSON puro");
    assert_eq!(json["command"], "run");
    assert_eq!(json["status"], "denied");

    // Nothing was spawned: no log file, no background process.
    assert!(
        !dir.path().join(".zavik/run.log").exists(),
        "negado = nada executado"
    );
}

#[test]
fn detach_dry_run_plans_without_executing() {
    let dir = project_with_start();
    let output = zavik()
        .args([
            "run",
            dir.path().to_str().unwrap(),
            "--detach",
            "--dry-run",
            "--json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "dry-run é read-only: sai 0");
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout deve ser JSON puro");
    assert_eq!(json["status"], "planned");
    assert_eq!(json["resolved"], "npm run start");
    assert!(
        !dir.path().join(".zavik/run.log").exists(),
        "nada executado"
    );
}
