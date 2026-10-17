//! Human-language selection for CLI messages (pt-BR + en).
//!
//! **Machine output never comes through here**: JSON keys/values, SARIF and
//! exit codes are always English (docs/design-i18n.md §1).
//!
//! Catalog model (gettext-style): the **pt-BR literal is the key** and the
//! `EN` table maps `pt-BR → en`. `t()` on `En` translates when an entry
//! exists and falls back to identity otherwise — a message migrated later
//! shows up untranslated instead of guessed. Translate FIRST, format after
//! (`t_fmt` replaces `{}` in order); formatting first would bake the values
//! into the key and never match.

use std::sync::OnceLock;

/// Supported UI languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    PtBr,
}

/// Language used when nothing asks for one.
///
/// **Migration status (docs/design-i18n.md):** stays `PtBr` until the phase-2
/// sweep migrates every human message; the final pre-launch commit flips this
/// to `En` (one line + test updates). Never flip with untranslated messages
/// still printing.
pub const DEFAULT_LANG: Lang = Lang::PtBr;

impl Lang {
    /// Parses a user-supplied tag (`--lang`, `DEVFORGE_LANG`).
    pub fn parse(raw: &str) -> Option<Lang> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "en" | "en-us" | "en-gb" => Some(Lang::En),
            "pt" | "pt-br" | "ptb" => Some(Lang::PtBr),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::PtBr => "pt-BR",
        }
    }
}

static CURRENT: OnceLock<Lang> = OnceLock::new();

/// Sets the process-wide language. First call wins (the CLI calls it once at
/// startup, before any output).
pub fn set_lang(lang: Lang) {
    let _ = CURRENT.set(lang);
}

/// Current language (defaults to [`DEFAULT_LANG`] if never set).
pub fn lang() -> Lang {
    *CURRENT.get().unwrap_or(&DEFAULT_LANG)
}

/// Resolves the language: `--lang` flag > `DEVFORGE_LANG` env > system
/// locale (`LC_ALL`/`LC_MESSAGES`/`LANG`) > default.
///
/// The locale can only opt **into** pt-BR (`pt`, `pt_BR`, `pt-BR`…); any
/// other locale falls through to [`DEFAULT_LANG`] — which is `PtBr` during
/// the migration and `En` after the pre-launch flip, yielding exactly the
/// ROADMAP rule "default en when the locale isn't BR". Invalid explicit
/// values are an error — never a silent fallback.
pub fn resolve(flag: Option<&str>) -> Result<Lang, String> {
    if let Some(raw) = flag {
        return Lang::parse(raw).ok_or_else(|| invalid_lang(raw));
    }
    if let Ok(raw) = std::env::var("DEVFORGE_LANG") {
        if !raw.trim().is_empty() {
            return Lang::parse(&raw).ok_or_else(|| invalid_lang(&raw));
        }
    }
    Ok(from_locale().unwrap_or(DEFAULT_LANG))
}

/// System locale → `PtBr` when it clearly asks for Portuguese, else `None`
/// (precedence: `LC_ALL` > `LC_MESSAGES` > `LANG`; empty = unset).
fn from_locale() -> Option<Lang> {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        let Ok(raw) = std::env::var(var) else {
            continue;
        };
        if raw.trim().is_empty() {
            continue;
        }
        let family = raw.split(['.', '_', '-']).next().unwrap_or("");
        return if family.eq_ignore_ascii_case("pt") {
            Some(Lang::PtBr)
        } else {
            None
        };
    }
    None
}

fn invalid_lang(raw: &str) -> String {
    format!("idioma inválido: '{raw}' (aceita: pt-BR | en — flag --lang ou env DEVFORGE_LANG)")
}

/// Translates `msg` under `current` (pure core — unit-testable without the
/// process-wide state).
pub fn translate(current: Lang, msg: &str) -> &str {
    match current {
        Lang::PtBr => msg, // pt-BR is the source: identity
        Lang::En => EN
            .iter()
            .find(|(source, _)| *source == msg)
            .map(|(_, translation)| *translation)
            .unwrap_or(msg), // not migrated yet: honest fallback
    }
}

/// Translates under the process-wide language.
pub fn t(msg: &str) -> &str {
    translate(lang(), msg)
}

/// Translates a `{}`-template, then substitutes `args` in order.
/// Templates without placeholders just pass through `t`.
pub fn t_fmt(msg: &str, args: &[&str]) -> String {
    let mut out = t(msg).to_string();
    for arg in args {
        out = out.replacen("{}", arg, 1);
    }
    out
}

/// `--help` body (pt-BR source). Kept as a const so the catalog key can
/// never drift from the emitted text (docs/design-i18n.md).
pub const HELP_PT: &str = "DevForge - agente de preparação de ambientes\n\n\
         USO:\n\
         \x20 devforge [caminho]                    detecta a stack (padrão)\n\
         \x20 devforge detect [caminho]             idem\n\
         \x20 devforge analyze [caminho] [flags]    analisa dependências (lockfile, versões)\n\
         \x20 devforge context [caminho] [flags]    briefing: runbook (build/test/run), entradas,\n\
         \x20               arquivos-chave e riscos — numa chamada só (bom pra IA)\n\
         \x20 devforge diagnose [flags]              resume um erro: causa real + conserto\n\
         \x20               (aceita --message \"...\", --input <arq> ou pipe: cmd 2>&1 | devforge diagnose)\n\
         \x20 devforge env [caminho] [flags]          fingerprint do ambiente: toolchains com versão,\n\
         \x20               presença no PATH e o que falta — 1 chamada (bom pra IA)\n\
         \x20 devforge run [caminho] [flags]         sobe o projeto: resolve o start (scripts.start,\n\
         \x20               Procfile, Makefile, convenção), espera ficar pronto e mostra o log;\n\
         \x20               ambiguidade pergunta; crash cedo vira diagnose; executa = pergunta\n\
         \x20               --detach: sobe em background e devolve o PID (sem supervisionar)\n\
         \x20 devforge up [url|caminho] [flags]     pipeline completo: clone? → audit → prepare → run\n\
         \x20               (mesmos gates de cada etapa isolada; --until/--skip controlam o corte;\n\
         \x20               url git é clonada com --depth 1 e sem hooks)\n\
         \x20 devforge audit [caminho] [flags]        auditoria de supply-chain (Socket/Snyk/OWASP):\n\
         \x20               risco por confluência de fatores; --fail-on gateia o CI (exit 1);\n\
         \x20               --policy/--ignore (TOML), --online pra checar hooks/downloads do npm\n\
         \x20 devforge license status               mostra a licença instalada (offline)\n\
         \x20 devforge license install <chave|@arq> valida (Ed25519, local) e instala\n\
         \x20 devforge license remove               remove a licença\n\
         \x20 devforge data status                 estado do dataset Pro (livre e offline;\n\
         \x20               sem dataset o audit segue igual — o tier grátis não muda)\n\
         \x20 devforge data update [flags]         baixa + verifica o dataset de risco fresco\n\
         \x20               (Pro: exige licença instalada e válida; recusa assinatura inválida)\n\
         \x20 devforge prepare [caminho] [flags]    instala dependências (sem Docker)\n\
         \x20 devforge docker  [caminho] [flags]    gera Dockerfile + build da imagem\n\n\
         FLAGS:\n\
         \x20 --dry-run    mostra o plano sem executar nada\n\
         \x20 --yes, -y    aprova tudo automaticamente\n\
         \x20 --force      permite sobrescrever arquivos existentes\n\
         \x20 --lang       idioma da saída humana: pt-BR | en\n\
         \x20 --online     (analyze/audit) consulta registries (versões / hooks)\n\
         \x20 --json       saída estruturada p/ IA (stdout = JSON puro; prepare/docker\n\
         \x20               entregam o plano, sem executar)\n\
         \x20 --all         desce em subpastas procurando projetos (monorepo /\n\
         \x20               manifesto aninhado); padrão é olhar só a raiz\n\
         \x20 --fail-on     (audit) severidade mínima que reprova o CI (exit 1):\n\
         \x20               low|medium|high|critical (padrão: high)\n\
         \x20 --policy      (audit) arquivo de política TOML (fail_on, allowlist)\n\
         \x20 --ignore      (audit) supressões TOML com expiração obrigatória\n\
         \x20 --sarif       (audit) emite SARIF 2.1.0 (GitHub Code Scanning)\n\
         \x20 --url         (data) origem do dataset: https://... ou file://<caminho>\n\
         \x20               (alternativa: env DF_DATASET_URL)\n\
         \x20 --timeout     (run) segundos esperando ficar pronto (padrão 60)\n\
         \x20 --health      (run) readiness explícito: tcp:<porta> | http://...\n\
         \x20 --detach      (run) sobe em background e devolve o PID (log: .devforge/run.log)\n\
         \x20 --until etapa (up) para depois da etapa: clone|audit|prepare|run\n\
         \x20 --skip etapas (up) pula etapa(s), separadas por vírgula\n\
         \x20 --            (run) repassa o resto ao comando de start (ex.: -- --port 3001)\n";

/// English twin of [`HELP_PT`] — catalog value, also never drifts.
pub const HELP_EN: &str = "DevForge - environment preparation agent (for humans and AIs)\n\n\
         USAGE:\n\
         \x20 devforge [path]                    detects the stack (default)\n\
         \x20 devforge detect [path]             same as above\n\
         \x20 devforge analyze [path] [flags]    analyzes dependencies (lockfile, versions)\n\
         \x20 devforge context [path] [flags]    briefing: runbook (build/test/run), entrypoints,\n\
         \x20               key files and risks — in a single call (AI-friendly)\n\
         \x20 devforge diagnose [flags]          summarizes an error: real cause + fix\n\
         \x20               (accepts --message \"...\", --input <file> or a pipe: cmd 2>&1 | devforge diagnose)\n\
         \x20 devforge env [path] [flags]        environment fingerprint: toolchains with versions,\n\
         \x20               PATH presence and what's missing — 1 call (AI-friendly)\n\
         \x20 devforge run [path] [flags]        boots the project: resolves start (scripts.start,\n\
         \x20               Procfile, Makefile, convention), waits until ready and shows the log;\n\
         \x20               ambiguity asks; early crash becomes diagnose; execution asks first\n\
         \x20               --detach: boots in background and returns the PID (no supervision)\n\
         \x20 devforge up [url|path] [flags]     full pipeline: clone? → audit → prepare → run\n\
         \x20               (same gates as each isolated step; --until/--skip control the cut;\n\
         \x20               git urls are cloned with --depth 1 and no hooks)\n\
         \x20 devforge audit [path] [flags]      supply-chain audit (Socket/Snyk/OWASP):\n\
         \x20               risk by factor confluence; --fail-on gates CI (exit 1);\n\
         \x20               --policy/--ignore (TOML), --online to check npm hooks/downloads\n\
         \x20 devforge license status            shows the installed license (offline)\n\
         \x20 devforge license install <key|@file> validates (Ed25519, local) and installs\n\
         \x20 devforge license remove            removes the license\n\
         \x20 devforge data status               state of the Pro dataset (free and offline;\n\
         \x20               without a dataset the audit works the same — the free tier never changes)\n\
         \x20 devforge data update [flags]       downloads + verifies the fresh risk dataset\n\
         \x20               (Pro: requires a valid installed license; refuses invalid signatures)\n\
         \x20 devforge prepare [path] [flags]    installs dependencies (no Docker)\n\
         \x20 devforge docker  [path] [flags]    generates a Dockerfile + builds the image\n\n\
         FLAGS:\n\
         \x20 --dry-run    shows the plan without executing anything\n\
         \x20 --yes, -y    approves everything automatically\n\
         \x20 --force      allows overwriting existing files\n\
         \x20 --lang       human-output language: pt-BR | en\n\
         \x20 --online     (analyze/audit) queries registries (versions / hooks)\n\
         \x20 --json       structured output for AIs (stdout = pure JSON; prepare/docker\n\
         \x20               deliver the plan, without executing)\n\
         \x20 --all         descends into subfolders looking for projects (monorepo /\n\
         \x20               nested manifests); default checks the root only\n\
         \x20 --fail-on     (audit) minimum severity that fails CI (exit 1):\n\
         \x20               low|medium|high|critical (default: high)\n\
         \x20 --policy      (audit) TOML policy file (fail_on, allowlist)\n\
         \x20 --ignore      (audit) TOML suppressions with mandatory expiry\n\
         \x20 --sarif       (audit) emits SARIF 2.1.0 (GitHub Code Scanning)\n\
         \x20 --url         (data) dataset origin: https://... or file://<path>\n\
         \x20               (alternative: env DF_DATASET_URL)\n\
         \x20 --timeout     (run) seconds to wait for readiness (default 60)\n\
         \x20 --health      (run) explicit readiness: tcp:<port> | http://...\n\
         \x20 --detach      (run) boots in background and returns the PID (log: .devforge/run.log)\n\
         \x20 --until step  (up) stops after the step: clone|audit|prepare|run\n\
         \x20 --skip steps (up) skips step(s), comma-separated\n\
         \x20 --            (run) passes the rest to the start command (e.g. -- --port 3001)\n";

/// pt-BR (key) → en. Keep sorted by the pt source for reviewability.
static EN: &[(&str, &str)] = &[
    // --- `--help` body (const HELP_PT — key can't drift from the text) ---
    (HELP_PT, HELP_EN),
    // --- section() headers (main.rs) ---
    ("AMBIENTE", "ENVIRONMENT"),
    ("ANÁLISE DE DEPENDÊNCIAS", "DEPENDENCY ANALYSIS"),
    ("ARQUIVOS-CHAVE", "KEY FILES"),
    ("AUDITORIA DE SUPPLY-CHAIN", "SUPPLY-CHAIN AUDIT"),
    ("COMO RODAR (runbook)", "HOW TO RUN (runbook)"),
    ("CONFIANÇA (resumo)", "TRUST (summary)"),
    (
        "DADOS FRESCOS (Pro — dataset assinado)",
        "FRESH DATA (Pro — signed dataset)",
    ),
    ("DIAGNÓSTICO", "DIAGNOSIS"),
    ("ENTRADAS", "ENTRYPOINTS"),
    ("FERRAMENTAS FALTANDO", "MISSING TOOLS"),
    (
        "INSTALAR TOOLCHAIN (com sua permissão)",
        "INSTALL TOOLCHAIN (with your permission)",
    ),
    ("PLANO DO UP", "UP PLAN"),
    ("RESULTADO", "RESULT"),
    ("RESULTADO DO UP", "UP RESULT"),
    ("STACKS DETECTADOS", "DETECTED STACKS"),
    (
        "VERIFICAÇÃO DE CONFIANÇA (pré-instalação)",
        "TRUST VERIFICATION (pre-install)",
    ),
    (
        "Workflow de detecção (dependency_scan)",
        "Detection workflow (dependency_scan)",
    ),
    // --- banner ---
    ("   projeto: {}", "   project: {}"),
    // --- `env` human output ---
    (
        "    • {} ❌ fora do PATH — sugestão: {}",
        "    • {} ❌ not on PATH — suggestion: {}",
    ),
    (
        "  nenhum stack detectado nesta pasta.",
        "  no stack detected in this folder.",
    ),
    (
        "\n(leitura apenas: checa o PATH e roda `<tool> --version` de ferramentas",
        "\n(read-only: checks the PATH and runs `<tool> --version` of tools",
    ),
    (
        "já instaladas; não instala nada e nunca lê arquivos .env)",
        "already installed; installs nothing and never reads .env files)",
    ),
    ("(versão não lida)", "(version unreadable)"),
    // --- permission prompts ---
    ("\n⚠️  RISCO DE SEGURANÇA: {}", "\n⚠️  SECURITY RISK: {}"),
    (
        "\n🔐 Permissão necessária: {}",
        "\n🔐 Permission required: {}",
    ),
    ("   ação   : {}", "   action   : {}"),
    ("   motivo : {}", "   reason   : {}"),
    (
        " [s]im / [n]ão / [a]sim para todas: ",
        " [y]es / [n]o / [a]lways to all: ",
    ),
    (" [s]im / [n]ão: ", " [y]es / [n]o: "),
    ("   Executar?{}", "   Proceed?{}"),
    (
        "   resposta inválida (use s, n ou a).",
        "   invalid answer (use y, n or a).",
    ),
    (
        "   resposta inválida (use s ou n).",
        "   invalid answer (use y or n).",
    ),
    // --- `up` step details ---
    (
        "git clone --depth 1 (sem submódulos, sem hooks)",
        "git clone --depth 1 (no submodules, no hooks)",
    ),
    (
        "devforge audit (relatório; não bloqueia o pipeline)",
        "devforge audit (report; doesn't block the pipeline)",
    ),
    (
        "devforge prepare (confiança + toolchain + instalação, com gates)",
        "devforge prepare (trust + toolchain + install, with gates)",
    ),
    (
        "devforge run (resolve o start; execução crítica, humano confirma)",
        "devforge run (resolves start; critical execution, human confirms)",
    ),
    // --- `audit` human output (main.rs: print_audit) ---
    ("Pacotes auditados: {}", "Packages audited: {}"),
    (
        "⚠ 0 pacotes auditados — não há o que atestar aqui",
        "⚠ 0 packages audited — there is nothing to certify here",
    ),
    (
        "   (manifesto sem dependências declaradas — ex.: projeto novo, ou aponte",
        "   (manifest without declared dependencies — e.g. a new project, or point",
    ),
    (
        "    o DevForge num crate/membro específico). O veredito NÃO atesta nada.",
        "    DevForge at a specific crate/member). The verdict certifies NOTHING.",
    ),
    ("Score (pior pacote): {}/100", "Score (worst package): {}/100"),
    (
        "(offline: hooks/downloads de registry não consultados — use --online)",
        "(offline: registry hooks/downloads not queried — use --online)",
    ),
    (
        "Nenhuma achação acima do limiar. ✅",
        "No findings above the threshold. ✅",
    ),
    (
        "• {} — {}  (score {}, confiança {})",
        "• {} — {}  (score {}, confidence {})",
    ),
    ("fatores: {}", "factors: {}"),
    ("Supressões ativas: {}", "Active suppressions: {}"),
    ("  - {} ({}) expira {}", "  - {} ({}) expires {}"),
    (
        "⚠ Supressões EXPIRADAS (re-triagem necessária): {}",
        "⚠ EXPIRED suppressions (re-triage needed): {}",
    ),
    ("  - {} ({}) expirou {}", "  - {} ({}) expired {}"),
    (
        "Veredito: SEM DADOS ⚠️  (0 pacotes — nada foi atestado)",
        "Verdict: NO DATA ⚠️  (0 packages — nothing was certified)",
    ),
    ("Veredito: {}", "Verdict: {}"),
    ("APROVADO ✅", "APPROVED ✅"),
    ("REPROVADO ❌", "REJECTED ❌"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_catalog_has_unique_non_empty_entries() {
        for (index, (source, target)) in EN.iter().enumerate() {
            assert!(!source.trim().is_empty(), "chave vazia no índice {index}");
            assert!(!target.trim().is_empty(), "tradução vazia: {source}");
            let duplicates = EN.iter().filter(|(other, _)| other == source).count();
            assert_eq!(duplicates, 1, "chave duplicada: {source}");
        }
    }

    #[test]
    fn placeholder_count_matches_between_source_and_translation() {
        // t_fmt substitutes `{}` positionally — a mismatch would drop args.
        for (source, target) in EN {
            assert_eq!(
                source.matches("{}").count(),
                target.matches("{}").count(),
                "placeholders divergem: {source} / {target}"
            );
        }
    }

    #[test]
    fn ptbr_is_identity_and_en_translates_known_messages() {
        assert_eq!(translate(Lang::PtBr, "AMBIENTE"), "AMBIENTE");
        assert_eq!(translate(Lang::En, "AMBIENTE"), "ENVIRONMENT");
        // Unmigrated message: honest fallback, never a guess.
        assert_eq!(
            translate(Lang::En, "mensagem ainda não migrada"),
            "mensagem ainda não migrada"
        );
    }

    #[test]
    fn t_fmt_translates_first_then_substitutes_in_order() {
        let out = t_fmt("    • {} ❌ fora do PATH — sugestão: {}", &["go"]);
        // Default lang is pt during migration → identity path with args filled.
        assert!(out.contains("go"), "arg substituído: {out}");
        let en = translate(Lang::En, "    • {} ❌ fora do PATH — sugestão: {}");
        assert!(en.contains("{}"), "template preservado na tradução");
    }

    #[test]
    fn resolution_order_is_flag_then_env_then_locale_then_default() {
        // Flag wins, invalid flag fails loudly.
        assert_eq!(resolve(Some("en")).unwrap(), Lang::En);
        assert_eq!(resolve(Some("pt-BR")).unwrap(), Lang::PtBr);
        assert!(
            resolve(Some("de")).is_err(),
            "valor inválido barra, nunca silencioso"
        );

        // Env var next (set here, removed after — this test owns the env
        // vars on purpose so the sub-asserts cannot race each other).
        std::env::set_var("DEVFORGE_LANG", "en");
        assert_eq!(resolve(None).unwrap(), Lang::En);
        std::env::remove_var("DEVFORGE_LANG");

        // Locale: `pt*` opts into pt-BR; a non-pt locale falls to the
        // default (which becomes `en` after the pre-launch flip).
        for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            std::env::remove_var(var);
        }
        std::env::set_var("LANG", "pt_BR.UTF-8");
        assert_eq!(resolve(None).unwrap(), Lang::PtBr, "locale BR → pt-BR");
        std::env::set_var("LANG", "en_US.UTF-8");
        assert_eq!(resolve(None).unwrap(), DEFAULT_LANG, "não-BR → default");
        std::env::set_var("LC_ALL", "pt_PT.UTF-8");
        assert_eq!(resolve(None).unwrap(), Lang::PtBr, "LC_ALL > LANG");
        std::env::remove_var("LC_ALL");
        std::env::remove_var("LANG");

        // Nothing set → default.
        assert_eq!(resolve(None).unwrap(), DEFAULT_LANG);
    }

    #[test]
    fn lang_parsing_accepts_common_tags_case_insensitively() {
        assert_eq!(Lang::parse("EN"), Some(Lang::En));
        assert_eq!(Lang::parse("pt-br"), Some(Lang::PtBr));
        assert_eq!(Lang::parse("  PT  "), Some(Lang::PtBr));
        assert_eq!(Lang::parse("fr"), None);
    }
}
