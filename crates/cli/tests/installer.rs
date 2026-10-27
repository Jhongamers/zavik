//! Integration tests for `install.sh` (F1 — the installer door).
//!
//! The installer is the only door that touches the network, so the tests
//! fake a whole release (tarball + sha256) served over `file://` and assert:
//! happy path installs a working binary, a tampered archive is refused, an
//! unsupported OS gets a clear message, the script parses as POSIX sh, and
//! version resolution picks the latest release (API JSON faked over file://)
//! with an honest fallback to the pinned version when the API is unreachable.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use tempfile::TempDir;

fn install_sh() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../install.sh")
        .canonicalize()
        .expect("install.sh na raiz do repo")
}

/// Runs the installer with an isolated HOME/install dir. Test machines must
/// never get a real binary dropped in `~/.local/bin`.
fn installer(dir: &TempDir) -> Command {
    let mut cmd = Command::new("sh");
    cmd.arg(install_sh())
        .env("HOME", dir.path())
        .env("ZAVIK_INSTALL_DIR", dir.path().join("bin"));
    cmd
}

/// Builds a fake release for `version`: tarball + sha256, downloadable via
/// `file://` (curl supports it) — same asset naming as release.yml.
fn fake_release(dir: &TempDir, version: &str) -> String {
    let target = "x86_64-unknown-linux-gnu";
    let asset = format!("zavik-{version}-{target}.tar.gz");

    let stage = dir.path().join("stage");
    fs::create_dir_all(&stage).unwrap();
    let bin = stage.join("zavik");
    fs::write(&bin, "#!/bin/sh\necho zavik test-binary\n").unwrap();
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();

    let tar = Command::new("tar")
        .args([
            "-czf",
            dir.path().join(&asset).to_str().unwrap(),
            "-C",
            stage.to_str().unwrap(),
            "zavik",
        ])
        .status()
        .unwrap();
    assert!(tar.success(), "tar do fixture");

    // Checksum file with the BARE asset name (like `sha256sum asset`), so the
    // installer can verify after `cd` into its download dir.
    let checksum = Command::new("sha256sum")
        .arg(&asset)
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(checksum.status.success(), "sha256sum do fixture");
    fs::write(dir.path().join(format!("{asset}.sha256")), &checksum.stdout).unwrap();

    asset
}

// --- syntax -----------------------------------------------------------------

#[test]
fn installer_passes_posix_syntax_check() {
    let status = Command::new("sh")
        .args(["-n", install_sh().to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success(), "sh -n install.sh deve passar");
}

// --- happy path -------------------------------------------------------------

#[test]
fn installer_downloads_verifies_and_installs_a_working_binary() {
    let release = TempDir::new().unwrap();
    fake_release(&release, "v0.0.0-test");
    let home = TempDir::new().unwrap();

    let output = installer(&home)
        .env("ZAVIK_VERSION", "v0.0.0-test")
        .env(
            "ZAVIK_BASE_URL",
            format!("file://{}", release.path().display()),
        )
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "instalador deve sair 0: {stdout} {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("installed:"),
        "confirma a instalação: {stdout}"
    );

    let installed = home.path().join("bin/zavik");
    assert!(installed.is_file(), "binário instalado em bin/zavik");
    let version = Command::new(&installed).output().unwrap();
    assert!(
        String::from_utf8_lossy(&version.stdout).contains("test-binary"),
        "o binário instalado roda"
    );
}

// --- anti-tamper ------------------------------------------------------------

#[test]
fn installer_refuses_a_tampered_archive() {
    let release = TempDir::new().unwrap();
    let asset = fake_release(&release, "v0.0.0-test");

    // Corrupt the tarball AFTER the checksum file was written.
    let mut bytes = fs::read(release.path().join(&asset)).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    fs::write(release.path().join(&asset), bytes).unwrap();

    let home = TempDir::new().unwrap();
    let output = installer(&home)
        .env("ZAVIK_VERSION", "v0.0.0-test")
        .env(
            "ZAVIK_BASE_URL",
            format!("file://{}", release.path().display()),
        )
        .output()
        .unwrap();

    assert!(!output.status.success(), "checksum divergente deve barrar");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("checksum mismatch"),
        "a recusa fala de checksum: {combined}"
    );
    assert!(
        !home.path().join("bin/zavik").exists(),
        "nada instalado em caso de fraude"
    );
}

// --- unsupported OS ---------------------------------------------------------

#[test]
fn installer_refuses_unsupported_os_with_clear_message() {
    // A fake `uname` reporting SunOS, shadowing the real one via PATH.
    let fake_bin = TempDir::new().unwrap();
    let uname = fake_bin.path().join("uname");
    fs::write(&uname, "#!/bin/sh\necho SunOS\n").unwrap();
    fs::set_permissions(&uname, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        fake_bin.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let home = TempDir::new().unwrap();
    let output = installer(&home).env("PATH", &path).output().unwrap();

    assert!(!output.status.success(), "SO desconhecido deve barrar");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("SunOS") && combined.contains("unsupported"),
        "a mensagem diz qual SO e que não é suportado: {combined}"
    );
}

// --- latest release resolution (pin > autodetect > fallback) -----------------

#[test]
fn installer_autodetects_the_latest_release_from_the_api_json() {
    let release = TempDir::new().unwrap();
    fake_release(&release, "v9.9.9-test");
    // GitHub's release JSON, faked over file:// (same shape the API returns).
    fs::write(
        release.path().join("latest.json"),
        r#"{"tag_name": "v9.9.9-test", "name": "v9.9.9-test"}"#,
    )
    .unwrap();

    let home = TempDir::new().unwrap();
    let output = installer(&home)
        .env_remove("ZAVIK_VERSION") // autodetect only kicks in unpinned
        .env(
            "ZAVIK_LATEST_URL",
            format!("file://{}/latest.json", release.path().display()),
        )
        .env(
            "ZAVIK_BASE_URL",
            format!("file://{}", release.path().display()),
        )
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "autodetect deve instalar a última release: {} {stderr}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !stderr.contains("warning"),
        "sem fallback quando a API responde: {stderr}"
    );
    let installed = home.path().join("bin/zavik");
    let version = Command::new(&installed).output().unwrap();
    assert!(
        String::from_utf8_lossy(&version.stdout).contains("test-binary"),
        "o binário da release autodetectada roda"
    );
}

#[test]
fn installer_falls_back_to_the_pinned_version_when_the_api_is_unreachable() {
    // The pin lives in install.sh itself; the test reads it so a release bump
    // never breaks this fixture.
    let script = fs::read_to_string(install_sh()).unwrap();
    let pin = script
        .split("FALLBACK_VERSION=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("install.sh declara FALLBACK_VERSION");
    assert!(pin.starts_with('v'), "pin tem cara de tag: {pin}");

    let release = TempDir::new().unwrap();
    fake_release(&release, pin);

    let home = TempDir::new().unwrap();
    let output = installer(&home)
        .env_remove("ZAVIK_VERSION") // force the autodetect path
        .env(
            "ZAVIK_LATEST_URL",
            format!("file://{}/missing.json", release.path().display()),
        )
        .env(
            "ZAVIK_BASE_URL",
            format!("file://{}", release.path().display()),
        )
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "API fora do ar não pode barrar a instalação: {stderr}"
    );
    assert!(
        stderr.contains("warning: could not determine the latest release"),
        "o fallback é honesto (avisa qual pin será usado): {stderr}"
    );
    assert!(
        stderr.contains(pin),
        "o aviso diz a versão pinada: {stderr}"
    );
    assert!(
        home.path().join("bin/zavik").is_file(),
        "o pin de fallback ainda instala"
    );
}
