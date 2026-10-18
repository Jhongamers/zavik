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

#[test]
fn help_translates_to_english_and_stays_pt_by_default() {
    let pt = Command::new(env!("CARGO_BIN_EXE_devforge"))
        .arg("help")
        .output()
        .unwrap();
    assert!(pt.status.success());
    let pt = String::from_utf8_lossy(&pt.stdout);
    assert!(pt.contains("USO:"), "help default continua pt-BR: {pt}");

    let en = Command::new(env!("CARGO_BIN_EXE_devforge"))
        .args(["help", "--lang", "en"])
        .output()
        .unwrap();
    assert!(en.status.success());
    let en = String::from_utf8_lossy(&en.stdout);
    assert!(en.contains("USAGE:"), "help em EN: {en}");
    assert!(!en.contains("USO:"), "sem pt vazando no --lang en: {en}");
    assert!(
        en.contains("--lang       human-output language: pt-BR | en"),
        "--lang documentado em EN: {en}"
    );
    // O texto novo vale nos dois idiomas (chave = const HELP_PT, sem drift).
    assert!(
        pt.contains("--lang       idioma da saída humana: pt-BR | en"),
        "--lang documentado em pt-BR: {pt}"
    );
}

#[test]
fn diagnose_human_output_follows_lang_flag() {
    // Human path: section, composed summary, cause and labels all follow `--lang`.
    let en = devforge()
        .env_remove("DEVFORGE_LANG")
        .args([
            "diagnose",
            "--message",
            "sh: 1: cargo: not found",
            "--lang",
            "en",
        ])
        .output()
        .unwrap();
    assert!(en.status.success());
    let stdout = stdout_of(&en);
    assert!(stdout.contains("DIAGNOSIS"), "seção em EN: {stdout}");
    assert!(
        stdout.contains("Recognized 1 pattern(s)"),
        "summary composto em EN: {stdout}"
    );
    assert!(
        stdout.contains("not installed on the PATH"),
        "causa em EN: {stdout}"
    );
    assert!(stdout.contains("  fix:"), "rótulo em EN: {stdout}");
    assert!(
        !stdout.contains("conserto:") && !stdout.contains("DIAGNÓSTICO"),
        "sem pt vazando no --lang en: {stdout}"
    );

    // Default stays pt-BR until the phase-3 flip.
    let pt = devforge()
        .env_remove("DEVFORGE_LANG")
        .args(["diagnose", "--message", "sh: 1: cargo: not found"])
        .output()
        .unwrap();
    assert!(pt.status.success());
    let stdout = stdout_of(&pt);
    assert!(
        stdout.contains("DIAGNÓSTICO") && stdout.contains("  conserto:"),
        "default continua pt-BR: {stdout}"
    );
}

#[test]
fn diagnose_json_prose_is_always_english_whatever_the_lang_flag() {
    // JSON contract: prose inside JSON is English even with the default
    // (pt-BR) language — and the document is byte-for-byte the same.
    let pt = devforge()
        .env_remove("DEVFORGE_LANG")
        .args(["diagnose", "--message", "sh: 1: cargo: not found", "--json"])
        .output()
        .unwrap();
    let en = devforge()
        .args([
            "diagnose",
            "--message",
            "sh: 1: cargo: not found",
            "--json",
            "--lang",
            "en",
        ])
        .output()
        .unwrap();
    assert!(pt.status.success() && en.status.success());

    let pt_json: serde_json::Value = serde_json::from_slice(&pt.stdout).expect("json default");
    let en_json: serde_json::Value = serde_json::from_slice(&en.stdout).expect("json en");
    assert_eq!(
        pt_json, en_json,
        "JSON não muda com --lang (chaves, enums e prose)"
    );
    let summary = pt_json["summary"].as_str().expect("summary string");
    assert!(
        summary.starts_with("Recognized 1 pattern(s)"),
        "prose de JSON em EN sob default pt: {summary}"
    );
    let cause = pt_json["causes"][0]["cause"].as_str().expect("cause string");
    assert!(
        cause.contains("not installed on the PATH"),
        "cause em EN: {cause}"
    );
}

#[test]
fn run_output_follows_lang_flag() {
    // NotFound path: how-to-start guidance in English, no pt residue.
    let bare = TempDir::new().unwrap();
    std::fs::write(
        bare.path().join("package.json"),
        r#"{"name":"i18n-run-bare","version":"1.0.0"}"#,
    )
    .unwrap();
    let en = devforge()
        .env_remove("DEVFORGE_LANG")
        .args(["run", bare.path().to_str().unwrap(), "--lang", "en"])
        .output()
        .unwrap();
    let stdout = stdout_of(&en);
    assert!(
        stdout.contains("I couldn't find how to start this project."),
        "not_found em EN: {stdout}"
    );
    assert!(
        stdout.contains("declare a start") && !stdout.contains("Não achei"),
        "sem pt no --lang en: {stdout}"
    );

    // Planned path: the dry-run plan headers in English.
    let runnable = TempDir::new().unwrap();
    std::fs::write(
        runnable.path().join("package.json"),
        r#"{"name":"i18n-run-ok","scripts":{"start":"node server.js"}}"#,
    )
    .unwrap();
    let en = devforge()
        .env_remove("DEVFORGE_LANG")
        .args([
            "run",
            runnable.path().to_str().unwrap(),
            "--dry-run",
            "--lang",
            "en",
        ])
        .output()
        .unwrap();
    assert!(en.status.success());
    let stdout = stdout_of(&en);
    assert!(stdout.contains("Execution plan:"), "plano em EN: {stdout}");
    assert!(stdout.contains("   command :"), "label em EN: {stdout}");
    assert!(
        stdout.contains("(dry-run: nothing was executed)"),
        "nota em EN: {stdout}"
    );
    assert!(
        !stdout.contains("Plano de execução") && !stdout.contains("nada foi executado"),
        "sem pt no plano: {stdout}"
    );
}

#[test]
fn data_and_license_follow_lang_flag() {
    // Isolated config → deterministic "absent" paths for dataset and license.
    let config = TempDir::new().unwrap();

    let en = devforge()
        .env_remove("DEVFORGE_LANG")
        .env("XDG_CONFIG_HOME", config.path())
        .args(["data", "status", "--lang", "en"])
        .output()
        .unwrap();
    assert!(en.status.success());
    let stdout = stdout_of(&en);
    assert!(
        stdout.contains("no dataset installed"),
        "data status em EN: {stdout}"
    );
    assert!(
        stdout.contains("free: audit uses the offline engine") && !stdout.contains("grátis"),
        "sem pt no --lang en: {stdout}"
    );

    let en = devforge()
        .env_remove("DEVFORGE_LANG")
        .env("XDG_CONFIG_HOME", config.path())
        .args(["license", "status", "--lang", "en"])
        .output()
        .unwrap();
    let stdout = stdout_of(&en);
    assert!(
        stdout.contains("no license installed"),
        "license status em EN: {stdout}"
    );
    assert!(!stdout.contains("nenhuma licença"), "sem pt: {stdout}");

    // JSON prose stays English under the default pt language (hint field).
    let out = devforge()
        .env_remove("DEVFORGE_LANG")
        .env("XDG_CONFIG_HOME", config.path())
        .args(["data", "status", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value =
        serde_json::from_str(stdout_of(&out).trim()).expect("data status json");
    let hint = json["hint"].as_str().unwrap_or("no hint");
    assert!(
        hint.contains("offline with the built-in engine") && !hint.contains("motor embutido"),
        "hint EN em JSON sob --lang pt: {hint}"
    );
}

#[test]
fn up_dry_run_output_follows_lang_flag() {
    // pt default: plan section, target label and the dry-run note stay pt.
    let project = js_project();
    let pt = devforge()
        .env_remove("DEVFORGE_LANG")
        .args(["up", project.path().to_str().unwrap(), "--dry-run"])
        .output()
        .unwrap();
    let stdout = stdout_of(&pt);
    assert!(stdout.contains("PLANO DO UP"), "cabeçalho pt: {stdout}");
    assert!(stdout.contains("Alvo:"), "Alvo pt: {stdout}");
    assert!(
        stdout.contains("(--dry-run: nada foi executado.)"),
        "nota pt: {stdout}"
    );

    // --lang en: the same plan fully English, no pt residue.
    let project = js_project();
    let en = devforge()
        .env_remove("DEVFORGE_LANG")
        .args([
            "up",
            project.path().to_str().unwrap(),
            "--dry-run",
            "--lang",
            "en",
        ])
        .output()
        .unwrap();
    let stdout = stdout_of(&en);
    assert!(stdout.contains("UP PLAN"), "UP PLAN em EN: {stdout}");
    assert!(stdout.contains("Target:"), "Target em EN: {stdout}");
    assert!(
        stdout.contains("(--dry-run: nothing was executed.)"),
        "nota em EN: {stdout}"
    );
    assert!(
        !stdout.contains("Alvo:") && !stdout.contains("nada foi executado"),
        "sem pt no --lang en: {stdout}"
    );
}

#[test]
fn prepare_trust_and_plan_follow_lang_flag() {
    // pt default: trust line, plan section and dry-run note stay pt.
    let project = js_project();
    let pt = devforge()
        .env_remove("DEVFORGE_LANG")
        .args(["prepare", project.path().to_str().unwrap(), "--dry-run"])
        .output()
        .unwrap();
    let stdout = stdout_of(&pt);
    assert!(
        stdout.contains("Verificação de confiança: nenhum risco detectado."),
        "trust pt: {stdout}"
    );
    assert!(stdout.contains("PLANO (sem Docker)"), "plano pt: {stdout}");

    // --lang en: the same surfaces fully English, no pt residue.
    let project = js_project();
    let en = devforge()
        .env_remove("DEVFORGE_LANG")
        .args([
            "prepare",
            project.path().to_str().unwrap(),
            "--dry-run",
            "--lang",
            "en",
        ])
        .output()
        .unwrap();
    let stdout = stdout_of(&en);
    assert!(
        stdout.contains("Trust verification: no risk detected."),
        "trust em EN: {stdout}"
    );
    assert!(
        stdout.contains("PLAN (without Docker)"),
        "plano em EN: {stdout}"
    );
    assert!(
        stdout.contains("(--dry-run: nothing was executed.)"),
        "nota em EN: {stdout}"
    );
    assert!(
        !stdout.contains("Verificação") && !stdout.contains("sem Docker"),
        "sem pt no --lang en: {stdout}"
    );
}

#[test]
fn parse_and_policy_errors_follow_lang_flag() {
    // Composed parse errors are built in the current language.
    let project = js_project();
    let en = devforge()
        .env_remove("DEVFORGE_LANG")
        .args([
            "run",
            project.path().to_str().unwrap(),
            "--banana",
            "--lang",
            "en",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&en.stderr);
    assert!(
        stderr.contains("unknown option: --banana"),
        "stderr em EN: {stderr}"
    );

    let pt = devforge()
        .env_remove("DEVFORGE_LANG")
        .args(["run", project.path().to_str().unwrap(), "--banana"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&pt.stderr);
    assert!(
        stderr.contains("opção desconhecida: --banana"),
        "stderr em pt: {stderr}"
    );

    // audit --fail-on validation composes with the flag value.
    let en = devforge()
        .env_remove("DEVFORGE_LANG")
        .args([
            "audit",
            project.path().to_str().unwrap(),
            "--fail-on",
            "bogus",
            "--lang",
            "en",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&en.stderr);
    assert!(
        stderr.contains("--fail-on invalid: bogus (use low|medium|high|critical)"),
        "stderr em EN: {stderr}"
    );
}
