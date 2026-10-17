//! `devforge run` (v1) — start command resolution + supervision.
//!
//! Two halves, both tested without external network:
//!
//! 1. **Resolution** ([`resolve_run`]): declared > convention > ask ladder.
//!    The project *says* how it runs (scripts.start, Procfile, Makefile,
//!    compose) and we only fall back to a language convention when it doesn't.
//!    Ambiguity becomes [`RunResolution::Ambiguous`] — the presentation layer
//!    asks; **we never choose on our own** (same anti-false-positive philosophy
//!    as the rest).
//!
//! 2. **Supervision** ([`supervise`]): a server never terminates, so "it
//!    worked" is not an exit code — it's a **port accepting connections**
//!    (heuristic readiness) before the timeout; an early crash becomes output
//!    for `diagnose`; stopping is SIGTERM → wait → SIGKILL. Heuristics are
//!    labeled as heuristics.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Start command resolution
// ---------------------------------------------------------------------------

/// Single execution plan (the resolved command and where it came from).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunPlan {
    pub command: String,
    pub args: Vec<String>,
    /// Human-readable description of the source (e.g. `"package.json scripts.start"`).
    pub source: String,
}

/// A candidate presented when the resolution is ambiguous.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCandidate {
    pub command: String,
    pub args: Vec<String>,
    pub label: String,
}

/// Resolution outcome: ready plan, a choice is needed, or nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunResolution {
    Ready(RunPlan),
    Ambiguous(Vec<RunCandidate>),
    NotFound,
}

fn plan(command: &str, args: &[&str], source: &str) -> RunResolution {
    RunResolution::Ready(RunPlan {
        command: command.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        source: source.to_string(),
    })
}

fn candidate(command: &str, args: &[&str], label: &str) -> RunCandidate {
    RunCandidate {
        command: command.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        label: label.to_string(),
    }
}

/// Resolves how to start the project at `root`.
pub fn resolve_run(root: &Path) -> RunResolution {
    // --- Layer A: declared (the project says how it runs) ---

    // A1) package.json `scripts.start`
    if let Ok(text) = std::fs::read_to_string(root.join("package.json")) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            let has_start = json
                .get("scripts")
                .and_then(|s| s.get("start"))
                .is_some_and(|s| !s.as_str().unwrap_or("").is_empty());
            if has_start {
                let pm = json
                    .get("packageManager")
                    .and_then(|p| p.as_str())
                    .and_then(|p| p.split(['@', ' ']).next())
                    .unwrap_or("npm");
                let pm = if pm == "npm" || pm == "pnpm" || pm == "yarn" { pm } else { "npm" };
                return plan(pm, &["run", "start"], "package.json scripts.start");
            }
        }
    }

    // A2) Procfile (Heroku default): `web:` preferred; 1 process only → that one.
    if let Ok(text) = std::fs::read_to_string(root.join("Procfile")) {
        let processes: Vec<(String, String)> = text
            .lines()
            .filter_map(|line| line.split_once(':'))
            .map(|(name, cmd)| (name.trim().to_string(), cmd.trim().to_string()))
            .filter(|(_, cmd)| !cmd.is_empty())
            .collect();
        if let Some((_, cmd)) = processes.iter().find(|(name, _)| name == "web") {
            return procfile_plan(cmd, "Procfile web");
        }
        if processes.len() == 1 {
            let (name, cmd) = &processes[0];
            return procfile_plan(cmd, &format!("Procfile {name}"));
        }
        if processes.len() > 1 {
            let candidates = processes
                .iter()
                .map(|(name, cmd)| procfile_candidate(cmd, name))
                .collect();
            return RunResolution::Ambiguous(candidates);
        }
    }

    // A3) Makefile with run/start/serve target
    for makefile in ["Makefile", "GNUmakefile", "makefile"] {
        if let Ok(text) = std::fs::read_to_string(root.join(makefile)) {
            let targets = makefile_targets(&text);
            for preferred in ["run", "start", "serve"] {
                if targets.iter().any(|t| t == preferred) {
                    return plan("make", &[preferred], &format!("{makefile} alvo '{preferred}'"));
                }
            }
        }
    }

    // A4) docker compose
    for compose in ["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"] {
        if root.join(compose).exists() {
            return plan("docker", &["compose", "up"], compose);
        }
    }

    // --- Layer B: per-language convention ---

    // Python: Django (manage.py) > main.py
    if root.join("manage.py").exists() {
        return plan("python3", &["manage.py", "runserver"], "manage.py (Django)");
    }
    if root.join("main.py").exists() {
        return plan("python3", &["main.py"], "main.py");
    }

    // Ruby: Rails > rackup
    if root.join("bin").join("rails").exists() {
        return plan("bin/rails", &["server"], "bin/rails (Rails)");
    }
    if root.join("config.ru").exists() {
        return plan("rackup", &[], "config.ru");
    }

    // Elixir: only Phoenix has a canonical start
    if let Ok(mix) = std::fs::read_to_string(root.join("mix.exs")) {
        if mix.contains("phoenix") {
            return plan("mix", &["phx.server"], "mix.exs (Phoenix)");
        }
    }

    // Rust: several bins → ask; one → cargo run; none → nothing to run.
    // NOVO-1 (corpus Fase A, docs/dogfooding.md): lib-only projects used
    // to resolve a `cargo run` that would fail — NotFound is the truth,
    // and the workspace's members are scanned for real binaries.
    if root.join("Cargo.toml").exists() {
        let bins = cargo_bin_names(root);
        if bins.len() > 1 {
            let candidates = bins
                .iter()
                .map(|name| candidate("cargo", &["run", "--bin", name], &format!("binário '{name}'")))
                .collect();
            return RunResolution::Ambiguous(candidates);
        }
        if bins.is_empty() {
            return RunResolution::NotFound;
        }
        return plan("cargo", &["run"], "Cargo.toml");
    }

    // Go: main at the root > single cmd/ > several cmd/ → ask
    if root.join("go.mod").exists() {
        if root.join("main.go").exists() {
            return plan("go", &["run", "."], "main.go na raiz");
        }
        let cmds = go_cmd_packages(root);
        if cmds.len() == 1 {
            let path = format!("./cmd/{}", cmds[0]);
            return plan("go", &["run", &path], &format!("cmd/{}", cmds[0]));
        }
        if cmds.len() > 1 {
            let candidates = cmds
                .iter()
                .map(|name| {
                    candidate("go", &["run", &format!("./cmd/{name}")], &format!("cmd/{name}"))
                })
                .collect();
            return RunResolution::Ambiguous(candidates);
        }
    }

    // .NET: .sln or a single csproj; several csproj without sln → ask
    if root.read_dir().is_ok() {
        let entries: Vec<String> = std::fs::read_dir(root)
            .map(|it| {
                it.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        if entries.iter().any(|name| name.ends_with(".sln")) {
            return plan("dotnet", &["run"], "projeto .NET (.sln)");
        }
        let projects: Vec<&String> = entries.iter().filter(|name| name.ends_with(".csproj")).collect();
        if projects.len() == 1 {
            return plan("dotnet", &["run"], &format!("projeto {}", projects[0]));
        }
        if projects.len() > 1 {
            let candidates = projects
                .iter()
                .map(|name| candidate("dotnet", &["run"], name.as_str()))
                .collect();
            return RunResolution::Ambiguous(candidates);
        }
    }

    // Swift Package
    // Swift: Package.swift with executables — a library-only package has
    // nothing to run, and several executables need a product choice
    // (NOVO-4, corpus: swift-argument-parser ships 9 → plain `swift run`
    // fails). Mirrors the Rust bin handling.
    if root.join("Package.swift").exists() {
        let executables = swift_executable_names(&root.join("Package.swift"));
        if executables.len() > 1 {
            let candidates = executables
                .iter()
                .map(|name| {
                    candidate(
                        "swift",
                        &["run", name],
                        &format!("executável '{name}'"),
                    )
                })
                .collect();
            return RunResolution::Ambiguous(candidates);
        }
        if executables.is_empty() {
            return RunResolution::NotFound;
        }
        return plan("swift", &["run"], "Package.swift");
    }

    // PHP: Laravel (artisan)
    if root.join("artisan").exists() {
        return plan("php", &["artisan", "serve"], "artisan (Laravel)");
    }

    // Java/Kotlin: Spring Boot (declared plugin)
    if let Ok(pom) = std::fs::read_to_string(root.join("pom.xml")) {
        if pom.contains("spring-boot-maven-plugin") {
            return plan("mvn", &["spring-boot:run"], "pom.xml (Spring Boot)");
        }
    }
    for gradle in ["build.gradle", "build.gradle.kts"] {
        if let Ok(text) = std::fs::read_to_string(root.join(gradle)) {
            if text.contains("org.springframework.boot") {
                if root.join("gradlew").exists() {
                    return plan("./gradlew", &["bootRun"], &format!("{gradle} (Spring Boot)"));
                }
                return plan("gradle", &["bootRun"], &format!("{gradle} (Spring Boot)"));
            }
        }
    }

    // Dart: bin/*.dart (shared gate with the briefing runbook — NOVO-3)
    if has_dart_bin(root) {
        return plan("dart", &["run"], "bin/*.dart");
    }

    RunResolution::NotFound
}

/// Procfile: `web: npm start` → argv (no shell; split on spaces —
/// honest limitation: complex quoting is not supported).
fn procfile_plan(cmd: &str, source: &str) -> RunResolution {
    let mut parts = cmd.split_whitespace();
    let program = parts.next().unwrap_or("");
    let args: Vec<&str> = parts.collect();
    plan(program, &args, source)
}

fn procfile_candidate(cmd: &str, label: &str) -> RunCandidate {
    let mut parts = cmd.split_whitespace();
    let program = parts.next().unwrap_or("");
    let args: Vec<&str> = parts.collect();
    candidate(program, &args, label)
}

/// Makefile rule targets (ignores `VAR = ...` assignment lines).
fn makefile_targets(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.contains('=') && !line.starts_with('\t'))
        .filter_map(|line| line.split_once(':'))
        .map(|(name, _)| name.trim().to_string())
        .filter(|name| !name.is_empty() && !name.contains(' '))
        .collect()
}

/// Names of the binaries declared in `Cargo.toml` (default: `src/main.rs`).
/// Binary names exposed by a single package directory: explicit `[[bin]]`,
/// `src/main.rs` (named after the package) and `src/bin/*`.
fn package_bin_names(dir: &Path) -> Vec<String> {
    let mut names = Vec::new();
    let mut package_name: Option<String> = None;
    if let Ok(text) = std::fs::read_to_string(dir.join("Cargo.toml")) {
        if let Ok(doc) = text.parse::<toml::Value>() {
            package_name = doc
                .get("package")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .map(str::to_string);
            if let Some(bins) = doc.get("bin").and_then(|b| b.as_array()) {
                for bin in bins {
                    if let Some(name) = bin.get("name").and_then(|n| n.as_str()) {
                        names.push(name.to_string());
                    }
                }
            }
        }
    }
    if dir.join("src").join("main.rs").exists() {
        names.push(package_name.unwrap_or_else(|| "default".to_string()));
    }
    if let Ok(entries) = std::fs::read_dir(dir.join("src").join("bin")) {
        for entry in entries.flatten() {
            let path = entry.path();
            match path.extension().and_then(|e| e.to_str()) {
                Some("rs") => {
                    if let Some(stem) =
                        path.file_stem().map(|s| s.to_string_lossy().into_owned())
                    {
                        names.push(stem);
                    }
                }
                None if path.is_dir() && path.join("main.rs").exists() => {
                    names.push(entry.file_name().to_string_lossy().into_owned());
                }
                _ => {}
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// Binary names reachable from `root`: its own package plus, when the
/// manifest declares `[workspace].members`, every member's package — a
/// virtual workspace root has no `src/` of its own. NOVO-1 (corpus Fase A,
/// docs/dogfooding.md): a lib-only project must resolve to `NotFound`
/// instead of a doomed `cargo run`, while a workspace with exactly one
/// member binary still resolves to the `cargo run` cargo itself would run.
pub(crate) fn cargo_bin_names(root: &Path) -> Vec<String> {
    let mut names = package_bin_names(root);
    if let Ok(text) = std::fs::read_to_string(root.join("Cargo.toml")) {
        if let Ok(doc) = text.parse::<toml::Value>() {
            if let Some(members) = doc
                .get("workspace")
                .and_then(|w| w.get("members"))
                .and_then(|m| m.as_array())
            {
                let exclude: Vec<String> = doc
                    .get("workspace")
                    .and_then(|w| w.get("exclude"))
                    .and_then(|e| e.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                for dir in crate::detector::expand_members(root, members, &exclude) {
                    names.extend(package_bin_names(&dir));
                }
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// Packages under `cmd/*/main.go`.
/// Whether a Dart package exposes `bin/*.dart` (the only thing `dart run`
/// can execute) — shared with the briefing runbook (NOVO-3).
pub(crate) fn has_dart_bin(root: &Path) -> bool {
    let Ok(bin_dir) = std::fs::read_dir(root.join("bin")) else {
        return false;
    };
    bin_dir
        .flatten()
        .any(|entry| entry.file_name().to_string_lossy().ends_with(".dart"))
}

/// Executable product names declared in a `Package.swift`
/// (`.executableTarget(name: "…")` / `.executable(name: "…")`; the DSL is
/// free-form so `name` may sit on the following line — `\s*` covers it).
pub(crate) fn swift_executable_names(package: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(package) else {
        return Vec::new();
    };
    let re = regex::Regex::new(r#"\.executable(?:Target)?\s*\(\s*name:\s*"([^"]+)""#)
        .expect("static regex");
    let mut names: Vec<String> = re
        .captures_iter(&text)
        .map(|cap| cap[1].to_string())
        .collect();
    names.sort();
    names.dedup();
    names
}

pub(crate) fn go_cmd_packages(root: &Path) -> Vec<String> {
    let mut packages = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root.join("cmd")) {
        for entry in entries.flatten() {
            if entry.path().join("main.go").exists() {
                packages.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    packages.sort();
    packages
}

// ---------------------------------------------------------------------------
// Process supervision (foreground, v1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthCheck {
    Tcp(u16),
    /// URL `http://...` (v1 does not support https); the probe happens in the loop.
    Http(String),
}

/// Parses `--health tcp:3000` / `--health http://localhost:8000/health`.
pub fn parse_health(spec: &str) -> Result<HealthCheck, String> {
    if let Some(port) = spec.strip_prefix("tcp:") {
        return port
            .parse::<u16>()
            .map(HealthCheck::Tcp)
            .map_err(|_| format!("porta inválida em --health: {spec}"));
    }
    if spec.starts_with("http://") {
        // validate the format HERE; the connection probe happens in the readiness loop
        return parse_http_url(spec)
            .map(|_| HealthCheck::Http(spec.to_string()))
            .ok_or_else(|| format!("URL inválida em --health: {spec}"));
    }
    Err(format!(
        "--health aceita 'tcp:<porta>' ou 'http://...' (v1 sem https): {spec}"
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    /// The process terminated on its own.
    Exited,
    /// Never became "ready" within the timeout and was taken down.
    TimedOut,
    /// Interrupted (Ctrl-C / stop flag).
    Stopped,
}

#[derive(Debug)]
pub struct SupervisedOutcome {
    pub state: RunState,
    pub exit_code: Option<i32>,
    pub ready: bool,
    pub port: Option<u16>,
    pub duration_ms: u128,
    pub tail_stdout: Vec<String>,
    pub tail_stderr: Vec<String>,
}

pub struct SuperviseOptions {
    /// Limit while waiting for readiness (does not cap the time used once ready).
    pub timeout: Duration,
    pub health: Option<HealthCheck>,
    /// Candidate ports besides those discovered from the child (Linux).
    pub candidates: Vec<u16>,
    /// Forwards the child's stdout/stderr live (human mode).
    pub stream: bool,
    /// How many trailing lines we keep for diagnose/report.
    pub ring_size: usize,
}

impl Default for SuperviseOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            health: None,
            candidates: Vec::new(),
            stream: true,
            ring_size: 50,
        }
    }
}

/// Runs `child` until it exits, becomes ready (port), or blows the timeout.
pub fn supervise(
    mut child: Child,
    opts: &SuperviseOptions,
    stop: Option<Arc<AtomicBool>>,
) -> std::io::Result<SupervisedOutcome> {
    let start = Instant::now();
    let pid = child.id();

    let stdout_ring: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
    let stderr_ring: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
    let mut readers = Vec::new();
    if let Some(out) = child.stdout.take() {
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let handle = pump(out, stdout_ring.clone(), opts.stream, false, opts.ring_size, done_tx);
        readers.push((handle, done_rx));
    }
    if let Some(err) = child.stderr.take() {
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let handle = pump(err, stderr_ring.clone(), opts.stream, true, opts.ring_size, done_tx);
        readers.push((handle, done_rx));
    }

    let mut ready = false;
    let mut port = None;
    let exit_code;

    let state = loop {
        if stop.as_ref().is_some_and(|f| f.load(Ordering::SeqCst)) {
            exit_code = terminate(&mut child);
            break RunState::Stopped;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_code = status.code();
                break RunState::Exited;
            }
            Ok(None) => {}
            Err(error) => return Err(error),
        }
        if !ready {
            let discovered = child_listen_ports(pid);
            if let Some(found) = check_ready(&opts.health, &opts.candidates, &discovered) {
                ready = true;
                port = Some(found);
            } else if start.elapsed() > opts.timeout {
                exit_code = terminate(&mut child);
                break RunState::TimedOut;
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    };

    let _ = child.wait();
    // Waits for the readers with a DEADLINE: an orphaned descendant (a grandchild
    // that grabbed the pipe) must not hold the CLI forever. If the deadline
    // blows, the thread dies on its own when the fd closes.
    for (handle, done_rx) in readers {
        if done_rx.recv_timeout(Duration::from_secs(2)).is_ok() {
            let _ = handle.join();
        }
    }

    Ok(SupervisedOutcome {
        state,
        exit_code,
        ready,
        port,
        duration_ms: start.elapsed().as_millis(),
        tail_stdout: ring_snapshot(&stdout_ring),
        tail_stderr: ring_snapshot(&stderr_ring),
    })
}

fn ring_snapshot(ring: &Arc<Mutex<VecDeque<String>>>) -> Vec<String> {
    ring.lock().map(|r| r.iter().cloned().collect()).unwrap_or_default()
}

/// Forwards SIGTERM to the GROUP → waits 5s → SIGKILL. Returns the final exit code.
fn terminate(child: &mut Child) -> Option<i32> {
    #[cfg(unix)]
    unsafe {
        // group (catches grandchildren: npm → node) + direct (in case the child is not the leader)
        libc::killpg(child.id() as i32, libc::SIGTERM);
        libc::kill(child.id() as i32, libc::SIGTERM);
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.code(),
            Ok(None) => {
                if Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => break,
        }
    }
    #[cfg(unix)]
    unsafe {
        libc::killpg(child.id() as i32, libc::SIGKILL);
    }
    let _ = child.kill();
    child.wait().ok().and_then(|status| status.code())
}

fn pump<R: Read + Send + 'static>(
    reader: R,
    ring: Arc<Mutex<VecDeque<String>>>,
    stream: bool,
    to_stderr: bool,
    ring_size: usize,
    done: std::sync::mpsc::Sender<()>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut lines = BufReader::new(reader).lines();
        while let Some(Ok(line)) = lines.next() {
            if stream {
                if to_stderr {
                    eprintln!("{line}");
                } else {
                    println!("{line}");
                }
            }
            if let Ok(mut ring) = ring.lock() {
                if ring.len() >= ring_size {
                    ring.pop_front();
                }
                ring.push_back(line);
            }
        }
        let _ = done.send(());
    })
}

fn check_ready(
    health: &Option<HealthCheck>,
    candidates: &[u16],
    discovered: &[u16],
) -> Option<u16> {
    match health {
        Some(HealthCheck::Tcp(port)) => {
            if tcp_ready(*port) {
                Some(*port)
            } else {
                None
            }
        }
        Some(HealthCheck::Http(url)) => http_probe(url),
        None => candidates
            .iter()
            .chain(discovered.iter())
            .find(|port| tcp_ready(**port))
            .copied(),
    }
}

fn tcp_ready(port: u16) -> bool {
    let Ok(addr) = format!("127.0.0.1:{port}").to_socket_addrs() else {
        return false;
    };
    let Some(addr) = addr.into_iter().next() else {
        return false;
    };
    TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok()
}

/// Decomposes `http://host[:port]/caminho` into (port, path).
fn parse_http_url(url: &str) -> Option<(u16, String)> {
    let rest = url.strip_prefix("http://")?;
    let (host_port, path) = match rest.find('/') {
        Some(index) => (&rest[..index], rest[index..].to_string()),
        None => (rest, "/".to_string()),
    };
    if host_port.is_empty() {
        return None;
    }
    let port = match host_port.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() => port.parse().ok()?,
        _ => 80,
    };
    Some((port, path))
}

/// Minimal http GET probe: any response byte = up. Returns the port.
fn http_probe(url: &str) -> Option<u16> {
    let (port, path) = parse_http_url(url)?;
    let mut stream = TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").to_socket_addrs().ok()?.next()?,
        Duration::from_millis(250),
    )
    .ok()?;
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).ok()?;
    let mut byte = [0u8; 1];
    match stream.read(&mut byte) {
        Ok(count) if count > 0 => Some(port),
        _ => None,
    }
}

/// Ports in LISTEN state of the process and of its descendants (Linux; empty on
/// other OSes). Descendants matter: `npm run start` spawns `node` as a
/// grandchild, and the listening port is the grandchild's.
#[cfg(target_os = "linux")]
pub fn child_listen_ports(pid: u32) -> Vec<u16> {
    use std::collections::HashSet;

    let mut inodes: HashSet<String> = HashSet::new();
    for process in process_tree(pid) {
        if let Ok(entries) = std::fs::read_dir(format!("/proc/{process}/fd")) {
            for entry in entries.flatten() {
                if let Ok(link) = std::fs::read_link(entry.path()) {
                    if let Some(text) = link.to_str() {
                        if let Some(inode) = text
                            .strip_prefix("socket:[")
                            .and_then(|s| s.strip_suffix(']'))
                        {
                            inodes.insert(inode.to_string());
                        }
                    }
                }
            }
        }
    }
    if inodes.is_empty() {
        return Vec::new();
    }

    let mut ports = Vec::new();
    for table in ["/proc/net/tcp", "/proc/net/tcp6"] {
        if let Ok(text) = std::fs::read_to_string(table) {
            for line in text.lines().skip(1) {
                let columns: Vec<&str> = line.split_whitespace().collect();
                // column 3 = state (0A = LISTEN); column 9 = inode
                if columns.len() > 9 && columns[3] == "0A" && inodes.contains(columns[9]) {
                    if let Some(port) = columns[1].rsplit(':').next() {
                        if let Ok(port) = u16::from_str_radix(port, 16) {
                            ports.push(port);
                        }
                    }
                }
            }
        }
    }
    ports
}

/// Process tree: `pid` + descendants (`/proc/<pid>/task/<pid>/children`).
#[cfg(target_os = "linux")]
fn process_tree(pid: u32) -> Vec<u32> {
    let mut processes = vec![pid];
    let mut queue = vec![pid];
    while let Some(process) = queue.pop() {
        if let Ok(text) = std::fs::read_to_string(format!("/proc/{process}/task/{process}/children")) {
            for child in text.split_whitespace().filter_map(|value| value.parse::<u32>().ok()) {
                processes.push(child);
                queue.push(child);
            }
        }
    }
    processes
}

#[cfg(not(target_os = "linux"))]
pub fn child_listen_ports(_pid: u32) -> Vec<u16> {
    Vec::new()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::process::Command;
    use tempfile::TempDir;

    fn ready(resolution: RunResolution) -> RunPlan {
        match resolution {
            RunResolution::Ready(plan) => plan,
            other => panic!("esperava Ready, veio {other:?}"),
        }
    }

    #[test]
    fn npm_start_declared() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name":"x","scripts":{"start":"node server.js"}}"#,
        )
        .unwrap();
        let plan = ready(resolve_run(dir.path()));
        assert_eq!(plan.command, "npm");
        assert_eq!(plan.args, vec!["run", "start"]);
        assert_eq!(plan.source, "package.json scripts.start");
    }

    #[test]
    fn npm_without_start_does_not_guess() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"name":"x"}"#).unwrap();
        assert_eq!(resolve_run(dir.path()), RunResolution::NotFound);
    }

    #[test]
    fn procfile_web_wins() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("Procfile"), "worker: node w.js\nweb: npm start\n").unwrap();
        let plan = ready(resolve_run(dir.path()));
        assert_eq!(plan.command, "npm");
        assert_eq!(plan.source, "Procfile web");
    }

    #[test]
    fn procfile_multiple_without_web_asks() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("Procfile"), "worker: node w.js\n clock: node c.js\n").unwrap();
        match resolve_run(dir.path()) {
            RunResolution::Ambiguous(candidates) => assert_eq!(candidates.len(), 2),
            other => panic!("esperava Ambiguous, veio {other:?}"),
        }
    }

    #[test]
    fn makefile_target_run() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("Makefile"), "build:\n\tgo build\n\nrun:\n\t./app\n").unwrap();
        let plan = ready(resolve_run(dir.path()));
        assert_eq!(plan.command, "make");
        assert_eq!(plan.args, vec!["run"]);
    }

    #[test]
    fn declared_beats_convention() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"start":"node s.js"}}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("Makefile"), "run:\n\t./app\n").unwrap();
        assert_eq!(ready(resolve_run(dir.path())).source, "package.json scripts.start");
    }

    #[test]
    fn python_django_and_main() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("main.py"), "print('oi')").unwrap();
        assert_eq!(ready(resolve_run(dir.path())).command, "python3");

        std::fs::write(dir.path().join("manage.py"), "").unwrap();
        let plan = ready(resolve_run(dir.path()));
        assert_eq!(plan.args, vec!["manage.py", "runserver"]);
    }

    #[test]
    fn rust_multiple_bins_asks() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[[bin]]\nname = \"a\"\npath = \"src/a.rs\"\n\n[[bin]]\nname = \"b\"\npath = \"src/b.rs\"\n",
        )
        .unwrap();
        match resolve_run(dir.path()) {
            RunResolution::Ambiguous(candidates) => {
                assert_eq!(candidates.len(), 2);
                assert_eq!(candidates[0].command, "cargo");
            }
            other => panic!("esperava Ambiguous, veio {other:?}"),
        }
    }

    #[test]
    fn rust_single_bin_cargo_run() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src").join("main.rs"), "fn main(){}").unwrap();
        assert_eq!(ready(resolve_run(dir.path())).command, "cargo");
    }

    #[test]
    fn rust_lib_only_is_not_found() {
        // NOVO-1: corpus Fase A caught serde/anyhow (lib-only) resolving
        // a `cargo run` that would fail. No binaries = nothing to run.
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src").join("lib.rs"), "").unwrap();
        assert_eq!(resolve_run(dir.path()), RunResolution::NotFound);
    }

    #[test]
    fn rust_virtual_workspace_with_one_member_bin_runs() {
        // The DevForge repo shape: virtual root + exactly one member bin
        // is genuinely runnable — `cargo run` at the root works there.
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("crates/app").join("src")).unwrap();
        std::fs::write(
            dir.path().join("crates/app/Cargo.toml"),
            "[package]\nname = \"app\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("crates/app/src/main.rs"),
            "fn main(){}",
        )
        .unwrap();
        let plan = ready(resolve_run(dir.path()));
        assert_eq!(plan.command, "cargo");
        assert_eq!(plan.args, vec!["run"]);
    }

    #[test]
    fn rust_virtual_workspace_lib_only_is_not_found() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("crates/lib").join("src")).unwrap();
        std::fs::write(
            dir.path().join("crates/lib/Cargo.toml"),
            "[package]\nname = \"libcrate\"\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("crates/lib/src/lib.rs"), "").unwrap();
        assert_eq!(resolve_run(dir.path()), RunResolution::NotFound);
    }

    #[test]
    fn rust_src_bin_autodiscovery() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        std::fs::create_dir_all(dir.path().join("src/bin/cli")).unwrap();
        std::fs::write(
            dir.path().join("src/bin/cli/main.rs"),
            "fn main(){}",
        )
        .unwrap();
        assert_eq!(ready(resolve_run(dir.path())).command, "cargo");
    }

    #[test]
    fn swift_lib_only_is_not_found() {
        // NOVO-4: a library-only Package.swift has nothing to run.
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("Package.swift"),
            "// swift-tools-version:5.9\nlet package = Package(name: \"x\", targets: [.target(name: \"Lib\")])\n",
        )
        .unwrap();
        assert_eq!(resolve_run(dir.path()), RunResolution::NotFound);
    }

    #[test]
    fn swift_single_executable_runs() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("Package.swift"),
            ".executableTarget(name: \"cli\", dependencies: [])",
        )
        .unwrap();
        let plan = ready(resolve_run(dir.path()));
        assert_eq!(plan.command, "swift");
        assert_eq!(plan.args, vec!["run"]);
    }

    #[test]
    fn swift_several_executables_ask() {
        // Real shape of swift-argument-parser: `name:` on the line after
        // the marker — plain `swift run` would fail with several products.
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("Package.swift"),
            ".executableTarget(\n    name: \"generate-manual\"\n)\n.executable(name: \"other\", dependencies: [])\n",
        )
        .unwrap();
        match resolve_run(dir.path()) {
            RunResolution::Ambiguous(candidates) => {
                assert_eq!(candidates.len(), 2);
                assert_eq!(candidates[0].command, "swift");
                assert_eq!(candidates[0].args, vec!["run", "generate-manual"]);
            }
            other => panic!("esperava Ambiguous, veio {other:?}"),
        }
    }

    #[test]
    fn go_cmd_single_and_multiple() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("go.mod"), "module x\n").unwrap();
        std::fs::create_dir_all(dir.path().join("cmd").join("api")).unwrap();
        std::fs::write(dir.path().join("cmd").join("api").join("main.go"), "package main").unwrap();

        let plan = ready(resolve_run(dir.path()));
        assert_eq!(plan.args, vec!["run", "./cmd/api"]);

        std::fs::create_dir_all(dir.path().join("cmd").join("worker")).unwrap();
        std::fs::write(dir.path().join("cmd").join("worker").join("main.go"), "package main").unwrap();
        assert!(matches!(resolve_run(dir.path()), RunResolution::Ambiguous(_)));
    }

    #[test]
    fn spring_boot_maven() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("pom.xml"),
            "<plugin>spring-boot-maven-plugin</plugin>",
        )
        .unwrap();
        let plan = ready(resolve_run(dir.path()));
        assert_eq!(plan.args, vec!["spring-boot:run"]);
    }

    #[test]
    fn empty_dir_is_not_found() {
        let dir = TempDir::new().unwrap();
        assert_eq!(resolve_run(dir.path()), RunResolution::NotFound);
    }

    #[test]
    fn health_parse() {
        assert_eq!(parse_health("tcp:3000"), Ok(HealthCheck::Tcp(3000)));
        // valid http validates WITHOUT connecting (the server may still be up)
        assert_eq!(
            parse_health("http://localhost:8080/health"),
            Ok(HealthCheck::Http("http://localhost:8080/health".into()))
        );
        assert!(parse_health("https://x").is_err());
        assert!(parse_health("tcp:abc").is_err());
        assert!(parse_health("http://").is_err());
    }

    #[test]
    fn supervise_one_shot_success_and_failure() {
        let ok = Command::new("true")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let outcome = supervise(ok, &SuperviseOptions::default(), None).unwrap();
        assert_eq!(outcome.state, RunState::Exited);
        assert_eq!(outcome.exit_code, Some(0));

        let bad = Command::new("false")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let outcome = supervise(bad, &SuperviseOptions::default(), None).unwrap();
        assert_eq!(outcome.state, RunState::Exited);
        assert_eq!(outcome.exit_code, Some(1));
    }

    #[test]
    fn supervise_detects_port_and_stops_on_flag() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = {
            let stop = stop.clone();
            std::thread::spawn(move || {
                // keeps the listener alive; waits until it connects (readiness) and
                // gives the supervise loop time to observe the port before asking to stop
                for _ in 0..100 {
                    if TcpStream::connect_timeout(
                        &format!("127.0.0.1:{port}").parse().unwrap(),
                        Duration::from_millis(100),
                    )
                    .is_ok()
                    {
                        std::thread::sleep(Duration::from_millis(400));
                        stop.store(true, Ordering::SeqCst);
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            })
        };

        let child = Command::new("sleep")
            .arg("30")
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let opts = SuperviseOptions {
            timeout: Duration::from_secs(10),
            candidates: vec![port],
            ..SuperviseOptions::default()
        };
        let outcome = supervise(child, &opts, Some(stop)).unwrap();
        stop_thread.join().unwrap();
        assert!(outcome.ready, "devia ter ficado pronto");
        assert_eq!(outcome.port, Some(port));
        assert_eq!(outcome.state, RunState::Stopped);
    }

    #[test]
    fn supervise_timeout_kills_process() {
        let child = Command::new("sleep")
            .arg("30")
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let opts = SuperviseOptions {
            timeout: Duration::from_millis(400),
            ..SuperviseOptions::default()
        };
        let started = Instant::now();
        let outcome = supervise(child, &opts, None).unwrap();
        assert_eq!(outcome.state, RunState::TimedOut);
        assert!(!outcome.ready);
        assert!(started.elapsed() < Duration::from_secs(10), "devia ter matado logo");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn child_listen_ports_finds_real_port() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let ports = child_listen_ports(std::process::id());
        assert!(ports.contains(&port), "porta {port} deveria aparecer em {ports:?}");
    }

    #[test]
    fn ring_captures_final_output() {
        let child = Command::new("echo")
            .arg("linha-de-teste")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let outcome = supervise(child, &SuperviseOptions { stream: false, ..Default::default() }, None).unwrap();
        assert!(outcome.tail_stdout.iter().any(|line| line.contains("linha-de-teste")));
    }
}
