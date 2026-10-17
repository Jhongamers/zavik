//! `run --detach`: start the project in the background and walk away.
//!
//! An AI must never wait, so this is the "sobe e devolve o PID" door. The
//! process gets its **own session** (`setsid`): closing this terminal or
//! Ctrl-C must not take the dev server down. stdout/stderr go to the log
//! file; nobody supervises it — that's the honest trade of detached mode
//! (the PID comes back so the caller can stop it).
//!
//! On Windows there is no `setsid`: the process stays tied to the console
//! session (known limitation, printed by the CLI).

use std::path::Path;
use std::process::{Command, Stdio};

/// Spawns `program args...` in `root`, output redirected to `log_path`.
/// Returns the PID of the running process. Errors are plain strings — the
/// caller decides how to format them (JSON/human + diagnosis).
pub fn spawn_detached(
    program: &str,
    args: &[String],
    root: &Path,
    log_path: &Path,
) -> Result<u32, String> {
    let log = std::fs::File::create(log_path)
        .map_err(|error| format!("não abri o log {}: {error}", log_path.display()))?;
    let stderr_log = log
        .try_clone()
        .map_err(|error| format!("não clonei o log {}: {error}", log_path.display()))?;

    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr_log));

    // setsid — NOT process_group: setsid fails if we are already a group
    // leader, and a new session covers a new group anyway (sid == pgid).
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    // The child is intentionally NOT waited on: we drop the handle and the
    // process keeps running (reparented to init when we exit).
    command
        .spawn()
        .map(|child| child.id())
        .map_err(|error| format!("não foi possível executar '{program}': {error}"))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// Isolated per-test dir (tagged: tests run in parallel in one process).
    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("devforge-detached-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn detached_process_lives_in_its_own_session_and_is_stoppable() {
        let dir = tmpdir("session");
        let log = dir.join("run.log");
        let pid = spawn_detached("sleep", &["30".into()], &dir, &log).expect("spawn");
        assert!(pid > 0, "PID devolvido: {pid}");
        unsafe {
            // Alive…
            assert_eq!(libc::kill(pid as libc::pid_t, 0), 0, "processo vivo");
            // …in ANOTHER session (true detach, not just another group)…
            assert_ne!(
                libc::getsid(pid as libc::pid_t),
                libc::getsid(0),
                "sessão própria = sobrevive ao terminal"
            );
            // …and stoppable via the returned PID.
            assert_eq!(libc::kill(pid as libc::pid_t, libc::SIGKILL), 0);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detached_output_goes_to_the_log() {
        let dir = tmpdir("log");
        let log = dir.join("run.log");
        let pid = spawn_detached(
            "sh",
            &["-c".into(), "echo detach-marker".into()],
            &dir,
            &log,
        )
        .expect("spawn");

        let mut content = String::new();
        for _ in 0..50 {
            content = std::fs::read_to_string(&log).unwrap_or_default();
            if content.contains("detach-marker") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGKILL);
        }
        assert!(content.contains("detach-marker"), "log: {content:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn spawn_failure_is_an_error_not_a_panic() {
        let dir = tmpdir("fail");
        let log = dir.join("run.log");
        let error =
            spawn_detached("definitely-not-a-real-binary-xyz", &[], &dir, &log).unwrap_err();
        assert!(
            error.contains("definitely-not-a-real-binary-xyz"),
            "erro nomeia o comando: {error}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
