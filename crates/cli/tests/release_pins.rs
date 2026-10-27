//! Invariant tests for the release pins in the root `Cargo.toml` and the npm
//! package version.
//!
//! `cargo publish` requires every internal path dependency to carry a
//! `version` (path feeds local builds, version feeds the registry), and a
//! version that drifts from the workspace version fails the publish of the
//! whole closure — usually discovered at release time, on the founder's
//! machine, under time pressure. Same philosophy as `action_yml.rs`: the
//! coercion becomes automatic instead of a manual checklist item.
//!
//! `install.sh`'s FALLBACK_VERSION is deliberately NOT asserted here — it is
//! designed to be safely forgettable (stale pin degrades to "old version +
//! warning", see `docs/design-installer.md` §2), so CI must not force it.

use std::fs;
use std::path::PathBuf;

fn root_manifest() -> String {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop(); // crates/cli -> crates/
    root.pop(); // crates/ -> repo root
    root.push("Cargo.toml");
    fs::read_to_string(&root).expect("root Cargo.toml must exist")
}

/// The `zavik-* = { version = "...", path = "..." }` pins declared under
/// `[workspace.dependencies]` — the only place a release bump can miss.
fn internal_dependency_pins(manifest: &str) -> Vec<(String, String)> {
    let mut in_section = false;
    let mut pins = Vec::new();
    for line in manifest.lines() {
        if line.starts_with('[') {
            in_section = line == "[workspace.dependencies]";
            continue;
        }
        if !in_section || !line.starts_with("zavik-") {
            continue;
        }
        let name = line
            .split(" =")
            .next()
            .expect("crate name before `=`")
            .trim()
            .to_string();
        let version = line
            .split("version = \"")
            .nth(1)
            .unwrap_or_else(|| panic!("{name} has no version pin — cargo publish requires it"))
            .split('"')
            .next()
            .expect("closing quote")
            .to_string();
        pins.push((name, version));
    }
    pins
}

#[test]
fn every_internal_dependency_pin_matches_the_workspace_version() {
    let expected = env!("CARGO_PKG_VERSION");
    let pins = internal_dependency_pins(&root_manifest());

    // The whole closure must be pinned, not just some of it: a missing crate
    // breaks `cargo publish --workspace` exactly like a wrong version.
    let declared: Vec<&str> = pins.iter().map(|(name, _)| name.as_str()).collect();
    for crate_name in [
        "zavik-shared",
        "zavik-events",
        "zavik-sdk",
        "zavik-plugins",
        "zavik-core",
        "zavik-kernel",
    ] {
        assert!(
            declared.contains(&crate_name),
            "{crate_name} lost its [workspace.dependencies] pin"
        );
    }

    for (name, version) in pins {
        assert_eq!(
            version, expected,
            "{name} pin drifted from the workspace version — cargo publish would fail"
        );
    }
}

#[test]
fn npm_package_version_matches_the_workspace_version() {
    // The npm shim downloads the release matching ITS OWN version
    // (`npm/lib/ensure.js` defaults to `package.json`'s version), so a
    // package published at 0.1.5 while the release is 0.1.4 would 404 on
    // first run for every user.
    let mut manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.pop(); // crates/cli -> crates/
    manifest.pop(); // crates/ -> repo root
    manifest.push("npm/package.json");
    let package = fs::read_to_string(&manifest).expect("npm/package.json must exist");

    let version = package
        .split("\n  \"version\": \"")
        .nth(1)
        .expect("npm package.json has a version field")
        .split('"')
        .next()
        .expect("closing quote");
    assert_eq!(
        version,
        env!("CARGO_PKG_VERSION"),
        "npm package version and released binary version drifted apart"
    );
}
