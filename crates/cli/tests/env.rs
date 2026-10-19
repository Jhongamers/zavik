//! Integration tests for `zavik env` (real binary).
//!
//! `env` is a read-only environment fingerprint: OS/arch + each detected
//! stack's tools with PATH presence and real version. No gates (nothing
//! executes beyond `--version` probes of tools already installed) and it
//! never reads `.env` files.

#![cfg(unix)]

use std::fs;
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

/// Minimal JavaScript project so stack detection has something to find.
fn js_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("package.json"),
        r#"{"name":"env-fixture","version":"1.0.0"}"#,
    )
    .unwrap();
    dir
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

// --- env --json -------------------------------------------------------------

#[test]
fn env_json_reports_os_stacks_and_tool_versions() {
    let project = js_project();
    let output = zavik()
        .args(["env", "--json"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "env é read-only: deve sair 0");

    let body = single_json(&output.stdout);
    assert_eq!(body["schema"], 1);
    assert_eq!(body["command"], "env");
    assert!(!body["os"].as_str().unwrap().is_empty(), "os presente");
    assert!(!body["arch"].as_str().unwrap().is_empty(), "arch presente");

    // JavaScript detected → node/npm listed with a boolean presence flag.
    let stacks = body["stacks"].as_array().unwrap();
    let js = stacks
        .iter()
        .find(|s| s["language"] == "JavaScript")
        .expect("stack JavaScript detectado");
    let tools: Vec<&str> = js["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(tools.contains(&"node"), "node listado: {tools:?}");
    assert!(tools.contains(&"npm"), "npm listado: {tools:?}");
    for tool in js["tools"].as_array().unwrap() {
        assert!(tool["present"].is_boolean(), "present é booleano");
        // Version is a string when read, null when the probe failed — never
        // an invented value.
        assert!(
            tool["version"].is_string() || tool["version"].is_null(),
            "version é string ou null: {tool}"
        );
    }
}

#[test]
fn env_json_reports_missing_tools_when_path_is_empty() {
    let project = js_project();
    let output = zavik()
        .env("PATH", "") // a máquina "sem nada instalado"
        .args(["env", "--json"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "read-only: sai 0 mesmo com tudo faltando"
    );

    let body = single_json(&output.stdout);
    let missing = body["missing"].as_array().unwrap();
    let names: Vec<&str> = missing.iter().map(|m| m.as_str().unwrap()).collect();
    assert!(
        names.contains(&"node"),
        "node faltando com PATH vazio: {names:?}"
    );
    assert!(
        names.contains(&"npm"),
        "npm faltando com PATH vazio: {names:?}"
    );

    // Missing tools carry present=false and version=null (no invention).
    let js = body["stacks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["language"] == "JavaScript")
        .expect("stack JavaScript");
    let node = js["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "node")
        .unwrap();
    assert_eq!(node["present"], false);
    assert_eq!(node["version"], serde_json::Value::Null);
}

#[test]
fn env_never_reads_dotenv_files() {
    let project = js_project();
    fs::write(
        project.path().join(".env"),
        "SUPER_SECRET_TOKEN=leak-me-not\n",
    )
    .unwrap();

    let output = zavik()
        .args(["env", "--json"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("leak-me-not") && !stdout.contains("SUPER_SECRET"),
        "env não lê .env — segredos não podem aparecer na saída"
    );
}

// --- human mode -------------------------------------------------------------

#[test]
fn env_human_prints_sections() {
    let project = js_project();
    let output = zavik()
        .args(["env"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ENVIRONMENT"), "seção ENVIRONMENT no modo humano");
    assert!(stdout.contains("os:"), "OS na saída humana");
    assert!(stdout.contains("node"), "toolchains por stack");
}
