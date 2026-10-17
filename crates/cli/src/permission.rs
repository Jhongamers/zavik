//! Permission gates for the presentation layer (CLI).
//!
//! The `core` never decides alone: it calls [`PermissionGate::confirm`] before
//! each sensitive action. Here we implement the interactive version (asks on the
//! terminal) and the "approve everything" version (`--yes`).
//!
//! **Security rule:** code execution / supply-chain risks
//! (`PermissionRequest.critical`) are **never** covered by "yes to all"
//! nor by `--yes`. They require explicit individual confirmation — that way an AI (or
//! a `--yes` in CI) does not install arbitrary code without a human deciding.

use devforge_sdk::{Decision, PermissionGate, PermissionRequest};
use devforge_shared::i18n::{t, t_fmt};
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

/// Asks the user on the terminal before each action.
#[derive(Default)]
pub struct InteractivePermissionGate {
    approve_all: AtomicBool,
}

impl InteractivePermissionGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks the question on the terminal.
    ///
    /// `allow_batch` controls whether "yes to all" is offered (never for risks).
    fn ask(&self, request: &PermissionRequest, allow_batch: bool) -> Decision {
        if request.critical {
            println!(
                "{}",
                t_fmt("\n⚠️  RISCO DE SEGURANÇA: {}", &[&request.title])
            );
        } else {
            println!(
                "{}",
                t_fmt("\n🔐 Permissão necessária: {}", &[&request.title])
            );
        }
        println!("{}", t_fmt("   ação   : {}", &[&request.detail]));
        println!("{}", t_fmt("   motivo : {}", &[&request.reason]));

        let prompt = t(if allow_batch {
            " [s]im / [n]ão / [a]sim para todas: "
        } else {
            " [s]im / [n]ão: "
        })
        .to_string();

        loop {
            print!("{}", t_fmt("   Executar?{}", &[&prompt]));
            let _ = io::stdout().flush();

            let mut line = String::new();
            match io::stdin().read_line(&mut line) {
                // EOF (e.g., non-interactive execution without `--yes`): denies for safety.
                Ok(0) => {
                    println!();
                    return Decision::Denied;
                }
                Ok(_) => {}
                Err(_) => return Decision::Denied,
            }

            match line.trim().to_ascii_lowercase().as_str() {
                "s" | "sim" | "y" | "yes" => return Decision::Approved,
                "a" | "all" | "todas" if allow_batch => {
                    self.approve_all.store(true, Ordering::Relaxed);
                    return Decision::Approved;
                }
                "n" | "nao" | "não" | "no" | "" => return Decision::Denied,
                _ => println!(
                    "{}",
                    t(if allow_batch {
                        "   resposta inválida (use s, n ou a)."
                    } else {
                        "   resposta inválida (use s ou n)."
                    })
                ),
            }
        }
    }
}

impl PermissionGate for InteractivePermissionGate {
    fn confirm(&self, request: &PermissionRequest) -> Decision {
        // Code execution risk: individual confirmation, always.
        if request.critical {
            return self.ask(request, false);
        }
        if self.approve_all.load(Ordering::Relaxed) {
            return Decision::Approved;
        }
        self.ask(request, true)
    }
}

/// Approves everything automatically (used with `--yes`).
///
/// **Except** critical risks: `--yes` cannot approve arbitrary code
/// execution by itself. In that case it returns `Denied` and the flow aborts with a warning.
pub struct AutoApprovePermissionGate;

impl PermissionGate for AutoApprovePermissionGate {
    fn confirm(&self, request: &PermissionRequest) -> Decision {
        if request.critical {
            Decision::Denied
        } else {
            Decision::Approved
        }
    }
}
