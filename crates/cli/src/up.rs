//! `devforge up` — step planning for the composed pipeline.
//!
//! Pure logic (no I/O): whether the target is a URL, which steps run, what
//! `--until`/`--skip` cut. The orchestration itself lives in `main.rs` and is
//! literally the isolated commands (audit → prepare → run) sharing their gates.

/// Steps of the composed pipeline, in execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpStep {
    Clone,
    Audit,
    Prepare,
    Run,
}

impl UpStep {
    pub fn name(self) -> &'static str {
        match self {
            UpStep::Clone => "clone",
            UpStep::Audit => "audit",
            UpStep::Prepare => "prepare",
            UpStep::Run => "run",
        }
    }

    /// One-line description for `--dry-run` (translated: human surface).
    pub fn description(self) -> &'static str {
        use devforge_shared::i18n::t;
        match self {
            UpStep::Clone => t("git clone --depth 1 (sem submódulos, sem hooks)"),
            UpStep::Audit => t("devforge audit (relatório; não bloqueia o pipeline)"),
            UpStep::Prepare => t("devforge prepare (confiança + toolchain + instalação, com gates)"),
            UpStep::Run => t("devforge run (resolve o start; execução crítica, humano confirma)"),
        }
    }
}

const ORDER: [UpStep; 4] = [UpStep::Clone, UpStep::Audit, UpStep::Prepare, UpStep::Run];

const VALID_STEPS: &str = "clone, audit, prepare, run";

fn step_for(name: &str) -> Option<UpStep> {
    ORDER.iter().copied().find(|step| step.name() == name)
}

/// Is `target` a git URL to clone (vs. a local path)?
///
/// Prefixes we accept, plus any target ending in `.git` (local bare-ish repos).
pub fn is_git_url(target: &str) -> bool {
    ["http://", "https://", "git@", "ssh://", "file://"]
        .iter()
        .any(|prefix| target.starts_with(prefix))
        || target.ends_with(".git")
}

/// Derives the clone destination directory name from the URL
/// (`git@host:org/repo.git` → `repo`), sanitized to `[A-Za-z0-9._-]`.
pub fn repo_name(url: &str) -> Result<String, String> {
    let trimmed = url.strip_suffix(".git").unwrap_or(url);
    let last = trimmed.rsplit(['/', ':']).next().unwrap_or("");
    let sanitized: String = last
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-'))
        .collect();
    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        return Err(format!("não consegui derivar o nome do repositório de: {url}"));
    }
    Ok(sanitized)
}

/// Plans which steps run: URL adds `clone`; `--until` cuts the tail;
/// `--skip` removes steps. Invalid combinations are errors (never guesses).
pub fn up_plan(
    target_is_url: bool,
    until: Option<&str>,
    skip: &[&str],
) -> Result<Vec<UpStep>, String> {
    for value in skip {
        step_for(value)
            .ok_or_else(|| format!("--skip: etapa desconhecida: {value} (válidas: {VALID_STEPS})"))?;
    }
    let until_index = match until {
        Some(name) => {
            let step = step_for(name).ok_or_else(|| {
                format!("--until: etapa desconhecida: {name} (válidas: {VALID_STEPS})")
            })?;
            Some(
                ORDER
                    .iter()
                    .position(|candidate| *candidate == step)
                    .expect("step comes from ORDER"),
            )
        }
        None => None,
    };
    if skip.contains(&"clone") {
        // Never sensible: a path has no clone step; with a URL the clone IS
        // the target (skipping it would silently run on the cwd instead).
        return Err(
            "--skip clone não é suportado: sem URL não há o que clonar; com URL o clone é obrigatório"
                .to_string(),
        );
    }
    if !target_is_url && until == Some("clone") {
        return Err("--until clone exige uma URL (sem URL não há etapa de clone)".to_string());
    }

    let mut plan = Vec::new();
    for (index, step) in ORDER.iter().enumerate() {
        if step == &UpStep::Clone && !target_is_url {
            continue;
        }
        if let Some(until_index) = until_index {
            if index > until_index {
                break;
            }
        }
        if skip.contains(&step.name()) {
            continue;
        }
        plan.push(*step);
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(plan: &[UpStep]) -> Vec<&'static str> {
        plan.iter().map(|step| step.name()).collect()
    }

    #[test]
    fn path_without_flags_runs_audit_prepare_run() {
        let plan = up_plan(false, None, &[]).unwrap();
        assert_eq!(names(&plan), ["audit", "prepare", "run"]);
    }

    #[test]
    fn url_prefixes_clone_step() {
        let plan = up_plan(true, None, &[]).unwrap();
        assert_eq!(names(&plan), ["clone", "audit", "prepare", "run"]);
    }

    #[test]
    fn until_cuts_the_tail() {
        let plan = up_plan(true, Some("audit"), &[]).unwrap();
        assert_eq!(names(&plan), ["clone", "audit"]);

        let plan = up_plan(false, Some("prepare"), &[]).unwrap();
        assert_eq!(names(&plan), ["audit", "prepare"]);
    }

    #[test]
    fn skip_removes_steps() {
        let plan = up_plan(false, None, &["run"]).unwrap();
        assert_eq!(names(&plan), ["audit", "prepare"]);

        let plan = up_plan(false, None, &["audit"]).unwrap();
        assert_eq!(names(&plan), ["prepare", "run"]);
    }

    #[test]
    fn skip_and_until_compose() {
        let plan = up_plan(false, Some("run"), &["audit"]).unwrap();
        assert_eq!(names(&plan), ["prepare", "run"]);
    }

    #[test]
    fn unknown_steps_are_errors() {
        assert!(up_plan(false, Some("banana"), &[]).unwrap_err().contains("--until"));
        assert!(up_plan(false, None, &["banana"]).unwrap_err().contains("--skip"));
    }

    #[test]
    fn skipping_clone_is_always_an_error() {
        let error = up_plan(false, None, &["clone"]).unwrap_err();
        assert!(error.contains("--skip clone"));
        // URL target too: skipping the clone would run the pipeline on the cwd.
        let error = up_plan(true, None, &["clone"]).unwrap_err();
        assert!(error.contains("--skip clone"));
    }

    #[test]
    fn until_clone_on_url_keeps_only_clone() {
        let plan = up_plan(true, Some("clone"), &[]).unwrap();
        assert_eq!(names(&plan), ["clone"]);
    }

    #[test]
    fn until_clone_without_url_is_an_error() {
        let error = up_plan(false, Some("clone"), &[]).unwrap_err();
        assert!(error.contains("--until clone"));
    }

    #[test]
    fn url_detection() {
        assert!(is_git_url("https://github.com/x/y"));
        assert!(is_git_url("git@github.com:x/y.git"));
        assert!(is_git_url("ssh://host/x/y.git"));
        assert!(is_git_url("file:///tmp/repo"));
        assert!(is_git_url("/tmp/some-repo.git"));
        assert!(!is_git_url("/tmp/some-repo"));
        assert!(!is_git_url("."));
        assert!(!is_git_url("../proj"));
    }

    #[test]
    fn repo_name_from_urls() {
        assert_eq!(repo_name("https://github.com/x/y").unwrap(), "y");
        assert_eq!(repo_name("https://github.com/x/y.git").unwrap(), "y");
        assert_eq!(repo_name("git@github.com:x/y.git").unwrap(), "y");
        assert!(repo_name("https://github.com/x/").is_err());
    }
}
