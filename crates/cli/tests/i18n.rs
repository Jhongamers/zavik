//! Integration tests for i18n (real binary): `--lang`, `DEVFORGE_LANG`,
//! the post-flip default (en), and the invariant that machine output
//! (JSON) never changes language (docs/design-i18n.md).

#![cfg(unix)]

use std::process::Command;
use tempfile::TempDir;

fn devforge() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_devforge"));
    // Locale must not flip the post-flip default (en) on any machine.
    cmd.env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("LANGUAGE")
        .env_remove("LANG");
    cmd
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
fn default_language_is_english_after_the_phase3_flip() {
    let project = js_project();
    let output = devforge()
        .env_remove("DEVFORGE_LANG") // the machine must not leak in
        .args(["env"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("ENVIRONMENT"),
        "default é EN desde o flip: {stdout}"
    );
    assert!(!stdout.contains("AMBIENTE"), "nada muda sem flag: {stdout}");
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
fn help_follows_the_lang_flag_with_english_default() {
    // Default (no flag): English since the phase-3 flip.
    let def = Command::new(env!("CARGO_BIN_EXE_devforge"))
        .arg("help")
        .output()
        .unwrap();
    assert!(def.status.success());
    let def = String::from_utf8_lossy(&def.stdout);
    assert!(def.contains("USAGE:"), "help default é EN: {def}");
    assert!(!def.contains("USO:"), "sem pt sem pedido: {def}");
    assert!(
        def.contains("--lang       human-output language: pt-BR | en"),
        "--lang documentado em EN: {def}"
    );

    // Explicit pt-BR: the same document fully Portuguese (chave = HELP_PT).
    let pt = Command::new(env!("CARGO_BIN_EXE_devforge"))
        .args(["help", "--lang", "pt-BR"])
        .output()
        .unwrap();
    assert!(pt.status.success());
    let pt = String::from_utf8_lossy(&pt.stdout);
    assert!(pt.contains("USO:"), "help em pt-BR: {pt}");
    assert!(
        pt.contains("--lang       idioma da saída humana: pt-BR | en"),
        "--lang documentado em pt-BR: {pt}"
    );

    // Explicit --lang en: same English document, no pt residue.
    let en = Command::new(env!("CARGO_BIN_EXE_devforge"))
        .args(["help", "--lang", "en"])
        .output()
        .unwrap();
    assert!(en.status.success());
    let en = String::from_utf8_lossy(&en.stdout);
    assert!(en.contains("USAGE:"), "help em EN: {en}");
    assert!(!en.contains("USO:"), "sem pt vazando no --lang en: {en}");
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

    // Explicit pt-BR keeps the pt path covered — the default is EN now.
    let pt = devforge()
        .env_remove("DEVFORGE_LANG")
        .args([
            "diagnose",
            "--message",
            "sh: 1: cargo: not found",
            "--lang",
            "pt-BR",
        ])
        .output()
        .unwrap();
    assert!(pt.status.success());
    let stdout = stdout_of(&pt);
    assert!(
        stdout.contains("DIAGNÓSTICO") && stdout.contains("  conserto:"),
        "pt-BR explícito continua pt: {stdout}"
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
    // Explicit pt-BR: plan section, target label and dry-run note stay pt.
    let project = js_project();
    let pt = devforge()
        .env_remove("DEVFORGE_LANG")
        .args([
            "up",
            project.path().to_str().unwrap(),
            "--dry-run",
            "--lang",
            "pt-BR",
        ])
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
    // Explicit pt-BR: trust line, plan section and dry-run note stay pt.
    let project = js_project();
    let pt = devforge()
        .env_remove("DEVFORGE_LANG")
        .args([
            "prepare",
            project.path().to_str().unwrap(),
            "--dry-run",
            "--lang",
            "pt-BR",
        ])
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
        .args([
            "run",
            project.path().to_str().unwrap(),
            "--banana",
            "--lang",
            "pt-BR",
        ])
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

/// Content-class strings (finding messages, audit evidence) are authored in
/// English and NEVER translated at runtime (docs/design-i18n.md §5): the UI
/// shell follows `--lang`, the facts stay English — same class as SARIF and
/// JSON prose. Runs in explicit pt-BR to prove the split even under pt.
#[test]
fn finding_and_evidence_content_is_english_even_in_ptbr() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name":"content-fixture","version":"1.0.0","dependencies":{"express":"*","lodash":"git+https://github.com/lodash/lodash.git"}}"#,
    )
    .unwrap();

    let analyze = devforge()
        .env_remove("DEVFORGE_LANG")
        .args(["analyze", "--lang", "pt-BR"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(analyze.status.success());
    let stdout = stdout_of(&analyze);
    assert!(
        stdout.contains("has no pinned version"),
        "finding content é classe EN: {stdout}"
    );
    assert!(
        stdout.contains("ANÁLISE DE DEPENDÊNCIAS"),
        "shell do analyze em pt-BR: {stdout}"
    );

    let audit = devforge()
        .env_remove("DEVFORGE_LANG")
        .args(["audit", "--lang", "pt-BR"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(audit.status.success());
    let stdout = stdout_of(&audit);
    assert!(
        stdout.contains("version not pinned (range/wildcard)"),
        "evidence é classe EN: {stdout}"
    );
    assert!(
        stdout.contains("AUDITORIA DE SUPPLY-CHAIN"),
        "shell do audit em pt-BR: {stdout}"
    );
}
