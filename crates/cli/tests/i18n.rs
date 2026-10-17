//! Integration tests for i18n (real binary): `--lang`, `DEVFORGE_LANG`,
//! the migration-phase default, and the invariant that machine output
//! (JSON) never changes language (docs/design-i18n.md).

#![cfg(unix)]

use std::process::Command;
use tempfile::TempDir;

fn devforge() -> Command {
    Command::new(env!("CARGO_BIN_EXE_devforge"))
}

/// Minimal JavaScript project — `env` prints human sections in both langs.
fn js_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name":"i18n-fixture","version":"1.0.0"}"#,
    )
    .unwrap();
    dir
}

fn stdout_of(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn default_language_is_ptbr_until_the_migration_flips() {
    let project = js_project();
    let output = devforge()
        .env_remove("DEVFORGE_LANG") // the machine must not leak in
        .args(["env"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = stdout_of(&output);
    assert!(stdout.contains("AMBIENTE"), "default segue pt-BR: {stdout}");
    assert!(
        !stdout.contains("ENVIRONMENT"),
        "sem flag nada muda: {stdout}"
    );
}

#[test]
fn lang_flag_switches_human_output_to_english() {
    let project = js_project();
    let output = devforge()
        .env_remove("DEVFORGE_LANG")
        .args(["env", "--lang", "en"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = stdout_of(&output);
    assert!(stdout.contains("ENVIRONMENT"), "--lang en: {stdout}");
    assert!(
        !stdout.contains("AMBIENTE"),
        "sem resíduo pt-BR no header: {stdout}"
    );
    assert!(
        stdout.contains("   project: "),
        "banner traduzido: {stdout}"
    );
}

#[test]
fn lang_flag_accepts_equals_form_and_ptbr_explicitly() {
    let project = js_project();
    let output = devforge()
        .args(["env", "--lang=pt-BR"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "--lang=pt-BR é válido");
    assert!(stdout_of(&output).contains("AMBIENTE"));
}

#[test]
fn env_var_lang_is_honored_for_agents() {
    let project = js_project();
    let output = devforge()
        .env("DEVFORGE_LANG", "en")
        .args(["env"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        stdout_of(&output).contains("ENVIRONMENT"),
        "DEVFORGE_LANG=en"
    );
}

#[test]
fn flag_beats_env_var() {
    let project = js_project();
    let output = devforge()
        .env("DEVFORGE_LANG", "en")
        .args(["env", "--lang", "pt-BR"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        stdout_of(&output).contains("AMBIENTE"),
        "--lang > DEVFORGE_LANG"
    );
}

#[test]
fn invalid_lang_fails_loudly_with_the_valid_options() {
    let project = js_project();
    let output = devforge()
        .env_remove("DEVFORGE_LANG")
        .args(["env", "--lang", "de"])
        .arg(project.path())
        .output()
        .unwrap();

    assert_ne!(output.status.code(), Some(0), "idioma inválido não passa");
    let combined = format!(
        "{}{}",
        stdout_of(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("pt-BR") && combined.contains("en"),
        "a mensagem lista os idiomas válidos: {combined}"
    );
}

#[test]
fn json_is_language_independent() {
    let project = js_project();
    let pt = devforge()
        .env_remove("DEVFORGE_LANG")
        .args(["env", "--json"])
        .arg(project.path())
        .output()
        .unwrap();
    let en = devforge()
        .args(["env", "--json", "--lang", "en"])
        .arg(project.path())
        .output()
        .unwrap();

    let pt_json: serde_json::Value = serde_json::from_slice(&pt.stdout).expect("json pt");
    let en_json: serde_json::Value = serde_json::from_slice(&en.stdout).expect("json en");
    // Machine contract: identical shape and English keys/values in both.
    assert_eq!(pt_json["command"], "env");
    assert_eq!(en_json["command"], "env");
    assert_eq!(pt_json["stacks"], en_json["stacks"]);
    assert_eq!(en_json["os"], pt_json["os"]);
}

#[test]
fn lang_after_double_dash_is_passed_to_the_child_not_consumed() {
    let project = TempDir::new().unwrap();
    std::fs::write(
        project.path().join("package.json"),
        r#"{"name":"i18n-run","scripts":{"start":"node server.js"}}"#,
    )
    .unwrap();
    let output = devforge()
        .env_remove("DEVFORGE_LANG")
        .args([
            "run",
            project.path().to_str().unwrap(),
            "--dry-run",
            "--json",
            "--",
            "--lang",
            "en",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "args pós -- não são nossos");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    let resolved = json["resolved"].as_str().unwrap();
    assert!(
        resolved.contains("--lang en"),
        "o --lang foi pro comando filho: {resolved}"
    );
}
