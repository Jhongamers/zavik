//! Integration test for broken-pipe behavior (dogfooding #15):
//!
//! Rust ignores SIGPIPE by default, so writing to a pipe whose read end is
//! already closed (`zavik … | head -c 1`) used to panic on stdout and exit
//! 101 — which an AI driver reads as a product crash. `main` restores the
//! default SIGPIPE handler so the process dies cleanly on the signal (exit
//! 141), like a normal CLI.

#![cfg(unix)]

use std::process::{Command, Stdio};

fn zavik() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_zavik"));
    // Locale must not flip the post-flip default (en) on any machine.
    cmd.env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("LANGUAGE")
        .env_remove("LANG");
    cmd
}

/// Runs `zavik analyze . --json` with its stdout piped into a child that
/// closes the read end immediately, then asserts the process did NOT panic.
#[test]
fn closed_stdout_pipe_dies_cleanly_not_with_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    // A minimal manifest so the command has something to analyze.
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"pipe-fixture\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    let mut child = zavik()
        .args(["analyze", ".", "--json"])
        .current_dir(dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn zavik");

    // Close the read end at once so the very first stdout write hits EPIPE.
    drop(child.stdout.take().expect("stdout piped"));

    let output = child.wait_with_output().expect("wait for zavik");

    // With SIGPIPE restored, the process is killed by the signal (no panic).
    // Without the fix it exits 101 after panicking on stdout.
    assert!(
        !output.status.success(),
        "process must not report success when its stdout is closed: {output:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked"),
        "no panic on broken pipe (SIGPIPE default restored): {stderr}"
    );
    assert!(
        !stderr.contains("Broken pipe"),
        "no 'Broken pipe' error surfaced: {stderr}"
    );
}
