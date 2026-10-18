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

fn fixture_python() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("main.py"), "print(\"hello\")\n").unwrap();
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"demo\"\n",
    )
    .unwrap();
    dir
}

#[test]
fn prepare_run_hints_json_is_english_even_under_default_pt() {
    let dir = fixture_python();
    let output = devforge()
        .env_remove("DEVFORGE_LANG")
        .args(["prepare", dir.path().to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    let body = json_lines(&output.stdout).pop().expect("json final de prepare");
    let hints: Vec<&str> = body["plan"]["run_hints"]
        .as_array()
        .expect("run_hints array")
        .iter()
        .map(|value| value.as_str().expect("hint string"))
        .collect();
    assert!(
        hints.contains(&"python3 main.py  (or your entrypoint)"),
        "hint EN no JSON sob default pt: {hints:?}"
    );
    // Same contract for the plan steps (composed at construction in Lang::En).
    let steps: Vec<&str> = body["plan"]["steps"]
        .as_array()
        .expect("steps array")
        .iter()
        .map(|step| step["title"].as_str().expect("title string"))
        .collect();
    assert!(
        steps.contains(&"Create virtual environment (python venv)"),
        "step title EN no JSON sob default pt: {steps:?}"
    );
}

#[test]
fn prepare_run_hints_human_follows_lang_flag() {
    let dir = fixture_python();
    let output = devforge()
        .env_remove("DEVFORGE_LANG")
        .args([
            "prepare",
            dir.path().to_str().unwrap(),
            "--dry-run",
            "--lang",
            "en",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("To run after preparation:"), "{stdout}");
    assert!(stdout.contains("(or your entrypoint)"), "{stdout}");
    assert!(!stdout.contains("Para rodar depois"), "sem pt: {stdout}");
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

#[test]
fn up_clone_failure_json_error_is_english() {
    // The composed clone error also reaches the JSON `"error"` field: it is
    // built in the target language (construction-lang), so JSON stays English
    // even under the default pt language.
    let sandbox = TempDir::new().unwrap();
    let missing = sandbox.path().join("nope-clone-src.git");
    let url = format!("file://{}", missing.display());
    let output = devforge()
        .env_remove("DEVFORGE_LANG")
        .current_dir(sandbox.path())
        .args(["up", &url, "--json", "--until", "clone"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "clone de fonte inexistente deve falhar"
    );
    let line: serde_json::Value = json_lines(&output.stdout)
        .into_iter()
        .last()
        .expect("uma linha JSON de falha");
    let error = line["error"].as_str().expect("error string");
    assert!(error.contains("git clone failed"), "error EN: {error}");
    assert!(!error.contains("falhou"), "resíduo pt no JSON: {error}");
    // git removes the destination on failure — nothing left behind.
    assert!(!sandbox.path().join("nope-clone-src").exists());
}
