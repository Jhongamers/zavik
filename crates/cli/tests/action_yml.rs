//! Invariant tests for `action.yml` — the published GitHub Action surface.
//!
//! The input defaults are invisible to `cargo` consumers but break at
//! runtime in someone else's CI when they drift (dogfooding #20: the
//! `repository` input defaulted to the literal placeholder `OWNER/zavik`,
//! so the snippet documented in the README 404'd on the install step —
//! found by the first anonymous E2E run after the repo went public).
//!
//! Invariants covered here (they run on every push via `cargo test`):
//! 1. `repository` default is the canonical release repo, `owner/name` shaped.
//! 2. `version` default is `v<CARGO_PKG_VERSION>` — the tag ↔ binary
//!    coherence that used to be checked by hand on every release.

use std::fs;
use std::path::PathBuf;

fn action_yml() -> String {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop(); // crates/cli -> crates/
    root.pop(); // crates/ -> repo root
    root.push("action.yml");
    fs::read_to_string(&root).expect("action.yml must exist at the repo root")
}

/// Returns the `default:` value of a top-level `inputs.<input>` entry.
fn input_default(yml: &str, input: &str) -> String {
    let mut in_block = false;
    for line in yml.lines() {
        let two_space = line.starts_with("  ") && !line.starts_with("   ");
        if two_space && line.trim() == format!("{input}:") {
            in_block = true;
            continue;
        }
        if in_block {
            if let Some(default) = line.strip_prefix("    default: ") {
                return default.trim().trim_matches('"').to_string();
            }
            // A new two-space key closes the block before any default.
            if two_space && !line.trim().is_empty() {
                in_block = false;
            }
        }
    }
    panic!("`inputs.{input}` has no default in action.yml");
}

#[test]
fn repository_default_points_at_the_canonical_release_repo() {
    let default = input_default(&action_yml(), "repository");
    // A placeholder or malformed value ships a broken install step to every
    // consumer following the README (they don't pass this input).
    assert_eq!(default, "Jhongamers/zavik");
    let (owner, name) = default.split_once('/').expect("`owner/name` format");
    assert!(!owner.is_empty() && !name.is_empty());
    assert!(
        !default.to_uppercase().contains("OWNER"),
        "literal OWNER placeholder is back: {default}"
    );
}

#[test]
fn version_default_matches_the_workspace_crate_version() {
    let default = input_default(&action_yml(), "version");
    let expected = format!("v{}", env!("CARGO_PKG_VERSION"));
    assert_eq!(
        default, expected,
        "action default tag and released binary version drifted apart"
    );
}
