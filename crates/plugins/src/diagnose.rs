//! Error diagnosis (the `diagnose` command): turns a failed command's output
//! into a **short summary + real probable cause + fix**.
//!
//! The point: the literal error is almost always a **symptom**, not the cause.
//! "cannot find -lssl" is linking, but the cause is "missing libssl-dev";
//! "node-gyp ERR" is a native module build, but the cause is "missing
//! python3/gcc". This module matches **known patterns** in the whole output and
//! returns the root cause, with the proof (the lines that matched) and the next
//! step. It is local Rust, deterministic, zero tokens.

use devforge_shared::{Cause, Diagnosis};
use regex::Regex;
use std::sync::OnceLock;

struct Rule {
    id: &'static str,
    /// Lowercase substrings that trigger the rule.
    patterns: &'static [&'static str],
    cause: &'static str,
    fix: &'static str,
    /// When the literal error misleads: explains it is just the symptom.
    note: Option<&'static str>,
    /// Higher = more likely / shows up first.
    priority: u8,
}

// Order/ranking by likelihood of being the ROOT cause (not the symptom).
const RULES: &[Rule] = &[
    Rule {
        id: "command-not-found",
        patterns: &["command not found", "is not recognized as an internal", ": not found"],
        cause: "Uma ferramenta necessária não está instalada no PATH.",
        fix: "instale a ferramenta (ou rode `devforge prepare` pra montar a toolchain).",
        note: None,
        priority: 30,
    },
    Rule {
        id: "missing-lib",
        patterns: &[
            "cannot find -l",
            "library not found for -l",
            ".h: no such file or directory",
        ],
        cause: "Falta o pacote de DESENVOLVIMENTO (cabeçalho/lib de link) de uma biblioteca nativa.",
        fix: "instale o pacote -dev/-devel correspondente e rode de novo.",
        note: Some(
            "O erro de linking/cabeçalho é só o SINTOMA — a causa real é o pacote de dev ausente.",
        ),
        priority: 25,
    },
    Rule {
        id: "missing-compiler",
        patterns: &[
            "linker `cc` not found",
            "linker `gcc` not found",
            "linker `cc.exe` not found",
            "x86_64-linux-gnu-gcc: not found",
        ],
        cause: "Falta a toolchain de compilação C/C++ (necessária pra módulos nativos).",
        fix: "Debian/Ubuntu: `sudo apt install build-essential`; macOS: `xcode-select --install`.",
        note: None,
        priority: 25,
    },
    Rule {
        id: "node-gyp",
        patterns: &["gyp err", "no xcode or clt", "gyp: no xcode"],
        cause: "Módulo nativo do npm precisa de ferramentas de build (python3 + gcc + make).",
        fix: "instale python3 e build-essential e rode `npm install` de novo.",
        note: Some("O 'node-gyp ERR' é consequência — a causa é a toolchain de build ausente."),
        priority: 22,
    },
    Rule {
        id: "python-module",
        patterns: &["modulenotfounderror", "no module named"],
        cause: "Um módulo Python não está instalado no ambiente atual.",
        fix: "instale o módulo (pip install <nome>) no mesmo venv.",
        note: None,
        priority: 20,
    },
    Rule {
        id: "missing-dep-go",
        patterns: &[
            "no required module provides package",
            "cannot find module for path",
            "to add it:",
        ],
        cause: "Dependência Go faltando no módulo (não está no go.mod/go.sum).",
        fix: "rode `go mod tidy` ou `go get <pacote>`.",
        note: None,
        priority: 20,
    },
    Rule {
        id: "java-version",
        patterns: &["unsupportedclassversionerror", "compiled by a more recent version"],
        cause: "Classe compilada com um JDK mais novo que o que está rodando.",
        fix: "alinhe as versões: compile e rode com o mesmo JDK.",
        note: None,
        priority: 18,
    },
    Rule {
        id: "port-in-use",
        patterns: &[
            "address already in use",
            "eaddrinuse",
            "port is already allocated",
            "bind: address already in use",
        ],
        cause: "A porta que o serviço quer usar já está ocupada por outro processo.",
        fix: "mude a porta ou encerre o processo que a usa.",
        note: None,
        priority: 15,
    },
    Rule {
        id: "permission",
        patterns: &["permission denied", "eacces", "access is denied"],
        cause: "Sem permissão de escrita/execução no caminho em questão.",
        fix: "verifique dono/permissões do diretório (evite `sudo` global, sobretudo em node_modules).",
        note: None,
        priority: 15,
    },
    Rule {
        id: "npm-global-perm",
        patterns: &["npm err! code eacces", "cannot open ... gyp", "mkdir '/usr/lib/node_modules'"],
        cause: "O npm está tentando escrever em pastas globais sem permissão.",
        fix: "evite `sudo npm`; use um gerenciador de versão (nvm/fnm) ou instalação local.",
        note: None,
        priority: 14,
    },
    Rule {
        id: "network",
        patterns: &[
            "could not resolve host",
            "etimedout",
            "connection refused",
            "unable to access",
            "certificate problem",
            "network error",
            "failed to fetch",
        ],
        cause: "Falha de rede/registry (offline, proxy, TLS ou DNS).",
        fix: "verifique conexão/proxy/CA — alguns registries (ex.: crates.io) exigem User-Agent.",
        note: None,
        priority: 12,
    },
    Rule {
        id: "oom",
        patterns: &[
            "out of memory",
            "oom-killer",
            "heap out of memory",
            "memoryerror",
            "std::bad_alloc",
            "cannot allocate memory",
        ],
        cause: "O processo ficou sem memória.",
        fix: "feche processos, aumente o limite ou (node) use `--max-old-space-size`.",
        note: None,
        priority: 12,
    },
    Rule {
        id: "no-space",
        patterns: &["no space left on device"],
        cause: "Disco cheio.",
        fix: "libere espaço em disco e tente de novo.",
        note: None,
        priority: 12,
    },
    Rule {
        id: "env-var",
        patterns: &[
            "environment variable",
            "undefined variable",
            "keyerror:",
            "no configuration for key",
        ],
        cause: "Falta uma variável de ambiente / config esperada pelo projeto.",
        fix: "defina a variável (veja `.env.example` ou o README).",
        note: None,
        priority: 10,
    },
    Rule {
        id: "java-main",
        patterns: &["could not find or load main class"],
        cause: "A classe principal não foi encontrada (classpath ou nome errado).",
        fix: "confira o classpath e o nome fully-qualified da classe.",
        note: None,
        priority: 10,
    },
    Rule {
        id: "panic",
        patterns: &["panicked at", "thread 'main' panicked", "stack overflow"],
        cause: "Panic/crash no próprio código (bug, não ambiente).",
        fix: "leia o backtrace; a causa está no código, não na toolchain.",
        note: None,
        priority: 5,
    },
];

/// Diagnoses a failed command's output.
pub fn diagnose(output: &str) -> Diagnosis {
    let mut hits: Vec<(u8, &Rule, Vec<String>)> = Vec::new();
    for rule in RULES {
        let evidence = evidence_lines(output, rule);
        if !evidence.is_empty() {
            hits.push((rule.priority, rule, evidence));
        }
    }
    // Most likely first; without duplicating the same rule.
    hits.sort_by_key(|(priority, _, _)| std::cmp::Reverse(*priority));
    hits.dedup_by(|a, b| a.1.id == b.1.id);

    let causes: Vec<Cause> = hits
        .into_iter()
        .map(|(_, rule, evidence)| {
            let (cause, fix) = dynamic(rule, output);
            Cause {
                id: rule.id.to_string(),
                cause,
                fix,
                note: rule.note.map(str::to_string),
                evidence,
            }
        })
        .collect();

    let summary = match causes.first() {
        Some(top) => format!(
            "Reconheci {} padrão(ões); o mais provável: {}",
            causes.len(),
            top.cause
        ),
        None => "Não reconheci o padrão do erro — seguem as linhas com mais cara de erro.".to_string(),
    };

    Diagnosis {
        summary,
        causes,
        top_errors: top_error_lines(output),
    }
}

fn evidence_lines(output: &str, rule: &Rule) -> Vec<String> {
    output
        .lines()
        .filter(|line| {
            let lowered = line.to_lowercase();
            rule.patterns.iter().any(|p| lowered.contains(p))
        })
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .take(3)
        .collect()
}

/// Cause/fix with a dynamic part extracted from the output itself, when possible.
fn dynamic(rule: &Rule, output: &str) -> (String, String) {
    let cause = rule.cause.to_string();
    let fix = match rule.id {
        "missing-lib" => extract(output, dash_l_regex()).map(|lib| {
            format!(
                "instale o pacote de dev da lib `{lib}` (Debian: lib{lib}-dev, Fedora: {lib}-devel) e rode de novo"
            )
        }),
        "python-module" => extract(output, no_module_regex())
            .map(|m| format!("pip install {m} (no mesmo venv/ambiente do projeto)")),
        "command-not-found" => extract_command(output)
            .map(|c| format!("`{c}` não está no PATH — instale ou rode `devforge prepare` pra toolchain")),
        _ => None,
    };
    let fix = fix.unwrap_or_else(|| rule.fix.to_string());
    (cause, fix)
}

// ---------------------------------------------------------------------------
// Dynamic extraction (regex compiled once)
// ---------------------------------------------------------------------------

fn dash_l_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(?:cannot find|library not found for) -l([A-Za-z0-9_.+-]+)").unwrap())
}

fn no_module_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?i)no module named ['"]([^'"]+)['"]"#).unwrap())
}

fn command_re1() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)([A-Za-z0-9_.+-]+): command not found").unwrap())
}

fn command_re2() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)([A-Za-z0-9_.+-]+) is not recognized").unwrap())
}

/// bash/sh form: `bash: 1: cargo: not found`.
fn command_re3() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)([A-Za-z0-9_.+-]+): not found").unwrap())
}

fn extract(output: &str, re: &Regex) -> Option<String> {
    re.captures(output)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_command(output: &str) -> Option<String> {
    extract(output, command_re1())
        .or_else(|| extract(output, command_re2()))
        .or_else(|| extract(output, command_re3()))
}

/// Fallback: lines that "look like errors", for when no rule matches.
fn top_error_lines(output: &str) -> Vec<String> {
    const KEYWORDS: &[&str] = &[
        "error", "failed", "fatal", "exception", "panic", "not found", "cannot", "undefined",
        "denied", "traceback", "abort",
    ];
    let mut out = Vec::new();
    for line in output.lines() {
        let lowered = line.to_lowercase();
        let trimmed = line.trim();
        if !trimmed.is_empty()
            && KEYWORDS.iter().any(|k| lowered.contains(k))
            && !out.iter().any(|e: &String| e == trimmed)
        {
            out.push(trimmed.to_string());
        }
        if out.len() >= 8 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linking_points_to_real_lib_cause() {
        let out = "   = note: /usr/bin/ld: cannot find -lssl\ncollect2: error: ld returned 1 exit status";
        let d = diagnose(out);
        let top = &d.causes[0];
        assert_eq!(top.id, "missing-lib");
        // The dynamic fix points at the dev library, not the linker.
        assert!(top.fix.contains("libssl-dev"), "fix: {}", top.fix);
        assert!(top.note.as_deref().unwrap_or("").contains("SINTOMA"));
    }

    #[test]
    fn command_not_found_extracts_name() {
        let d = diagnose("sh: 1: cargo: not found");
        assert_eq!(d.causes[0].id, "command-not-found");
        assert!(d.causes[0].fix.contains("cargo"), "fix: {}", d.causes[0].fix);
    }

    #[test]
    fn python_module_extracts_name() {
        let d = diagnose("Traceback: ModuleNotFoundError: No module named 'requests'");
        assert_eq!(d.causes[0].id, "python-module");
        assert!(d.causes[0].fix.contains("pip install requests"));
    }

    #[test]
    fn unknown_output_uses_fallback() {
        let d = diagnose("algo totalmente estranho sem padrao conhecido");
        assert!(d.causes.is_empty());
        assert!(d.summary.contains("Não reconheci"));
    }

    #[test]
    fn generic_error_goes_to_top_errors() {
        let d = diagnose("xyz failed with an unknown weird message");
        assert!(d.top_errors.iter().any(|l| l.contains("failed")));
    }

    #[test]
    fn real_header_error_still_triggers_missing_lib() {
        // Positive regression: a gcc/clang header error still gets Detected.
        let d = diagnose("main.c:1:10: fatal error: openssl/ssl.h: No such file or directory");
        assert_eq!(d.causes[0].id, "missing-lib");
    }

    #[test]
    fn no_false_positives_on_benign_texts() {
        // Each line, on its own, must match NO rule (false positives).
        let benignos = [
            "cat: foo.txt: No such file or directory",              // missing file, not a lib
            "fatal error: algo inesperado aconteceu",               // generic "fatal error:"
            "prebuild-install info successfully installed binary",   // successful prebuild
            "running 5 tests, 0 killed",                            // "killed" in a test
            "DEBUG is not set (ok em dev)",                         // benign "is not set"
            "the cache is warm and the app is healthy",             // no pattern at all
        ];
        for texto in benignos {
            let d = diagnose(texto);
            assert!(
                d.causes.is_empty(),
                "falso positivo em {texto:?}: {:?}",
                d.causes.iter().map(|c| &c.id).collect::<Vec<_>>()
            );
        }
    }
}
