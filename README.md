<div align="center">

# Zavik

**The open-source dev CLI & AI-orchestration engine** — detect your stack,
audit your dependency **supply chain**, hand any AI agent a one-call briefing.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![CI](https://github.com/Jhongamers/devforge/actions/workflows/ci.yml/badge.svg)](https://github.com/Jhongamers/devforge/actions/workflows/ci.yml)

[English](README.md) · [Português (Brasil)](README.pt-BR.md)

</div>

**Works with any agent** — Claude Code, Codex, OpenCode, Cursor, Antigravity,
aider, Copilot — or with no agent at all. Zavik is a plain CLI: stable
`--json` output and exit codes *are* the interface; there is no vendor plugin
to install. *(Compatibility, not affiliation.)*

## Install

**Quick install** (linux x64/arm64, macOS arm64; needs the `v0.1.0` release
published):

```bash
curl -fsSL https://raw.githubusercontent.com/Jhongamers/devforge/main/install.sh | sh
```

Detects your OS/arch, downloads the release tarball, **verifies its sha256**
(no skip flag) and installs to `~/.local/bin`. Prefer downloading the script,
reading it, then running it — our own `audit` classifies `curl | sh` as a
risk. Windows: grab the `.zip` from
[Releases](https://github.com/Jhongamers/devforge/releases). Overrides:
`ZAVIK_VERSION`, `ZAVIK_INSTALL_DIR`, `ZAVIK_BASE_URL` (mirrors,
air-gapped, tests).

**From source** (always works):

```bash
git clone https://github.com/Jhongamers/devforge && cd zavik
cargo build --release
cp target/release/zavik ~/.local/bin/   # or anywhere on your PATH
```

Requires Rust stable. Every `v*` tag publishes binaries
(linux x64/arm64, macOS arm64, Windows) + `.sha256` checksums on
[Releases](https://github.com/Jhongamers/devforge/releases); the installer
above and `cargo install` cover the main doors (package-manager one-liners
are on the launch roadmap).

> **For AI agents:** clone + `cargo build --release` is the canonical
> zero-mystery install. After that, `zavik context --json` is the fastest
> way to learn an unfamiliar repo.

## Quickstart

```bash
cd some-project
zavik                    # detect the stack
zavik context --json     # one-call briefing (runbook + entrypoints + trust)
zavik audit              # supply-chain audit
zavik up                 # audit → prepare → run, with intact gates
```

## Commands

```bash
zavik [path]                     # detect the stack (default)
zavik detect [path]              # same
zavik analyze [path] [flags]     # analyze the project's dependencies
zavik context [path] [flags]     # briefing: runbook + entrypoints + risks (1 call)
zavik diagnose [flags]           # summarize an error: root cause + fix
zavik env [path] [flags]         # environment fingerprint: tool versions + what's missing
zavik run [path] [--detach]      # boot the project (--detach: background + PID)
zavik up [url|path] [flags]      # pipeline: clone? → audit → prepare → run (same gates)
zavik audit [path] [flags]       # supply-chain audit (--fail-on gates CI)
zavik license status|install|remove # offline license (Ed25519 signature, local)
zavik data status|update         # risk dataset (free: status; Pro: update)
zavik prepare [path] [flags]     # install dependencies (no Docker)
zavik docker  [path] [flags]     # generate Dockerfile + build the image
```

Flags:

| Flag        | Effect                                                             |
| ----------- | ------------------------------------------------------------------ |
| `--dry-run` | Shows the plan and **runs nothing**.                                |
| `--yes`/`-y`| Approves everything automatically, without asking.                  |
| `--force`   | Allows **overwriting** existing files (e.g. `Dockerfile`).          |
| `--online`  | (`analyze`/`audit`) Queries registries (versions / hooks).          |
| `--json`    | Structured output for agents (stdout = pure JSON).                  |
| `--all`     | Descends into subfolders looking for projects (monorepo/nested).    |
| `--fail-on` | (`audit`) Minimum severity that fails CI (exit 1).                  |
| `--policy`  | (`audit`) TOML policy file (`fail_on`, `allowlist`).                |
| `--ignore`  | (`audit`) TOML suppressions with **mandatory expiry**.              |
| `--sarif`   | (`audit`) Emits SARIF 2.1.0 (GitHub Code Scanning).                 |
| `--url`     | (`data`) Dataset origin: `https://...` or `file://<path>`           |
|             | (alternative: env `ZV_DATASET_URL`).                                |
| `--message` | (`diagnose`) Inline error text.                                     |
| `--input`   | (`diagnose`) File containing the error output.                      |
| `--timeout` | (`run`) Seconds to wait for readiness (default 60).                 |
| `--health`  | (`run`) Explicit readiness: `tcp:<port>` \| `http://...`.           |
| `--until`   | (`up`) Stop after the stage: `clone\|audit\|prepare\|run`.          |
| `--skip`    | (`up`) Skip stage(s), comma-separated (never `clone`).              |

### JSON output (AI orchestration)

With `--json`, every command returns **compact JSON on stdout** (diagnostic
noise goes to stderr). Built for callers that are **agents**: human text makes
the model interpret and burn tokens; JSON delivers the fact ready-made. Every
document carries `"schema": 1`.

```bash
zavik detect  --json          # what is this project (ground truth)
zavik context --json          # briefing: runbook + entrypoints + risks (1 call)
zavik analyze --json          # sick dependencies
zavik diagnose --json         # root cause of an error (+ fix)
zavik env --json              # environment fingerprint: OS/arch, tool versions, missing
zavik run --dry-run --json    # resolved start command + where it came from
zavik run --json              # plan without executing (agent mode) or final result
zavik run --detach --json     # background: pid + log + stop_command (no supervision)
zavik up --dry-run --json     # pipeline plan (stage by stage, nothing executed)
zavik up --json               # JSON Lines: 1 object per stage + final summary
zavik audit --json            # supply-chain report (score, factors, verdict)
zavik license status --json   # installed license (plan, holder, expires, valid)
zavik data status --json      # dataset installed? version, age, staleness (free)
zavik data update --json      # Pro: downloads+verifies; no license → no_license, exit 1
zavik prepare --json          # plan + trust verification (does not execute)
zavik docker  --json          # same, Docker mode
```

> `prepare`/`docker` with `--json` deliver the **plan** and **risks**
> structurally and **do not execute** — execution is interactive/human. It's the
> "agent proposes, human approves" combo.

Example (`detect --json`, trimmed):
```json
{"schema":1,"command":"detect","stacks":[{"language":"Rust","package_manager":"cargo","frameworks":["Tokio"],"dependencies":[{"name":"serde","version":"=1.0.100","kind":"Runtime"}]}]}
```

> **The token-savings hook:** everything the CLI does is local deterministic
> Rust = **zero AI tokens**. The LLM is the brain (decides *what*), the CLI is
> the hands (knows *how*, with permission + verification). An agent calls
> `detect --json` and gets the fact instead of reading N manifest files.

### Language (`--lang`)

Human CLI text: **English by default** (flipped at the pre-launch i18n
milestone); `--lang pt-BR` (also `--lang=pt-BR` and `ZAVIK_LANG=pt-BR`,
or a BR system locale `LC_ALL`/`LANG=pt_BR…`) switches the messages to
Brazilian Portuguese, `--lang en` forces English back. **Machine output
never switches**: JSON keys/values, SARIF and exit codes are always
English. Migration design and status: `docs/design-i18n.md`.

```bash
zavik env --lang en            # human sections in English
ZAVIK_LANG=en zavik audit   # same, via env (agents/CI)
```

### One-call briefing (`context`)

An AI's biggest expense in an unfamiliar project is **exploration**
(glob/grep/reading the README to figure out "how do I build/test/run this?").
The `context` answers all of it **in a single call**:

```bash
zavik context --json
```

```json
{"schema":1,"command":"context",
 "runbook":{"setup":["npm install"],"build":["npm run build"],"test":["npm test"],"run":["npm run dev"],"lint":["npm run lint"]},
 "entrypoints":["src/index.js"],
 "key_files":[{"kind":"manifest","path":"package.json"},{"kind":"lockfile","path":"package-lock.json"},{"kind":"ci","path":".github/workflows"}],
 "trust":{"risks":[],"blocking":0}}
```

- **runbook**: `setup`/`build`/`test`/`run`/`lint` commands, taken from
  `package.json` `scripts`, `Makefile`/`justfile` targets, wrappers
  (`mvnw`/`gradlew`) and per-language conventions.
- **entrypoints**: entry files (`src/main.rs`, `index.js`,
  `cmd/api/main.go`, ...).
- **key_files**: manifest, lockfile, CI, Dockerfile, `.env.example`, README.
- Joins `detect` + `trust` + all of that — replaces N explorations with 1 JSON.

Without `--json`, the same content is shown in readable sections
(HOW TO RUN / ENTRYPOINTS / KEY FILES / TRUST).

> **Honesty:** the runbook is *inferred* (manifests + conventions). It gives the
> **most likely** command, not a guarantee it runs unmodified — projects with
> peculiar builds may need manual adjustment. CI YAML is not parsed (only
> listed in `key_files`).

### Error diagnosis (`diagnose`)

When a command fails, the literal error is almost always a **symptom, not the
cause**. `cannot find -lssl` looks like a linking problem, but the cause is
"missing `libssl-dev`"; `node-gyp ERR` looks like an npm problem, but the cause
is "missing python3/gcc". `diagnose` matches **known patterns** across the
whole output and returns the **real** cause + the fix, with the "proof"
(the lines that matched):

```bash
# 1) paste the error directly
zavik diagnose --message "sh: 1: cargo: not found"
#   → fix: `cargo` is not on PATH — run `zavik prepare` for the toolchain

# 2) or pipe (without leaving your build flow)
my_build 2>&1 | zavik diagnose

# 3) or from a file
zavik diagnose --input build.log --json
```

Local deterministic Rust (zero tokens). If **no** rule matches, it is honest
and returns the lines that look most like the error (a summary) instead of
pretending it understood. Why not `zavik run <command>`? To avoid adding a
new arbitrary-execution surface — you/the agent already have the error; only
the translation is missing.

### Run the project (`run`)

`run` closes the loop: **audit → prepare → run**. It **resolves how the
project boots** and boots it — never silently guesses:

1. **Declared** (the project says so): `package.json` `scripts.start`,
   Procfile (`web:`), `make run/start/serve`, `docker compose up`.
2. **Convention**: Django, Rails, Phoenix, `cargo run`, `go run .`,
   `dotnet run`, Spring Boot, Laravel, `dart run`…
3. **Ambiguous? It asks.** Nothing found? It says so honestly (won't run a
   guess).

```bash
zavik run                      # resolves, ASKS permission to execute, boots
zavik run --detach             # background: returns pid + log (no readiness wait)
zavik run --dry-run            # only shows the resolved command (no execution)
zavik run --timeout 90         # wait up to 90s for readiness (default 60)
zavik run --health http://localhost:3000/health   # explicit readiness
zavik run . -- --port 3001     # args after -- go to the start command
zavik run --json               # structured plan/result (agent mode)
```

**Detached:** `--detach` starts the project in its **own session** — closing
the terminal doesn't take the dev server down — and returns `pid`, `log`
(`.zavik/run.log`, follow with `tail -f`) and `stop_command` (e.g.
`kill 1234`). The trade is honest: it does **not** wait for readiness or
supervise; run without `--detach` for health check + crash→diagnose. Same
gate: executing is critical risk, `--yes` doesn't cover it. *(On Windows the
detached process stays tied to the console — known limitation.)*

**What makes "it worked" honest:** a server doesn't terminate, so the exit code
isn't enough — `run` detects a **port accepting connections** (heuristics:
`--health`, candidate ports, and on Linux the **real** port of the process and
its descendants via `/proc` — catches the grandchild `npm → node`).
Ctrl-C/SIGTERM takes down the **whole process group** (SIGTERM → wait →
SIGKILL). A crash before booting becomes a `diagnose` entry (cause + fix).

**Honest limitations (v1):** readiness is port heuristics (not a guarantee);
`--health http` has no https; no port discovery on macOS (use
`--health`/candidates); `--json` doesn't stream live (delivers the tail in the
result); `--detach` doesn't supervise readiness; POSIX (Linux/macOS).
And the standing rule: **executing is critical risk — `--yes` never covers
it**, the human decides (agent mode: `--json` delivers the plan, doesn't
execute).

### The full pipeline (`up`)

The product principle in one command: every stage works **isolated AND
composed**. `up` chains the pipeline through **literally the same code paths**
of the isolated commands — composing does not weaken security:

```bash
zavik up                        # audit → prepare → run in the current directory
zavik up ./my-project           # same in another path
zavik up https://github.com/x/y # safe clone (--depth 1, no submodules,
                                   # no hooks; destination = ./y — never overwrites)
zavik up --until audit          # stop after the audit (useful in CI)
zavik up --skip run             # prepare only, don't boot
zavik up --dry-run              # shows the stage plan; executes nothing
zavik up --json                 # JSON Lines: 1 object per stage + final summary
```

**Per-stage gates, intact:** trust/toolchain/installation checks and the `run`
execution ask exactly as in the isolated commands — and **a security abort
stops the pipeline BEFORE `run`, always** (non-interactive denies by default;
`--yes` never covers code execution). In `--json` (agent mode), the audit
delivers a real result, the prepare delivers the **plan** (no execution) and
the `run` denies without a terminal — the human is the one who executes.

**Honest limitations (v1):** the final `run` is foreground (the `up` ends when
the app ends — `--detach` is v1.1); no resume (failed? run it again); the clone
requires `git` and a destination that doesn't exist yet.

### Supply-chain audit (`audit`)

The `audit` answers "**can I trust** this code into my company?". It borrows
the heart of the established tools — **Socket.dev** (risk factors +
confluence), **Snyk/cargo-audit** (policy + `fail_on` + exit code) and
**OWASP** (FP suppression) — with three locks of our own:

1. **No single signal is a verdict.** Severity comes from the *sum* of weights
   (confluence) and only a score ≥ 25 becomes a finding. So `^4.18.0` (a normal
   range) or a benign `postinstall` (`node-gyp rebuild`, `prebuild-install`)
   **don't** trigger alerts — tested against the real esbuild.
2. **We classify the hook's CONTENT**, not just its existence: `curl|sh`,
   base64+eval, reverse shells and credential access are suspicious; native
   builds/husky are expected.
3. **Suppression requires expiry** (`expires` in the TOML) — an ignore never
   becomes an eternal shield for a compromised package; an expired suppression
   re-activates the alert and is listed for re-triage.

```bash
zavik audit                      # offline: origin (registry/git/path) + pin
zavik audit --online             # + install hooks and npm downloads
zavik audit --fail-on high       # exit 1 on high/critical (gates CI)
zavik audit --sarif > results.sarif   # SARIF 2.1.0 for GitHub Code Scanning
zavik audit --policy .zavik/policy.toml --ignore .zavik/ignore.toml
```

Policy (`.zavik/policy.toml`, versioned):
```toml
fail_on = "high"
allowlist = ["my-internal-package"]   # expected internal git → no alert
```
Suppression (`.zavik/ignore.toml` — `expires` is **mandatory**):
```toml
[[ignore]]
package = "some-package"
factor = "non-registry"
reason = "audited internal fork"
expires = "2027-01-01"
```

**Honesty:** we analyze the install script's *text*, not the files it invokes
(`node install.js`) — it's a signal, not a guarantee. Typosquat detection by
name similarity and lockfile integrity verification are on the roadmap (not
implemented yet). `--online` queries the npm registry.

#### CI/CD — GitHub Action (`action.yml`)

The repository ships a **composite action** that runs the audit on PRs:
downloads the binary from the Release (with `sha256` verification), publishes
the SARIF to Code Scanning and **fails the job** via the exit code.
Packaging comes with it — `.github/workflows/release.yml` publishes binaries
(linux x64/arm64, macOS arm64, windows) on every `v*` tag:

```bash
git tag v0.1.0 && git push origin v0.1.0   # publishes the Releases
```

Usage in the consumer's repository (`.github/workflows/audit.yml`):
```yaml
name: supply-chain audit
on: [pull_request]
permissions:
  contents: read
  security-events: write   # needed to publish the SARIF
jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: Jhongamers/devforge@v0.1.0
        with:
          fail-on: high
          # version: v0.1.0   # pinned tag (on purpose, not "latest")
          # sha256: <hash>    # optional: pins integrity at the action level
          # online: "true"    # default; offline the score only reaches Medium
```

**Honesty about CI:**
- The **gate always works**: `fail-on` → exit 1 fails the job, depending on
  nothing but the binary. But note: **offline** the audit only sees origin +
  pin, so the score never exceeds **Medium** — with `fail-on high` an offline
  audit would never fail anything. That's why the action defaults to
  `online: "true"` (hook *content* is what produces High/Critical).
- **Inline PR annotation** (SARIF) requires Code Scanning: free on public
  repositories; on **private** repositories it requires GitHub Advanced
  Security (paid). Without it, set `upload-sarif: "false"` and keep the gate.
- Inputs go through `env:` (not interpolated into `run:`) — immune to script
  injection; binary verified by `sha256`; ref **pinned** by tag.
- The action covers Linux (x64/arm64) and macOS (arm64). Windows: download the
  `.zip` from the Release manually (the binary is published).

#### License (`license`) — monetization foundation (offline)

Chosen model: **open core + subscription**. The CLI is free; the license is
the key that unlocks (and will unlock) what's paid — with zero network calls.

```bash
zavik license install <key|@file>   # validates locally (Ed25519) and installs
zavik license status [--json]       # plan, holder, expiry, validity
zavik license remove
```

- **Format:** token `ZV1.<payload-b64url>.<signature>`; the binary embeds only
  the **public** key. 100% local verification, stored at
  `~/.config/zavik/license.key`.
- **Honesty:** it's an **anti-dishonesty** gate, not an anti-attacker one —
  patching the binary bypasses it (same deal as TablePlus/Transmit). It also
  doesn't defend against a system clock rolled back. Real protection arrives
  with the server-side phase.
- **What it blocks today:** only `data update` (the Pro dataset below).
  Everything else in the CLI — including `context`, which saves AI tokens —
  stays **100% free**. It's the foundation of the server-side phase, not DRM.
- License minting (vendor-only, never ships in the binary):
  `cargo run --example license_tool -- genkey|mint ...` — the private key
  stays with the vendor, **never** in the repo.

#### Pro risk dataset (`data`)

Today's offline `audit` (confluence factors, `--online`) is the **free tier,
intact**. `data update` layers **signed, fresh** risk data on top — recent
advisories (GitHub **reviewed** GHSA, ~90-day window) and an **exact** kill
list of typosquats/malicious packages (precise title patterns measured against
the real database — no fuzzy matching), downloaded from the Releases here and
verified with the embedded public key.

```bash
zavik data status [--json]               # free, offline: version, age, staleness
zavik data update [--json] [--url <url>] # Pro: downloads, VERIFIES, installs (needs license)
```

| | Free (today) | Pro (license + `data update`) |
| --- | --- | --- |
| Offline `audit` (confluence, controlled FP) | ✅ | ✅ |
| `context` (token savings), `run`, `up`, `prepare`... | ✅ | ✅ |
| Recent advisories by name + kill list in `audit` | — | ✅ `dataset_findings` section |
| Daily dataset updates (signed Release asset) | — | ✅ |
| `data status` (dataset state) | ✅ (free) | ✅ |

Rules and limits (by design, see `docs/design-data.md`):

- **Gate only on `update`** — no valid license → `no_license`/`license_invalid`,
  exit 1, **without touching the network**. `status` is free.
- **Signature required**: the Ed25519 envelope is verified before writing a
  single byte; a refusal (tampered/wrong key) leaves the previous dataset
  intact. The dataset key is **separate** from the license key (blast radius).
- **Honest staleness**: datasets older than 14 days get a warning — freshness
  is part of the product, not hidden.
- **Controlled FP**: advisories match by **name only** → a `low` finding "possible
  exposure — check your version" (no ecosystem range-matching in v1); the kill
  list matches **exactly** → `high`. **No** fuzzy matching, ever.
- **The section doesn't touch your CI**: it changes neither `score`, `passed`
  nor `--fail-on` — free and Pro audits stay comparable; without a dataset the
  JSON is **byte identical** to before (no `dataset` key).

### Dependency analysis (`analyze`)

`analyze` is **read-only** — it installs and changes nothing. Two layers:

- **Offline (always runs):** wildcard/unpinned versions (`*`, `latest`, empty),
  dependencies without a version, dependencies from outside the registry
  (git/path/file), **missing lockfile**, and the same dependency declared with
  different versions.
- **Online (`--online`):** queries **crates.io / npm / PyPI** and warns when a
  newer version exists than the one you asked for. Capped at 40 requests per
  run (rate limits and time).

```
⚠️  Rust: 'serde' has a newer version on the registry: 1.0.100 -> 1.0.229.
⚠️  JavaScript: no lockfile found (expected: package-lock.json or ...).
ℹ️  JavaScript: 'my-lib' comes from 'git+https://...' (outside the registry).
```

> Unlike `prepare`/`docker`, `analyze` **asks no permission** because it runs
> nothing on your machine — only `--online` makes read-only HTTP requests.

### Permissions

By default Zavik **does nothing on its own**: before each sensitive action
(install, generate/overwrite a file, run `docker build`) it asks:

```
🔐 Permission required: Install dependencies (npm)
   action : npm install
   reason : Downloads and installs the packages in package.json.
   Run it? [y]es / [n]o / [a]ll:
```

- `y` runs just that step, `n` skips, `a` approves all remaining.
- Without a terminal (non-interactive execution) it **denies by default**, for
  safety.
- Existing files are **never** overwritten without `--force`.

### Security & supply-chain (automatic verification)

A manifest (`package.json`, `Cargo.toml`, ...) is **untrusted input**: a
malicious project (or an agent driving Zavik unknowingly) could ask it to
install arbitrary code. So **before** any `prepare`/`docker` runs, a
**trust pre-flight** executes that does not blindly trust the manifest:

- **npm lifecycle scripts** (`preinstall`/`install`/`postinstall`/`prepare`)
  → run arbitrary code on `npm install`. We show the exact command. This covers
  **both the root project and every dependency**: before `npm install`, we
  query the registry (without downloading tarballs) the install hooks of each
  dependency and aggregate everything into a single warning — e.g.
  `esbuild → postinstall: node install.js`.
- **Dependency outside the registry** (git/path/file) → third-party code with
  no package-manager audit.
- **Registry "sanity"**: for npm we check **weekly downloads** (catches a live
  typosquat: `lodahs` ~125 vs `lodash` ~162,000,000); for **Rust, Python,
  Ruby, PHP, .NET, Dart, Elixir, Go and Java/Maven** we check the package
  **exists** on the official registry (crates.io, PyPI, RubyGems, Packagist,
  NuGet, pub.dev, Hex, `proxy.golang.org`, Maven Central).

Each risk requires **individual** confirmation — **"yes to all" and `--yes`
do NOT apply here**. If any risk is refused (or `--yes` is used), the
`prepare`/`docker` **aborts without installing anything**.

> **Command injection:** already impossible by construction — commands run
> without a shell (`Command::new(prog).args([...])`), so a package name with
> `; rm -rf /` becomes a literal argument. Docker image names are sanitized
> to `[a-z0-9._-]`.
>
> **Honest limitations:** the download check is a signal, not proof — a
> high-traffic typosquat or a legitimately tiny package may slip through. For
> dependency `postinstall`s, we look at the version `npm install` would pull
> (exact when the requirement is pinned; `latest` when it's a range), so a
> script that only exists in another version can be missed — and none of this
> replaces reading the code. It's an extra safety net, not a guarantee.

### What it prepares

- **`prepare`** (no Docker): runs each detected stack's installer
  (`cargo build`, `npm install`, `pip install` into `.venv`,
  `go mod download`, `composer install`, `bundle install`, `dotnet restore`,
  `mix deps.get`, `cmake`/`make`, ...) and shows the command to run next.
- **`docker`**: generates a `Dockerfile` per stack (multi-stage when it makes
  sense), runs `docker build` and shows the `docker run`. Dockerfiles come
  with a comment warning that `CMD`/variables may need adjustment — they are a
  **starting point**, not a guarantee that every project runs unmodified.

### Environment fingerprint (`env`)

One read-only call that replaces an agent's ~10 probes for *"does this machine
even work?"*:

```bash
zavik env --json
```

```json
{"schema":1,"command":"env","os":"linux","arch":"x86_64",
 "stacks":[{"language":"JavaScript","tools":[
   {"name":"node","present":true,"version":"v22.10.0"},
   {"name":"npm","present":true,"version":"10.9.0"}]}],
 "missing":[]}
```

- **Presence** = PATH lookup; **version** = `<tool> --version` of tools
  already installed (argv, **no shell**). If the probe fails,
  `"version": null` — it never invents a version.
- **Read-only**: it installs nothing and **never reads `.env` files** (your
  secrets are not its business).
- Human mode shows the same as sections (OS + tools per stack + install
  suggestions for what's missing) — the JSON shape is the agent interface.

### Missing toolchain (node/npm/go/...)

Before running `prepare` (no Docker), it checks the stack's **tools** are on
the PATH (`cargo`, `node`+`npm`, `go`, `python3`, `php`+`composer`, ...) and,
if something is missing, **warns immediately** with the install suggestion for
**your OS** (apt/dnf/pacman on Linux, `brew` on macOS, `winget` on Windows):

```
MISSING TOOLS
  • ruby (Ruby)
      suggestion: sudo apt install ruby
```

- **Offers to install (Phase 2, with permission):** for a **curated set** of
  tools (node/npm/go/python3/java/php/ruby), it **asks** whether to run the
  install command. Each one is a **critical** confirmation — **neither "yes to
  all" nor `--yes` cover it**. Runs **without a shell** (argv by argv) with
  **inherited stdio**, so a password-prompting `sudo`/`brew` works. It then
  **re-verifies** the PATH and reports honestly (it may refuse or fail). Other
  tools (e.g. Rust, which is `curl | sh`) stay **suggestion-only** — it won't
  guess commands/versions.
- In **Docker** mode no check runs: the toolchain lives inside the container.
- Suggestions are a **reasonable guess** per OS, not a guarantee — every
  machine can differ, and the package manager may not even exist.

> ⚠️ Installing dependencies executes commands on your machine (and
> `npm install` runs the project's scripts). Use `--dry-run` to inspect first.

## Architecture

Zavik follows **Clean Architecture** with the **Microkernel** pattern for
modularity, testability and extensibility. The critical components:

- **EventBus:** reactive, decoupled communication between modules.
- **Plugin SDK:** contracts (`Capabilities`) that isolate plugins.
- **Workflow Engine:** transactional, staged execution
  (`Workflow -> Step -> Action`).
- **Kernel:** the orchestrator managing the plugin lifecycle.

### Workspace layout (`crates/`)

Dependencies flow from outside in (see `arquiteture.md`):

```text
shared  ->  events  ->  sdk  ->  { core, kernel } / plugins
```

| Crate              | Role                                                             | Depends on                     |
| ------------------ | ---------------------------------------------------------------- | ------------------------------ |
| `zavik-shared`  | Domain: universal types (`Context`, `ProjectStack`, `Language`).  | —                              |
| `zavik-events`  | Event bus (`EventBus`, `EventEnvelope`).                          | `shared`                       |
| `zavik-sdk`     | Contracts: `Capability`, `FileSystemAccess`, `ProjectDetector`, `CommandRunner` and the `PermissionGate` port. | `shared`, `events` |
| `zavik-plugins` | V1 plugins: `LocalFileSystem`, `StackDetector`, `LocalCommandRunner`. | `shared`, `sdk`             |
| `zavik-core`    | Use cases: `WorkflowEngine` and `Provisioner` (project preparation). | `shared`, `sdk`                |
| `zavik-kernel`  | Microkernel: plugin registry and lifecycle.                       | `shared`, `events`, `sdk`      |
| `zavik` (CLI)   | Entry point + interactive `PermissionGate`.                        | all                            |

## Stack detection

The `StackDetector` plugin recognizes a project's stack by reading the root
manifests. One project can have **several** stacks (e.g. a Tauri app has both
Rust and Node).

With the **`--all`** flag it also **descends into subfolders** looking for
manifests — useful for **monorepos** or nested manifests (e.g. `.csproj` under
`Src/Core/`, `package.json` under `pkgs/api/`). The scan skips
dependency/build dirs (`node_modules`, `target`, ...) with depth and count
limits. Without `--all`, only the root is read (predictable behavior).

| Language         | Manifest                     | Recognized frameworks (examples)               |
| ---------------- | ---------------------------- | ---------------------------------------------- |
| Rust             | `Cargo.toml`                 | Tauri, Actix Web, Axum, Rocket, Bevy, Yew...   |
| JavaScript/TS    | `package.json`               | React, Next.js, Vue, Angular, Express, NestJS, Electron, Vite... |
| Python           | `pyproject.toml`, `requirements.txt`, `Pipfile`, `setup.py` | Django, Flask, FastAPI, Celery, Pytest... |
| Go               | `go.mod`                     | Gin, Echo, Fiber, GORM, Chi, Cobra             |
| Java / Kotlin    | `pom.xml`, `build.gradle[.kts]` | Spring, Quarkus, Micronaut, Android         |
| PHP              | `composer.json`              | Laravel, Symfony, CakePHP, WordPress           |
| Ruby             | `Gemfile`                    | Ruby on Rails, Sinatra, Hanami, Jekyll         |
| C# / .NET        | `*.csproj`                   | ASP.NET Core, Entity Framework, MAUI           |
| Dart / Flutter   | `pubspec.yaml`               | Flutter                                         |
| Elixir           | `mix.exs`                    | Phoenix, Ecto, LiveView                        |
| Swift            | `Package.swift`              | Vapor, Hummingbird                             |
| C / C++          | `CMakeLists.txt`, `Makefile` | —                                              |

The full keyword table lives in `crates/plugins/src/detector/`.

## Status & honesty

- Everything described above is implemented and under test (200 passing,
  clippy clean). Pre-launch roadmap (installer, brew/npx/winget):
  [ROADMAP.md](ROADMAP.md).
- **Language**: human-facing CLI messages ship in **pt-BR + English** with
  **English as the default** (pre-launch i18n, `--lang pt-BR` to switch); the
  machine contract (JSON, SARIF, exit codes) is **always English**.
- Honest limitations are called out inline throughout this README — nothing
  claims more than the tool does. Automatic fix execution is **out of scope**
  by design: Zavik only **proposes** fixes.
- License: [MIT](LICENSE).
