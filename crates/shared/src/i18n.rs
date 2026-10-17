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

/// pt-BR (key) → en. Keep sorted by the pt source for reviewability.
static EN: &[(&str, &str)] = &[
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
