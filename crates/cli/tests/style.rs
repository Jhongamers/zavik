//! Integration tests for terminal styling (real binary).
//!
//! The contract: colors are PRESENTATION ONLY. They appear on a TTY (or when
//! `CLICOLOR_FORCE` asks for them), they vanish under `NO_COLOR` / pipes, and
//! they never touch the machine contract — `--json` stays byte-identical no
//! matter what the color decision is.

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
        r#"{"name":"style-fixture","version":"1.0.0"}"#,
    )
    .unwrap();
    dir
}

fn ansi_escapes(bytes: &[u8]) -> bool {
    bytes.contains(&0x1b)
}

// --- color presence/absence -------------------------------------------------

#[test]
fn piped_stdout_has_no_ansi() {
    // cargo test pipes stdout → not a TTY → plain text by default.
    let project = js_project();
    let output = zavik()
        .args(["env", "--lang", "en"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        !ansi_escapes(&output.stdout),
        "stdout piped deve vir sem ANSI"
    );
}

#[test]
fn clicolor_force_emits_ansi_even_without_tty() {
    let project = js_project();
    let output = zavik()
        .env("CLICOLOR_FORCE", "1")
        .args(["env", "--lang", "en"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        ansi_escapes(&output.stdout),
        "CLICOLOR_FORCE deve forçar ANSI mesmo sem TTY"
    );
}

#[test]
fn no_color_beats_clicolor_force() {
    // no-color.org: NO_COLOR wins over any force flag.
    let project = js_project();
    let output = zavik()
        .env("NO_COLOR", "1")
        .env("CLICOLOR_FORCE", "1")
        .args(["env", "--lang", "en"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        !ansi_escapes(&output.stdout),
        "NO_COLOR deve vencer CLICOLOR_FORCE"
    );
}

// --- the machine contract stays untouched -----------------------------------

#[test]
fn json_is_byte_identical_regardless_of_color() {
    // `--json` is the machine contract: ANSI must never leak in, and the bytes
    // must not depend on the color decision.
    let project = js_project();
    let plain = zavik()
        .args(["env", "--json"])
        .arg(project.path())
        .output()
        .unwrap();
    let forced = zavik()
        .env("CLICOLOR_FORCE", "1")
        .args(["env", "--json"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(plain.status.success() && forced.status.success());
    assert_eq!(
        plain.stdout, forced.stdout,
        "JSON byte-idêntico com/sem cor"
    );
    assert!(
        !ansi_escapes(&forced.stdout),
        "JSON nunca contém ANSI, mesmo com CLICOLOR_FORCE"
    );
}

// --- text survives styling (no color → identical words) --------------------

#[test]
fn styled_human_output_keeps_the_same_words() {
    // With CLICOLOR_FORCE the escape codes wrap the SAME text: a terminal (or
    // a strip filter) still reads "project:" and "ENVIRONMENT".
    let project = js_project();
    let output = zavik()
        .env("CLICOLOR_FORCE", "1")
        .args(["env", "--lang", "en"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ENVIRONMENT"), "seção com cor: {stdout}");
    assert!(stdout.contains("project: "), "banner com cor: {stdout}");
}
