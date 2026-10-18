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
/// **Migration status (docs/design-i18n.md):** flipped to `En` in the
/// pre-launch phase-3 commit — every human message goes through the catalog
/// and the content class (evidence/finding/risk) is authored in EN, so no pt
/// text leaks under the new default. `--lang pt-BR`, `DEVFORGE_LANG=pt-BR`
/// and a BR system locale still select pt-BR.
pub const DEFAULT_LANG: Lang = Lang::En;

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
/// other locale falls through to [`DEFAULT_LANG`] — `En` since the
/// pre-launch flip, yielding exactly the ROADMAP rule "default en when the
/// locale isn't BR". Invalid explicit values are an error — never a silent
/// fallback.
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
    t_fmt_in(lang(), msg, args)
}

/// [`t_fmt`] under an explicit language: for text composed at construction
/// (diagnosis summaries), where the caller knows the target language.
pub fn t_fmt_in(lang: Lang, msg: &str, args: &[&str]) -> String {
    let mut out = translate(lang, msg).to_string();
    for arg in args {
        out = out.replacen("{}", arg, 1);
    }
    out
}

/// English-forced translation, ignoring the current language: prose that
/// lands in JSON always comes out English, whatever `--lang` says
/// (docs/design-i18n.md §4 — dogfooding finding #6: machine output never
/// switches language; JSON prose included). Unknown keys stay identity,
/// same honest fallback as [`t`].
pub fn t_en(msg: &str) -> &str {
    translate(Lang::En, msg)
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
    // --- `diagnose` (plugins/diagnose.rs: rules, summaries, dynamic fixes) ---
    // Rule causes.
    (
        "Uma ferramenta necessária não está instalada no PATH.",
        "A required tool is not installed on the PATH.",
    ),
    (
        "Falta o pacote de DESENVOLVIMENTO (cabeçalho/lib de link) de uma biblioteca nativa.",
        "The DEVELOPMENT package (header/linking lib) of a native library is missing.",
    ),
    (
        "Falta a toolchain de compilação C/C++ (necessária pra módulos nativos).",
        "The C/C++ compilation toolchain is missing (needed for native modules).",
    ),
    (
        "Erro de compilação Rust: o compilador apontou o arquivo/linha do problema.",
        "Rust compilation error: the compiler pointed at the file/line of the problem.",
    ),
    (
        "Módulo nativo do npm precisa de ferramentas de build (python3 + gcc + make).",
        "An npm native module needs build tools (python3 + gcc + make).",
    ),
    (
        "Um módulo Python não está instalado no ambiente atual.",
        "A Python module is not installed in the current environment.",
    ),
    (
        "Dependência Go faltando no módulo (não está no go.mod/go.sum).",
        "Missing Go dependency in the module (not in go.mod/go.sum).",
    ),
    (
        "Classe compilada com um JDK mais novo que o que está rodando.",
        "The class was compiled with a newer JDK than the one running.",
    ),
    (
        "A porta que o serviço quer usar já está ocupada por outro processo.",
        "The port the service wants to use is already taken by another process.",
    ),
    (
        "Sem permissão de escrita/execução no caminho em questão.",
        "No write/execute permission on the path in question.",
    ),
    (
        "O npm está tentando escrever em pastas globais sem permissão.",
        "npm is trying to write to global folders without permission.",
    ),
    (
        "Falha de rede/registry (offline, proxy, TLS ou DNS).",
        "Network/registry failure (offline, proxy, TLS or DNS).",
    ),
    ("O processo ficou sem memória.", "The process ran out of memory."),
    ("Disco cheio.", "Disk full."),
    (
        "Falta uma variável de ambiente / config esperada pelo projeto.",
        "An environment variable / config expected by the project is missing.",
    ),
    (
        "A classe principal não foi encontrada (classpath ou nome errado).",
        "The main class was not found (classpath or wrong name).",
    ),
    (
        "Panic/crash no próprio código (bug, não ambiente).",
        "Panic/crash in the code itself (bug, not the environment).",
    ),
    // Rule fixes (the pure-code one, Debian/Ubuntu apt line, needs no entry).
    (
        "instale a ferramenta (ou rode `devforge prepare` pra montar a toolchain).",
        "install the tool (or run `devforge prepare` to set up the toolchain).",
    ),
    (
        "instale o pacote -dev/-devel correspondente e rode de novo.",
        "install the matching -dev/-devel package and run again.",
    ),
    (
        "corrija a linha citada pelo rustc; detalhes do código em `cargo explain <código>` (ex.: `cargo explain E0308`).",
        "fix the line the rustc pointed at; code details in `cargo explain <code>` (e.g. `cargo explain E0308`).",
    ),
    (
        "instale python3 e build-essential e rode `npm install` de novo.",
        "install python3 and build-essential and run `npm install` again.",
    ),
    (
        "instale o módulo (pip install <nome>) no mesmo venv.",
        "install the module (pip install <name>) in the same venv.",
    ),
    (
        "rode `go mod tidy` ou `go get <pacote>`.",
        "run `go mod tidy` or `go get <package>`.",
    ),
    (
        "alinhe as versões: compile e rode com o mesmo JDK.",
        "align the versions: build and run with the same JDK.",
    ),
    (
        "mude a porta ou encerre o processo que a usa.",
        "change the port or kill the process using it.",
    ),
    (
        "verifique dono/permissões do diretório (evite `sudo` global, sobretudo em node_modules).",
        "check the directory owner/permissions (avoid global `sudo`, especially in node_modules).",
    ),
    (
        "evite `sudo npm`; use um gerenciador de versão (nvm/fnm) ou instalação local.",
        "avoid `sudo npm`; use a version manager (nvm/fnm) or a local install.",
    ),
    (
        "verifique conexão/proxy/CA — alguns registries (ex.: crates.io) exigem User-Agent.",
        "check connection/proxy/CA — some registries (e.g. crates.io) require a User-Agent.",
    ),
    (
        "feche processos, aumente o limite ou (node) use `--max-old-space-size`.",
        "close processes, raise the limit or (node) use `--max-old-space-size`.",
    ),
    (
        "libere espaço em disco e tente de novo.",
        "free up disk space and try again.",
    ),
    (
        "defina a variável (veja `.env.example` ou o README).",
        "set the variable (see `.env.example` or the README).",
    ),
    (
        "confira o classpath e o nome fully-qualified da classe.",
        "check the classpath and the fully-qualified class name.",
    ),
    (
        "leia o backtrace; a causa está no código, não na toolchain.",
        "read the backtrace; the cause is in the code, not the toolchain.",
    ),
    // Rule notes ("the literal error is just the symptom").
    (
        "O erro de linking/cabeçalho é só o SINTOMA — a causa real é o pacote de dev ausente.",
        "The linking/header error is only the SYMPTOM — the real cause is the missing dev package.",
    ),
    (
        "'could not compile ... due to N previous errors' é só o resumo — a causa é o primeiro `error[E...]` listado.",
        "'could not compile ... due to N previous errors' is just the summary — the cause is the first `error[E...]` listed.",
    ),
    (
        "O 'node-gyp ERR' é consequência — a causa é a toolchain de build ausente.",
        "The 'node-gyp ERR' is a consequence — the cause is the missing build toolchain.",
    ),
    // Dynamic fixes (translated before the values are substituted).
    (
        "instale o pacote de dev da lib `{}` (Debian: lib{}-dev, Fedora: {}-devel) e rode de novo",
        "install the dev package for the lib `{}` (Debian: lib{}-dev, Fedora: {}-devel) and run again",
    ),
    (
        "pip install {} (no mesmo venv/ambiente do projeto)",
        "pip install {} (in the project's same venv/environment)",
    ),
    (
        "`{}` não está no PATH — instale ou rode `devforge prepare` pra toolchain",
        "`{}` is not on the PATH — install it or run `devforge prepare` for the toolchain",
    ),
    // Summaries (composed: translate first, then fill in order).
    (
        "Reconheci {} padrão(ões); o mais provável: {}",
        "Recognized {} pattern(s); the most likely: {}",
    ),
    (
        "Não reconheci o padrão do erro — seguem as linhas com mais cara de erro.",
        "I did not recognize the error's pattern — here are the lines that look most like errors.",
    ),
    // --- `diagnose` human labels (main.rs: print_diagnosis) ---
    ("  nota: {}", "  note: {}"),
    ("  conserto: {}", "  fix: {}"),
    ("  provas:", "  evidence:"),
    (
        "Linhas com mais cara de erro:",
        "Lines that look most like errors:",
    ),
    // --- `diagnose` input error (main.rs: read_error_input) ---
    (
        "cole a saída do erro, ou use --message \"...\", --input <arquivo> (ou pipe: `meu_comando 2>&1 | devforge diagnose`)",
        "paste the error output, or use --message \"...\", --input <file> (or pipe: `my_command 2>&1 | devforge diagnose`)",
    ),
    // --- `run` human output (main.rs: run_run + helpers) ---
    ("Não achei como iniciar este projeto.", "I couldn't find how to start this project."),
    (
        "   linguagens detectadas: {}",
        "   detected languages: {}",
    ),
    (
        "   declare um start (ex.: package.json scripts.start, Procfile, Makefile com alvo 'run')",
        "   declare a start (e.g. package.json scripts.start, Procfile, Makefile with a 'run' target)",
    ),
    (
        "   (cancelado — nada foi executado)",
        "   (cancelled — nothing was executed)",
    ),
    ("\nPlano de execução:", "\nExecution plan:"),
    ("   comando : {}", "   command : {}"),
    ("   origem  : {}", "   source  : {}"),
    (
        "   (dry-run: nada foi executado)",
        "   (dry-run: nothing was executed)",
    ),
    ("Executar o projeto", "Run the project"),
    (
        "roda código do projeto local com as credenciais do seu usuário",
        "runs the project's code locally with your user credentials",
    ),
    (
        "   ✗ negado (nada foi executado). --yes não cobre execução de código.",
        "   ✗ denied (nothing was executed). --yes does not cover code execution.",
    ),
    (
        "não preparei o diretório de log {}: {}",
        "could not prepare the log directory {}: {}",
    ),
    (
        "não foi possível executar '{}': {}",
        "could not execute '{}': {}",
    ),
    (
        "\n▶ rodando em background: {}",
        "\n▶ running in background: {}",
    ),
    (
        "   log  : {}   (tail -f pra acompanhar)",
        "   log  : {}   (tail -f to follow)",
    ),
    (
        "   parar: kill {}   (se precisar: kill -9 {})",
        "   stop: kill {}   (if needed: kill -9 {})",
    ),
    (
        "\n   (detached não espera ficar pronto nem supervisiona — pra isso",
        "\n   (detached doesn't wait for readiness nor supervises — for that",
    ),
    (
        "    rode `devforge run` sem --detach, com health check e diagnose)",
        "    run `devforge run` without --detach, with health check and diagnose)",
    ),
    (
        "    no Windows o processo continua ligado ao console (limitação conhecida)",
        "    on Windows the process stays attached to the console (known limitation)",
    ),
    ("\n▶ rodando: {}", "\n▶ running: {}"),
    (
        "   (Ctrl-C encerra: SIGTERM → espera → SIGKILL)",
        "   (Ctrl-C stops: SIGTERM → wait → SIGKILL)",
    ),
    (
        "\n✨ terminou normalmente em {}s{}",
        "\n✨ finished normally in {}s{}",
    ),
    (
        "\n❌ o projeto saiu com código {} após {}s{}",
        "\n❌ the project exited with code {} after {}s{}",
    ),
    (" (pronto na porta {})", " (ready on port {})"),
    (
        "\n⏱  não ficou \"pronto\" em {}s — processo derrubado.",
        "\n⏱  didn't become \"ready\" within {}s — process killed.",
    ),
    ("   últimas linhas:", "   last lines:"),
    (
        "   (prontidão = heurística de porta; use --health se o serviço é lento)",
        "   (readiness = port heuristic; use --health if the service is slow)",
    ),
    (
        "\n■ interrompido{} em {}s",
        "\n■ interrupted{} in {}s",
    ),
    (
        "   Vários pontos de entrada — escolha um:",
        "   Several entrypoints — pick one:",
    ),
    (
        "   número (vazio = cancelar): ",
        "   number (empty = cancel): ",
    ),
    ("   opção inválida.", "   invalid option."),
    // --- `audit` dataset section (main.rs: print_dataset_section) ---
    (
        "dataset {} de {} (idade: {} dia(s); {} advisories na janela, {} na kill list)",
        "dataset {} of {} (age: {} day(s); {} advisories in the window, {} on the kill list)",
    ),
    (
        "⚠ atrasado (>{} dias) — rode: devforge data update",
        "⚠ stale (>{} days) — run: devforge data update",
    ),
    (
        "\nNenhum achado novo nas dependências deste projeto.",
        "\nNo new findings in this project's dependencies.",
    ),
    (
        "\n🛑 Nome malicioso/typosquat presente nas dependências:",
        "\n🛑 Malicious/typosquat name present in the dependencies:",
    ),
    (
        "\n⚠ Possível exposição (casamento só por nome — confira sua versão):",
        "\n⚠ Possible exposure (name-only match — check your version):",
    ),
    (
        "\n(seção informativa: não altera o score nem o --fail-on do audit)",
        "\n(informational section: doesn't change the score or the audit's --fail-on)",
    ),
    // --- `data` command (main.rs: run_data/data_status/data_update/fetch_dataset) ---
    (
        "--url exige um valor (<https://...> ou file://<caminho>)",
        "--url requires a value (<https://...> or file://<path>)",
    ),
    (
        "opção desconhecida: {} (uso: devforge data status|update [--json] [--url <url>])",
        "unknown option: {} (usage: devforge data status|update [--json] [--url <url>])",
    ),
    (
        "subcomando desconhecido: {} (use: status | update)",
        "unknown subcommand: {} (use: status | update)",
    ),
    ("dataset instalado ✅", "dataset installed ✅"),
    (
        "   versão:  {} (gerado em {})",
        "   version:  {} (generated on {})",
    ),
    (
        "   idade:   {} dia(s) — {} advisories na janela + {} na kill list",
        "   age:   {} day(s) — {} advisories in the window + {} on the kill list",
    ),
    ("\n(fresco)", "\n(fresh)"),
    (
        "audit segue offline com o motor embutido; dados frescos são Pro (devforge data update)",
        "audit keeps running offline with the built-in engine; fresh data is Pro (devforge data update)",
    ),
    (
        "nenhum dataset instalado ({})",
        "no dataset installed ({})",
    ),
    (
        "   grátis: o audit usa o motor offline — nada muda no tier gratuito.",
        "   free: audit uses the offline engine — nothing changes in the free tier.",
    ),
    (
        "   Pro:    devforge data update (dados de risco frescos, exige licença).",
        "   Pro:    devforge data update (fresh risk data, requires a license).",
    ),
    (
        "dataset instalado, mas INVÁLIDO: {}",
        "dataset installed, but INVALID: {}",
    ),
    (
        "   nada é usado sem assinatura válida — rode devforge data update.",
        "   nothing is used without a valid signature — run devforge data update.",
    ),
    (
        "(dataset instalado INVÁLIDO — ignorado: {})",
        "(dataset installed INVALID — ignored: {})",
    ),
    (
        "nenhuma licença instalada",
        "no license installed",
    ),
    (
        "⛔ Recurso Pro (dataset de risco fresco): {}.",
        "⛔ Pro feature (fresh risk dataset): {}.",
    ),
    (
        "   Instale sua licença:  devforge license install <token>",
        "   Install your license:  devforge license install <token>",
    ),
    (
        "   (confira:             devforge license status)",
        "   (check:               devforge license status)",
    ),
    (
        "   Sem licença o audit segue grátis e offline — o tier gratuito não muda.",
        "   Without a license audit stays free and offline — the free tier doesn't change.",
    ),
    (
        "URL do dataset não configurada nesta build (use --url <url> ou env DF_DATASET_URL)",
        "dataset URL not configured in this build (use --url <url> or env DF_DATASET_URL)",
    ),
    ("baixando dataset de {} ...", "downloading dataset from {} ..."),
    (
        "   dados:   {} advisories na janela + {} nomes na kill list",
        "   data:   {} advisories in the window + {} names on the kill list",
    ),
    (
        "\nO audit desta máquina já usa os dados frescos (offline até o próximo update).",
        "\nThis machine's audit already uses the fresh data (offline until the next update).",
    ),
    ("❌ dataset recusado: {}", "❌ dataset refused: {}"),
    (
        "   (nada foi instalado — o dataset anterior, se houver, permanece)",
        "   (nothing was installed — the previous dataset, if any, remains)",
    ),
    (
        "não li o dataset local {}: {}",
        "could not read the local dataset {}: {}",
    ),
    (
        "origem não suportada (use https:// ou file://): {}",
        "unsupported origin (use https:// or file://): {}",
    ),
    ("falha ao baixar de {}: {}", "failed to download from {}: {}"),
    (
        "falha ao ler a resposta de {}: {}",
        "failed to read the response from {}: {}",
    ),
    (
        "resposta acima de {} bytes — recusada",
        "response above {} bytes — refused",
    ),
    // --- `license` command (main.rs: run_license + license.rs Display) ---
    (
        "nenhuma licença instalada ({})",
        "no license installed ({})",
    ),
    ("licença VÁLIDA", "license VALID"),
    ("   plano:    {}", "   plan:    {}"),
    ("   titular:  {}", "   holder:  {}"),
    ("   emitida:  {}", "   issued:  {}"),
    ("   expira:   {}", "   expires:  {}"),
    (
        "licença instalada, mas INVÁLIDA: {}",
        "license installed, but INVALID: {}",
    ),
    (
        "uso: devforge license install <chave|@arquivo>",
        "usage: devforge license install <key|@file>",
    ),
    ("licença rejeitada: {}", "license rejected: {}"),
    (
        "   ✓ licença instalada: {} (plano {}, expira {})",
        "   ✓ license installed: {} (plan {}, expires {})",
    ),
    ("   ✓ licença removida.", "   ✓ license removed."),
    (
        "   (não havia licença instalada)",
        "   (no license was installed)",
    ),
    (
        "subcomando desconhecido: {} (status|install|remove)",
        "unknown subcommand: {} (status|install|remove)",
    ),
    (
        "formato inválido (esperado DF1.<payload>.<assinatura>)",
        "invalid format (expected DF1.<payload>.<signature>)",
    ),
    ("base64 inválido", "invalid base64"),
    ("payload não é JSON válido", "payload is not valid JSON"),
    ("assinatura não confere", "signature doesn't match"),
    ("licença vencida em {}", "license expired on {}"),
    // --- SARIF is machine output: always English (docs/design-i18n.md) ---
    (
        "{} — score {} (confiança {}). Fatores: {}. {}",
        "{} — score {} (confidence {}). Factors: {}. {}",
    ),
    (
        "Script de instalação (executa código no install)",
        "Install script (runs code on install)",
    ),
    ("Acesso à rede no install", "Network access on install"),
    ("Acesso à shell no install", "Shell access on install"),
    (
        "Escrita em filesystem sensível no install",
        "Sensitive filesystem write on install",
    ),
    (
        "Leitura de variáveis de ambiente no install",
        "Environment variable read on install",
    ),
    ("Código ofuscado no install", "Obfuscated code on install"),
    (
        "Dependência fora do registro (git/path/file)",
        "Dependency outside the registry (git/path/file)",
    ),
    (
        "Pacote com baixa exposição (downloads)",
        "Package with low exposure (downloads)",
    ),
    (
        "Versão não fixada (range/coringa)",
        "Unpinned version (range/wildcard)",
    ),
    ("Sem hash de integridade", "No integrity hash"),
    // --- `DatasetError` (dataset.rs Display → `message_in`) ---
    (
        "chave pública do dataset não configurada nesta build",
        "dataset public key not configured in this build",
    ),
    (
        "formato do dataset inválido (envelope malformado)",
        "invalid dataset format (malformed envelope)",
    ),
    (
        "assinatura do dataset inválida — arquivo adulterado ou de chave desconhecida; recusado",
        "invalid dataset signature — tampered file or unknown key; refused",
    ),
    (
        "schema do dataset não suportado: {} (este build entende {})",
        "unsupported dataset schema: {} (this build understands {})",
    ),
    (
        "conteúdo do dataset inválido (JSON ou campos)",
        "invalid dataset content (JSON or fields)",
    ),
    (
        "dataset acima do limite de {} bytes — recusado",
        "dataset above the limit of {} bytes — refused",
    ),
    ("erro de arquivo: {}", "file error: {}"),
    // --- Toolchain hints (core/provision run_hints + core/toolchain) ---
    (
        "python3 main.py  (ou o seu entrypoint)",
        "python3 main.py  (or your entrypoint)",
    ),
    (
        "mvn spring-boot:run  (ou java -jar target/*.jar)",
        "mvn spring-boot:run  (or java -jar target/*.jar)",
    ),
    (
        "\nPara rodar depois de preparado:",
        "\nTo run after preparation:",
    ),
    (
        "instale '{}' (pacote sugerido: {})",
        "install '{}' (suggested package: {})",
    ),
    // --- Provision steps (core/provision local + docker; built in the
    //     target language — composed titles never match keys at emit) ---
    (
        "Baixar e compilar dependências (cargo)",
        "Download and compile dependencies (cargo)",
    ),
    (
        "Compila o projeto e baixa todas as crates declaradas.",
        "Compiles the project and downloads every declared crate.",
    ),
    ("Instalar dependências ({})", "Install dependencies ({})"),
    (
        "Baixa e instala os pacotes do package.json.",
        "Downloads and installs the packages in package.json.",
    ),
    (
        "Baixar módulos (go mod download)",
        "Download modules (go mod download)",
    ),
    (
        "Baixa os módulos declarados no go.mod.",
        "Downloads the modules declared in go.mod.",
    ),
    (
        "Instalar dependências (composer install)",
        "Install dependencies (composer install)",
    ),
    (
        "Instala os pacotes do composer.json.",
        "Installs the packages in composer.json.",
    ),
    (
        "Instalar gems (bundle install)",
        "Install gems (bundle install)",
    ),
    ("Instala as gems do Gemfile.", "Installs the gems in the Gemfile."),
    ("Restaurar pacotes (.NET)", "Restore packages (.NET)"),
    (
        "Baixa os pacotes NuGet do projeto.",
        "Downloads the project's NuGet packages.",
    ),
    ("Compilar projeto (.NET)", "Build project (.NET)"),
    (
        "Compila a solução em modo Debug.",
        "Builds the solution in Debug mode.",
    ),
    (
        "Baixar pacotes ({} pub get)",
        "Download packages ({} pub get)",
    ),
    (
        "Baixa as dependências do pubspec.yaml.",
        "Downloads the dependencies in pubspec.yaml.",
    ),
    (
        "Baixar dependências (mix deps.get)",
        "Download dependencies (mix deps.get)",
    ),
    (
        "Baixa as dependências do mix.exs.",
        "Downloads the dependencies in mix.exs.",
    ),
    ("Compilar (mix compile)", "Compile (mix compile)"),
    ("Compila o projeto Elixir.", "Compiles the Elixir project."),
    (
        "Compilar pacote (swift build)",
        "Build package (swift build)",
    ),
    (
        "Resolve e compila as dependências do Package.swift.",
        "Resolves and compiles the Package.swift dependencies.",
    ),
    ("Configurar build (cmake)", "Configure build (cmake)"),
    (
        "Gera os arquivos de build na pasta build/.",
        "Generates the build files in the build/ folder.",
    ),
    ("Compilar (cmake --build)", "Build (cmake --build)"),
    ("Compila o projeto.", "Builds the project."),
    (
        "Instalar dependências (poetry install)",
        "Install dependencies (poetry install)",
    ),
    (
        "Cria o ambiente virtual e instala as dependências.",
        "Creates the virtual environment and installs the dependencies.",
    ),
    (
        "Instalar dependências (pipenv install)",
        "Install dependencies (pipenv install)",
    ),
    (
        "Criar ambiente virtual (python venv)",
        "Create virtual environment (python venv)",
    ),
    (
        "Cria a pasta .venv isolando as dependências do sistema.",
        "Creates the .venv folder, isolating dependencies from the system.",
    ),
    (
        "Instalar dependências (pip install)",
        "Install dependencies (pip install)",
    ),
    (
        "Instala as dependências do projeto no ambiente virtual.",
        "Installs the project dependencies inside the virtual environment.",
    ),
    ("Compilar (gradle build)", "Build (gradle build)"),
    (
        "Baixa as dependências e compila o projeto.",
        "Downloads the dependencies and builds the project.",
    ),
    (
        "Baixar dependências (maven)",
        "Download dependencies (maven)",
    ),
    (
        "Resolve as dependências e compila sem rodar testes.",
        "Resolves the dependencies and builds without running tests.",
    ),
    ("Verificar Docker", "Check Docker"),
    (
        "Confere se o Docker está instalado antes de gerar/buildar imagens.",
        "Checks that Docker is installed before generating/building images.",
    ),
    ("Gerar {}", "Generate {}"),
    (
        "Cria o Dockerfile para o stack {} usando a imagem base oficial.",
        "Creates the Dockerfile for the {} stack using the official base image.",
    ),
    ("Construir imagem {}", "Build image {}"),
    (
        "Executa o `docker build` (pode baixar imagens e demorar).",
        "Runs `docker build` (may download images and take a while).",
    ),
    // --- `up` pipeline body (plan, clone, summary) ---
    ("Alvo: {}\n", "Target: {}\n"),
    ("   (pulado por flag)", "   (skipped by flag)"),
    ("(pulado)", "(skipped)"),
    (
        "\n(--dry-run: nada foi executado.)",
        "\n(--dry-run: nothing was executed.)",
    ),
    (
        "UP — ETAPA {} DE {}: {}",
        "UP — STEP {} OF {}: {}",
    ),
    (
        "clone sem URL (inconsistência interna do plano)",
        "clone without URL (internal plan inconsistency)",
    ),
    ("✅ clonado em {}", "✅ cloned into {}"),
    (
        "\n✨ Pipeline concluído.",
        "\n✨ Pipeline completed.",
    ),
    (
        "\n⏹️ Pipeline interrompido — nada além do ponto de aborto foi executado.",
        "\n⏹️ Pipeline interrupted — nothing past the abort point was executed.",
    ),
    // clone_repo errors (composed — built in the target language; they also
    // reach the JSON `"error"` field, so construction-lang matters here).
    (
        "o destino já existe: {} — nada foi clonado (remova ou use outro alvo)",
        "destination already exists: {} — nothing was cloned (remove it or use another target)",
    ),
    (
        "não consegui executar git: {}",
        "could not execute git: {}",
    ),
    (
        "git clone falhou ({})",
        "git clone failed ({})",
    ),
    // up.rs plan/derive errors (human stderr via the `?` handler).
    (
        "não consegui derivar o nome do repositório de: {}",
        "could not derive the repository name from: {}",
    ),
    (
        "--skip: etapa desconhecida: {} (válidas: {})",
        "--skip: unknown step: {} (valid: {})",
    ),
    (
        "--until: etapa desconhecida: {} (válidas: {})",
        "--until: unknown step: {} (valid: {})",
    ),
    (
        "--skip clone não é suportado: sem URL não há o que clonar; com URL o clone é obrigatório",
        "--skip clone is not supported: without a URL there is nothing to clone; with a URL the clone is mandatory",
    ),
    (
        "--until clone exige uma URL (sem URL não há etapa de clone)",
        "--until clone requires a URL (without a URL there is no clone step)",
    ),
    // --- analyze / context / shared printers ---
    (
        "plugin 'dependency_analyzer' não registrado",
        "plugin 'dependency_analyzer' not registered",
    ),
    (
        "\nConsultando registries (crates.io / npm / PyPI)… isso pode demorar.",
        "\nQuerying registries (crates.io / npm / PyPI)… this may take a while.",
    ),
    (
        "\nResumo: {} crítico(s), {} aviso(s), {} informação(ões).",
        "\nSummary: {} critical, {} warning(s), {} info.",
    ),
    (
        "Nenhum stack reconhecido.",
        "No stack recognized.",
    ),
    (
        "Nenhum manifesto reconhecido neste diretório.",
        "No manifest recognized in this directory.",
    ),
    (
        "\n• Linguagem : {}   |   Gerenciador: {}   |   Nome: {}",
        "\n• Language : {}   |   Manager: {}   |   Name: {}",
    ),
    ("  Manifesto : {}", "  Manifest : {}"),
    ("  Dependências: {}", "  Dependencies: {}"),
    (
        "✅ Nada a reportar: dependências e lockfiles parecem saudáveis.",
        "✅ Nothing to report: dependencies and lockfiles look healthy.",
    ),
    ("❌ A execução parou: {}", "❌ Execution stopped: {}"),
    (
        "   Corrija o problema e rode novamente.",
        "   Fix the problem and run again.",
    ),
    ("Nada foi executado.", "Nothing was executed."),
    (
        "\n✨ Concluído: {} passo(s) executado(s), {} pulado(s).",
        "\n✨ Done: {} step(s) executed, {} skipped.",
    ),
    // --- prepare section headers / mode label ---
    ("PLANO ({})", "PLAN ({})"),
    ("sem Docker", "without Docker"),
    ("com Docker", "with Docker"),
    // --- prepare: trust pre-flight ---
    (
        "   (verificação de confiança falhou, seguindo): {}",
        "   (trust verification failed, proceeding): {}",
    ),
    (
        "\n✅ Verificação de confiança: nenhum risco detectado.",
        "\n✅ Trust verification: no risk detected.",
    ),
    (
        "O DevForge não confia cegamente no manifest. Riscos encontrados:\n",
        "DevForge doesn't trust the manifest blindly. Risks found:\n",
    ),
    (
        "\n(Só avisos informativos — seguindo.)",
        "\n(Only informational warnings — proceeding.)",
    ),
    (
        "\n(--dry-run: nada será executado, então os riscos acima só foram exibidos.)",
        "\n(--dry-run: nothing will be executed, so the risks above were only displayed.)",
    ),
    (
        "\nOs {} risco(s) abaixo exigem confirmação EXPLÍCITA cada um ('sim para todas' e --yes NÃO valem aqui).",
        "\nThe {} risk(s) below require EXPLICIT confirmation each ('yes to all' and --yes do NOT apply here).",
    ),
    ("Aceitar risco: {}", "Accept risk: {}"),
    // --- prepare: missing tools + toolchain install ---
    (
        "Estas ferramentas não estão no PATH e serão necessárias pra rodar o projeto:\n",
        "These tools are not on the PATH and will be needed to run the project:\n",
    ),
    ("      sugestão: {}", "      suggestion: {}"),
    (
        "\nInstale manualmente e rode de novo. As sugestões são por gerenciador",
        "\nInstall manually and run again. The suggestions are per package",
    ),
    (
        "de pacotes — sua máquina pode variar.",
        "manager — your machine may vary.",
    ),
    ("Instalar {} ({})", "Install {} ({})"),
    (
        "Vou rodar (sem shell, argumento a argumento):\n  {}\nIsso altera seu sistema e pode pedir sua senha. Recuse se não confiar.",
        "I'll run (no shell, argument by argument):\n  {}\nThis changes your system and may ask for your password. Decline if you don't trust it.",
    ),
    ("{} não está no PATH", "{} is not on the PATH"),
    (
        "   {} — deixado de lado (você optou por não instalar).",
        "   {} — set aside (you chose not to install).",
    ),
    (
        "\n▶ Instalando {} — siga o prompt se ele pedir senha...",
        "\n▶ Installing {} — follow the prompt if it asks for a password...",
    ),
    ("   ✓ {} instalado.", "   ✓ {} installed."),
    (
        "   ✗ '{}' terminou com exit {}.",
        "   ✗ '{}' finished with exit {}.",
    ),
    (
        "   ✗ não consegui rodar: {}",
        "   ✗ couldn't run: {}",
    ),
    // --- prepare: body + execution section ---
    (
        "Nenhum stack reconhecido — nada a preparar.",
        "No stack recognized — nothing to prepare.",
    ),
    (
        "\n✅ Toolchain instalada — seguindo.",
        "\n✅ Toolchain installed — proceeding.",
    ),
    (
        "\n⏹️  Abortado por risco de segurança. Nada foi instalado nem buildado.",
        "\n⏹️  Aborted by security risk. Nothing was installed or built.",
    ),
    (
        "plugin 'file_system_access' não registrado",
        "plugin 'file_system_access' not registered",
    ),
    (
        "plugin 'command_runner' não registrado",
        "plugin 'command_runner' not registered",
    ),
    (
        "EXECUÇÃO (aprovação automática)",
        "EXECUTION (automatic approval)",
    ),
    (
        "EXECUÇÃO (você será perguntado a cada passo)",
        "EXECUTION (you'll be asked at every step)",
    ),
    // --- dispatch / parse / policy errors (human stderr) ---
    (
        "   [logger] evento de domínio: '{}'",
        "   [logger] domain event: '{}'",
    ),
    (
        "plugin 'stack_detector' não registrado",
        "plugin 'stack_detector' not registered",
    ),
    (
        "plugin 'local_command_runner' não registrado",
        "plugin 'local_command_runner' not registered",
    ),
    (
        "nenhuma stack detectada pra auditar",
        "no stack detected to audit",
    ),
    (
        "--fail-on inválido: {} (use low|medium|high|critical)",
        "--fail-on invalid: {} (use low|medium|high|critical)",
    ),
    (
        "fail_on inválido no policy: {}",
        "invalid fail_on in policy: {}",
    ),
    (
        "factor desconhecido na supressão: {}",
        "unknown factor in suppression: {}",
    ),
    (
        "supressão de '{}' sem 'expires' (expiração é obrigatória)",
        "suppression of '{}' without 'expires' (expiration is mandatory)",
    ),
    (
        "--lang exige um valor (pt-BR | en)",
        "--lang requires a value (pt-BR | en)",
    ),
    (
        "--timeout exige um número de segundos: {}",
        "--timeout requires a number of seconds: {}",
    ),
    ("opção desconhecida: {}", "unknown option: {}"),
    (
        "caminho informado mais de uma vez: {}",
        "path given more than once: {}",
    ),
    // --- detached runner: spawn_detached inner errors (the caller wraps
    //     them, but they are interpolated as arguments — build in lang) ---
    ("não abri o log {}: {}", "couldn't open the log {}: {}"),
    (
        "não clonei o log {}: {}",
        "couldn't duplicate the log handle {}: {}",
    ),
    // --- dataset findings (construction-lang: they reach JSON) ---
    (
        "possível exposição ({}) publicada em {} — confira se sua versão está coberta",
        "possible exposure ({}) published on {} — check whether your version is covered",
    ),
    (
        "possível exposição ({} + mais {} advisories para este pacote; este publicado em {}) — confira se sua versão está coberta",
        "possible exposure ({} + {} more advisories for this package; this one published on {}) — check whether your version is covered",
    ),
    (
        "nome consta na kill list como malicioso/typosquat: {}",
        "name is on the kill list as malicious/typosquat: {}",
    ),
    // --- workflow engine (core) and detector root error: human stderr only,
    //     rendered with the global language where they are constructed ---
    (
        "CORE: tentativa {}/{} para os passos {}",
        "CORE: attempt {}/{} for steps {}",
    ),
    (
        "CORE: -> executando passo '{}'",
        "CORE: -> executing step '{}'",
    ),
    (
        "CORE: workflow concluído na tentativa {}.",
        "CORE: workflow completed on attempt {}.",
    ),
    (
        "CORE: falha transitória ('{}'). Retentando em {}s (backoff exponencial)...",
        "CORE: transient failure ('{}'). Retrying in {}s (exponential backoff)...",
    ),
    (
        "workflow falhou após {} tentativas. Erro final: {}",
        "workflow failed after {} attempts. Final error: {}",
    ),
    (
        "diretório de projeto não encontrado: {}",
        "project directory not found: {}",
    ),
    // --- kernel/events stdout-noise and shared Error Display prefixes:
    //     human stderr, rendered with the global language at emit time ---
    (
        "KERNEL: registrando plugin '{}'",
        "KERNEL: registering plugin '{}'",
    ),
    (
        "KERNEL: DevForge inicializado.",
        "KERNEL: DevForge initialized.",
    ),
    (
        "EVENTBUS: publicando '{}' para {} assinante(s)",
        "EVENTBUS: publishing '{}' to {} subscriber(s)",
    ),
    ("erro de I/O: {}", "I/O error: {}"),
    (
        "erro de desserialização: {}",
        "deserialization error: {}",
    ),
    ("erro desconhecido: {}", "unknown error: {}"),
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
    fn keys_use_only_positional_placeholders() {
        // A named placeholder ({pm}) or format spec ({:?}) inside a catalog
        // key is NEVER substituted by t_fmt/t_fmt_in and leaks literally.
        // Every `{` in a key must belong to a plain `{}`.
        for (source, target) in EN {
            assert_eq!(
                source.matches('{').count(),
                source.matches("{}").count(),
                "chave com placeholder não-posicional: {source}"
            );
            assert_eq!(
                target.matches('{').count(),
                target.matches("{}").count(),
                "tradução com placeholder não-posicional: {target}"
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
        // default (already `en` since the pre-launch flip).
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

    #[test]
    fn t_en_forces_english_whatever_the_current_language() {
        // t_en() forces EN whatever the current language — the JSON contract
        // (dogfooding #6). The process default is EN since the flip, so pin
        // the contrast against an explicit pt translation of the same key.
        assert_eq!(translate(Lang::PtBr, "AMBIENTE"), "AMBIENTE");
        assert_eq!(t_en("AMBIENTE"), "ENVIRONMENT");
        assert_eq!(
            t_en("Veredito: SEM DADOS ⚠️  (0 pacotes — nada foi atestado)",
            ),
            "Verdict: NO DATA ⚠️  (0 packages — nothing was certified)"
        );
        // Unknown key: identity in both, never a guess.
        assert_eq!(t_en("mensagem ainda não migrada"), "mensagem ainda não migrada");
    }

    #[test]
    fn t_fmt_in_translates_the_requested_lang_before_filling() {
        let en = t_fmt_in(
            Lang::En,
            "Reconheci {} padrão(ões); o mais provável: {}",
            &["2", "some cause"],
        );
        assert_eq!(en, "Recognized 2 pattern(s); the most likely: some cause");
        // Placeholders survive translation in order (3× {} dynamic fix).
        let fix = t_fmt_in(
            Lang::En,
            "instale o pacote de dev da lib `{}` (Debian: lib{}-dev, Fedora: {}-devel) e rode de novo",
            &["ssl", "ssl", "ssl"],
        );
        assert_eq!(
            fix,
            "install the dev package for the lib `ssl` (Debian: libssl-dev, Fedora: ssl-devel) and run again"
        );
    }
}
