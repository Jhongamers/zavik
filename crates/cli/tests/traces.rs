//! Integration tests for internal trace gating (real binary).
//!
//! The contract: engine diagnostics (`KERNEL:`/`CORE:`/`EVENTBUS:`/`[logger]`)
//! are hidden by default — a modern CLI only prints project data. They return
//! on stderr with `ZAVIK_VERBOSE=1`, and they NEVER touch stdout, so
//! `zavik detect > file` and the `--json` machine contract stay clean. See
//! `docs/design-style.md` §7.

#![cfg(unix)]

use std::process::Command;
use tempfile::TempDir;

fn zavik() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_zavik"));
    // Locale must not flip the post-flip default (en) on any machine.
    cmd.env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("LANGUAGE")
        .env_remove("LANG");
    cmd
}

fn js_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name":"traces-fixture","version":"1.0.0"}"#,
    )
    .unwrap();
    dir
}

/// Every internal trace prefix, checked against a decoded stream.
const TRACE_MARKERS: [&str; 4] = ["KERNEL:", "CORE:", "EVENTBUS:", "[logger]"];

fn has_trace(text: &str) -> bool {
    TRACE_MARKERS.iter().any(|marker| text.contains(marker))
}

// --- default: quiet ----------------------------------------------------------

#[test]
fn human_default_prints_no_internal_traces() {
    let project = js_project();
    let output = zavik()
        .args(["detect", "--lang", "en"])
        .arg(project.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !has_trace(&stderr),
        "default deve vir sem traces internos no stderr: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !has_trace(&stdout),
        "default deve vir sem traces internos no stdout: {stdout}"
    );
    // Real output is intact: the section header still explains what ran.
    assert!(
        stdout.contains("DETECTED STACKS"),
        "a saída real continua vindo: {stdout}"
    );
}

// --- opt-in: ZAVIK_VERBOSE ---------------------------------------------------

#[test]
fn zavik_verbose_brings_traces_back_to_stderr() {
    let project = js_project();
    let output = zavik()
        .env("ZAVIK_VERBOSE", "1")
        .args(["detect", "--lang", "en"])
        .arg(project.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Kernel init, the bus and the workflow engine each show up, in EN.
    assert!(
        stderr.contains("KERNEL: registering plugin"),
        "verbose mostra o registro de plugins: {stderr}"
    );
    assert!(
        stderr.contains("EVENTBUS: publishing"),
        "verbose mostra o event bus: {stderr}"
    );
    assert!(
        stderr.contains("CORE:") && stderr.contains("executing step"),
        "verbose mostra os passos do workflow: {stderr}"
    );
}

#[test]
fn traces_never_touch_stdout_even_under_zavik_verbose() {
    let project = js_project();
    let output = zavik()
        .env("ZAVIK_VERBOSE", "1")
        .args(["detect", "--lang", "en"])
        .arg(project.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !has_trace(&stdout),
        "traces são diagnóstico: stdout deve ficar limpo (o CORE: ia pro arquivo em `> file`): {stdout}"
    );
}

#[test]
fn zavik_verbose_zero_means_off() {
    let project = js_project();
    let output = zavik()
        .env("ZAVIK_VERBOSE", "0")
        .args(["detect", "--lang", "en"])
        .arg(project.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !has_trace(&stderr),
        "ZAVIK_VERBOSE=0 desliga as traces (mesma regra do CLICOLOR_FORCE): {stderr}"
    );
}

// --- machine contract --------------------------------------------------------

#[test]
fn json_stdout_is_byte_identical_with_and_without_zavik_verbose() {
    let project = js_project();

    let plain = zavik()
        .args(["detect", "--json"])
        .arg(project.path())
        .output()
        .unwrap();
    let verbose = zavik()
        .env("ZAVIK_VERBOSE", "1")
        .args(["detect", "--json"])
        .arg(project.path())
        .output()
        .unwrap();

    assert!(plain.status.success() && verbose.status.success());
    assert_eq!(
        plain.stdout, verbose.stdout,
        "--json deve ser byte-idêntico com ou sem traces"
    );
    assert!(
        String::from_utf8_lossy(&plain.stdout)
            .trim_start()
            .starts_with('{'),
        "stdout em --json continua sendo JSON"
    );
    // Diagnostics live on stderr, never inside the JSON payload.
    let verbose_stdout = String::from_utf8_lossy(&verbose.stdout);
    assert!(
        !has_trace(&verbose_stdout),
        "trace nunca entra no payload --json: {verbose_stdout}"
    );
}
