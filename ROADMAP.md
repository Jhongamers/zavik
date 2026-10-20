# Roadmap — upcoming tasks (Zavik)

> Annotated for **future** implementation. None of this has been done yet.
> Goal: turn Zavik into a good product — a tool an AI orchestrates while
> spending few tokens, with real security.

[English](ROADMAP.md) · [Português (Brasil)](ROADMAP.pt-BR.md)

---

## 1. Security / supply-chain (closing gaps)

### 1.1 Install-time execution for ALL languages (today it's npm only)
The `postinstall` check today covers **only JS/TS**. Every ecosystem has an
equivalent and it is **not checked**:

- [ ] **Python**: `setup.py` / build hooks (`pyproject.toml` `[build-system]`)
      run on `pip install .`.
- [ ] **Rust**: `build.rs` + hooks/alias in `.cargo/config.toml` run on
      `cargo build`.
- [ ] **PHP**: `composer install` runs scripts (`pre/post-install-cmd`).
- [ ] **JVM (Maven/Gradle)**: build plugins execute code at build time.
- [ ] **.NET**: NuGet scripts / MSBuild targets.
- [ ] **Ruby**: native extension compilation on `bundle install`.
- [ ] **Elixir/Mix**: compile hooks.

### 1.2 `postinstall` of DEPENDENCIES (today only the root project)
The most dangerous vector (e.g. `event-stream`). Today we only read the root
`package.json`.
- [ ] Option A: install with `--ignore-scripts`, inspect `node_modules`, and
      only then allow the scripts.
- [ ] Option B: download tarballs only for suspicious packages and read the
      `package.json` inside (heavy; only worth it for suspects).

### 1.3 "Exists in the registry" coverage
The "lookup" now covers **all** supported ecosystems:
- [x] npm (weekly downloads anti-typosquat), crates.io, PyPI.
- [x] RubyGems, Packagist (PHP), NuGet (.NET), pub.dev (Dart), Hex (Elixir).
- [x] **Go** via `proxy.golang.org/@latest` (escapes uppercase → `!lowercase`).
- [x] **Maven Central** via `repo1.maven.org/maven2/{group}/{artifact}/`
      (Maven/Gradle detectors now keep the `groupId:artifactId` coordinate to
      allow the check).

### 1.4 Stronger anti-typosquat signals
- [ ] Popularity/downloads is a **signal, not proof**. To evaluate:
      account age, number of versions, name similarity with a famous package.

### 1.6 Missing toolchain (node/npm/go/...) — detect and suggest
The user should not discover "node not found" only when `npm install` fails.
Each OS has its own way of installing and not everything can be predicted
(sudo, distro, conflicts), so this is **phased and conservative**:

- [x] **Phase 1 (done): detect + suggest** — before `prepare` (local), checks
      whether the stack's tools are on the PATH (`which`/`where`) and shows
      the install suggestion **for the current OS family** (apt/dnf/pacman,
      brew, winget). Read-only, installs nothing.
- [x] **Phase 2 (done): ask and run the installation** — with a conservative
      design: only a **curated set** of tools (node/npm/go/python3/java/php/
      ruby) has an automatic command; the others (rust via `curl|sh`,
      dotnet/java with variants) stay suggestion-only. Each installation
      requires a **critical** confirmation at the `PermissionGate` (neither
      "yes to all" nor `--yes` cover it). Runs **without a shell** (argv) with
      **inherited stdio**, so password-prompting `sudo`/`brew` work; if the
      package manager doesn't exist or the tool isn't curated, it doesn't
      offer it. Afterwards it **re-verifies** the PATH and reports honestly
      (it can fail / be refused).
- [ ] **Phase 3: fix/suggest on error** — when a command fails, read the cause
      and suggest the next step (e.g. "missing lib X", "conflicting version").
      The least predictable; leave it for last.
- Honest limitation: the suggestions are a reasonable guess per OS, not a
  guarantee. They don't replace reading the error on your machine.

---

## 2. Token economy / becoming an AI orchestration product

**Positioning:** the LLM is the **brain** (decides *what*), the CLI is the
**hands** (knows *how*, with permission + verification). Everything the CLI
does is **local deterministic Rust = zero AI tokens**. The missing piece is
**structured** output.

### 2.1 `--json` on ALL commands (highest priority)
- [x] `detect --json`, `analyze --json`, `prepare --json`, `docker --json`.
- [x] Compact output (single line) on stdout; diagnostic logs go to stderr.
- [x] Stable schema with `"schema": 1`.
- [x] `prepare`/`docker --json` deliver **plan + trust check** and **do not
      execute** (execution is interactive).

### 2.2 `detect --json` as "ground truth"
- [x] Done together with 2.1. Replaces the AI reading N manifest files.

### 2.3 `prepare --dry-run --json`
- [x] Covered by `prepare --json` (structured plan + trust, without executing).
- [ ] (Optional) also expose `--dry-run` explicitly, mirroring `--json`.

### 2.4 Error summary / next step — `zavik diagnose` ✅
- [x] A `zavik diagnose` that takes a failed output and returns a **short
      diagnosis**: the **real** likely cause (not the symptom), a suggested
      fix, and the lines that matched (the "proof"). Saves reading a giant
      stack trace — and solves the case where the literal error points to the
      wrong place.
- [x] Input via `--message "..."`, `--input <file>` or **pipe**
      (`my_command 2>&1 | zavik diagnose`). Only reads stdin when it is not a
      terminal.
- [x] ~16 known-pattern rules (language-agnostic), local and deterministic
      Rust, zero tokens. The extra dynamic part is extracted from the error
      itself: `-lssl` → `libssl-dev`, `No module named 'requests'` →
      `pip install requests`, `cargo: not found` → toolchain. Misleading
      symptom rules (link, node-gyp) are marked with a `note` explaining it
      is only a consequence.
- [x] Honest fallback: if no rule matches, it returns the most error-looking
      lines (a summary) instead of pretending it understood.
- [x] `--json` (schema 1, pure stdout).

> Decision: **`diagnose` on text**, not `zavik run <command>`. That way Zavik
> doesn't gain a new arbitrary-execution surface (consistent with the security
> posture); the AI/user already has the error and only wants the translation.

### 2.5 Deterministic cache
- [ ] Repeated calls don't re-detect (detection is pure and cheap).

### 2.6 One-call briefing (`context`) — save exploration
- [x] **Done.** `zavik context [--json]` joins `detect` + `trust` + a
      **runbook** (setup/build/test/run/lint, from `scripts` in
      `package.json`, `Makefile`/`justfile` targets, `mvnw`/`gradlew`
      wrappers, per-language conventions) + **entrypoints** + **key files**
      (manifest, lockfile, CI, Dockerfile, `.env`, README). Replaces N
      AI `glob`/`grep`/reads with **1 JSON**. Pure read, executes nothing.
- Honest limitation: the runbook is **inferred** (most likely command, not a
  guarantee); CI is not parsed from YAML (only listed).

---

## 1.5 Detection — improvements found testing on REAL projects
Tested on 12 real projects (anyhow, cobra, itsdangerous, cobra, junit4,
polyfill-mbstring, sinatra, Newtonsoft.Json, dart-lang/args, phoenix,
swift-argument-parser, googletest). Findings:
- [x] **Bug fixed**: a lone Makefile used to become a false C/C++ detection
      (e.g. `cobra`/Go). Now it requires real C/C++ source or CMakeLists.txt.
- [x] **Nested/monorepo project discovery** (`--all`): optional recursive
      scan that finds manifests in subfolders (`.csproj` under `Src/...`,
      `package.json` under `pkgs/...`). Skips dependency/build dirs
      (`node_modules`, `target`, ...) with depth/quantity limits. Default
      still looks only at the root (stable behavior).
- [ ] Framework coverage: in the real tests only Phoenix reported a framework;
      expand/validate framework matching against real-world manifests.
- [ ] **Per-ecosystem validation corpus** (design `docs/design-corpus.md`,
      approved 17/10) — turn testing-on-real-projects (which found the
      Makefile bug and the 4 from dogfooding) into a permanent practice:
      real projects **pinned by SHA**, partial clone with cache, **no
      vendoring** of third-party code (repo goes public on PH), corpus
      **read-only only** (`run`/`prepare`/`up` **never** run other people's
      code — permission doctrine), anti-circular ground truth (lockfile =
      version truth + curated per-project metadata), `#[ignore]` tests outside
      the default `cargo test` (stays offline). Phases: A Rust → B npm/Python/
      Go → C diagnose with real stderr → D non-Rust monorepo (decide to
      implement or document). Seed: the 12 from §1.5.

---

## 3. Product / monetization (the wedge: supply-chain audit)

### 3.1 `zavik audit` — supply-chain audit ✅
- [x] **Done.** `zavik audit [--online] [--fail-on sev] [--policy] [--ignore]
      [--json]`. Patterns from the established tools: **Socket.dev** (risk
      factors + score confluence), **Snyk/cargo-audit** (policy + `fail_on` +
      exit code), **OWASP** (FP suppression with reason + expiry).
- [x] **Pure engine** in `plugins/src/audit.rs`: benign-vs-suspicious hook
      classifier (`node-gyp rebuild` OK; `curl|sh`/base64/credential =
      critical) + score by **confluence** (no single signal is a verdict;
      threshold 25 avoids flagging a normal range or a benign hook). 8 offline
      tests (incl. real esbuild NOT flagged, `curl|sh` → critical, allowlist,
      active/expired suppression).
- [x] **Security**: suppression **requires an expiry** (ignore never becomes an
      eternal shield); policy/suppression in versioned TOML; `--fail-on` gives
      exit 1 to gate CI; nothing is executed (only manifest/registry reads).
- [x] Online npm enrichment (hook + downloads) via the `npm_audit_signals`
      bridge; offline gets origin (registry/git/path) + pin.
- Honest limitations: it analyzes the **text** of the script, not the files it
  invokes; name-similarity typosquatting and lockfile integrity are not yet
  implemented (see 1.4).
- [x] **SARIF 2.1.0** (`--sarif`): findings become `results[]` with `ruleId`
      (factor), `level` (sev→note/warning/error) and `location` = manifest +
      **exact line** (lazy lookup, no regex/dependency). Read by GitHub Code
      Scanning → annotates the PR inline. Validated (line 4 of package.json).
### 3.2 GitHub Action + packaging ✅ (templates ready; GH validation pending)
- [x] **`action.yml`** (composite): downloads the binary from the Release →
      verifies `sha256` (optional input anchors the hash in the action) →
      runs `audit --sarif --fail-on` → publishes SARIF **before** the gate
      (the annotation shows even when the PR fails) → final exit code.
      Inputs via `env:` (immune to script injection); ref pinned by tag.
- [x] **`.github/workflows/release.yml`**: binaries per `v*` tag (linux
      x64/arm64, macOS arm64, windows) + `.sha256` as an asset.
- [x] **README**: consumer example workflow + honest limitations (inline
      SARIF requires Code Scanning; exit-code gating always works; Windows is
      manual download only).
- [x] **`v0.1.0` published** (19/10) ✅: annotated tag at `936c4fb` →
  `release.yml` green on all 4 targets, 8 assets (binaries + `.sha256`).
  Local E2E with the binary **downloaded from the release**: sha256 OK,
  `--version` 0.1.0, batch-E fix and `DEFAULT_DATASET_URL` baked into the
  artifact.
- [x] **`v0.1.1` published** (19/10) ✅: tag at `d440325` (3 dogfooding
  fixes: git-dep trust #13, member lockfile #14, SIGPIPE #15) → green
  release, 6 assets. Local E2E of the binary **downloaded from the release**:
  sha256 OK, and the 3 fixes proven on the artifact (git-dep → "outside the
  registry"; workspace-member → "Nothing to report"; `| head` → exit 141, no
  panic). **Honest version inconsistency:** the binary identified itself as
  `0.1.0` (the workspace `CARGO_PKG_VERSION` was never bumped); fixing it
  required a bump + re-tag (`v0.1.2`) — founder decision, recorded
  (**resolved in `v0.1.2`, below**).
- [x] **`v0.1.2` published** (19/10) ✅ — **version↔tag coercion
  resolved**: workspace `version` bump (`0.1.0` → `0.1.2`, founder decision;
  `0.1.1` skipped in the source semver, tag already published) + release
  pins following along (`install.sh` default, `action.yml` input default,
  snippets in both READMEs). Tag `v0.1.2` at `8aeec59` → green release on
  4/4 targets, 8 assets. E2E of the binary **downloaded from the release**:
  sha256 OK, **`--version` = `zavik 0.1.2`** (matches the tag), SIGPIPE exit
  141. Historical entries and test fixtures with their own manifest versions
  stayed intact by design.
- [x] **`v0.1.3` published** (20/10) ✅ — dogfooding fix #20: the
  `repository` input default was the literal placeholder `OWNER/zavik` (404
  for **every** consumer who followed the README without passing the input) →
  now `Jhongamers/zavik`; tag `v0.1.2` carries the bug in its `action.yml`
  snapshot (a pushed tag doesn't move) — hence the re-tag. **New invariant in
  `cargo test`:** the `version` default == `v` + `CARGO_PKG_VERSION`
  (tag↔binary coercion is now automatic, not a manual check) + the
  `repository` default in `owner/name` format with no placeholder — both with
  non-vacuity by mutation. Tag `v0.1.3` at `6ad7336` → release 4/4. E2E:
  sha256 OK, binary `zavik 0.1.3`, anonymous install.sh with the new default
  OK and **`action-smoke` green** — the action consumed a real release
  (anonymous curl + sha256 + audit + gate), closing door F2 with real E2E.
- [x] **Finding (19/10): `action.yml` uses pure anonymous `curl`** (it does
  not inject `GITHUB_TOKEN`). With the repo **private** the install step
  fails **404** (anonymous download of a private asset); proven that
  **authenticated** works (`gh release download` → file). **Resolved by
  opening the repo (20/10):** public repo → the anonymous path runs, real E2E
  green via `action-smoke` (public asset + sha256 + audit + gate).
  *Pending founder decision (not coded), now restricted to **private**
  consumers:* injecting `GITHUB_TOKEN` into the curl would make the action
  work in a private repo, but it couples the action to GitHub and changes the
  integrity model — recorded, awaiting go.
- Next (future): name-similarity typosquatting, lockfile integrity, online
  advisory DB (GHSA).

### 3.3 Monetization — offline license foundation (phase 1, $0 cost) ✅
- [x] **Model decided**: open core + subscription. Free CLI (adoption +
      trust wedge); server-side gate only when phase-1 revenue funds the
      backend (phase 2). Fresh data = the moat (code the AI rewrites).
- [x] **`zavik license status|install|remove`** (+ `--json`): `ZV1` token =
      payload (plan/holder/issued/expires) signed **Ed25519**; the binary
      carries only the public key; verification 100% local (offline-first
      kept). 5 engine tests + manual E2E (valid/tampered/expired/wrong-key/
      garbage — all correctly rejected).
- [x] **Minting tool** `examples/license_tool.rs` (`genkey`/`mint`) — not in
      the binary; the private key stays with the vendor, **never** in the repo.
- Honest limitations: an anti-honesty gate (a patch bypasses it), it doesn't
  defend against a backward clock; today it blocks **only** `data update` —
  the rest of the CLI, including `context`, stays 100% free.
- [x] **Risk dataset phase 1 — `zavik data status|update`** ✅
      (design + amendments: `docs/design-data.md`; staging 1–3, 5, 6 done):
      Ed25519 envelope with a **separate key** from the license (private key
      in `~/.zavik-dataset.key` + `ZV_DATASET_KEY` secret — never in the
      repo); `status` free/offline (absent / invalid / age / stale >14d);
      `update` license gate **before the network** (`no_license`/
      `license_invalid`, exit 1), `https`/`file://` download with a 32 MB
      cap, verification → atomic write (a refusal leaves the previous one
      intact); `audit` gains a `dataset_findings` section — **exact** kill
      list → `high`, advisory by **name only** → `low` (no fuzzy, no
      range-matching) — which does **not** touch `score`/`passed`/`--fail-on`;
      without the dataset, the audit JSON is **byte identical** (tested
      invariant). 24 new tests (15 unit + 9 integration; total 123) + real E2E
      with the production keys: update → status → audit with the section →
      tampered dataset refused with the previous one remaining.
- [x] **Dataset phase 1 complete (staging 4)** ✅: `license_tool
      build-dataset` builder (GHSA **reviewed** only, 90d window; kill list
      from 4 title patterns measured across the 389,683 real files → 61
      names; excludes version-scoped titles = anti-FP) +
      `.github/workflows/dataset.yml` workflow (daily cron + dispatch, sparse
      clone, build → sign with `ZV_DATASET_KEY` → **verify against the
      binary's key before publishing** → `dataset-latest` asset via `gh`, no
      third-party action). 11 tests + real E2E (36,885 GHSAs → 750 KB → audit
      with a real kill list). Activation executed: repo `Jhongamers/zavik`,
      `ZV_DATASET_KEY` secret, `DEFAULT_DATASET_URL` filled in; workflow
      dispatch + local E2E green (license `ZV1` → `data update file://` →
      audit with the FRESH DATA section). Direct download from the embedded
      URL resolves with a public repo (private asset today → honest 404). Tag
      `v0.1.0` published (19/10) with a binaries release on the 4 targets.
      Optional upgrade (founder): `ossf/malicious-packages` OSV corpus (238k
      names ≈ 28 MB per asset vs ~1 MB today) — see design-data §2.
- [ ] Action on the GitHub Marketplace (their billing, the client's compute)
      — after publishing the repo + tags.
- [ ] Server-side gate (GitHub App, dashboard, audit trail) — only with
      phase-1 revenue paying for the infra.

## 4. Launch & monetization (detailed plan: `docs/plano-lancamento.md`)
- Model: open core + subscription (free: the whole CLI; paid: fresh dataset +
  service). Phases, billing flow (MoR → key via expiry) and the Product Hunt
  kit are documented in the plan.
- [x] **UX principle (decided)**: isolated steps AND composite (`up`) — see
      `docs/plano-lancamento.md` 5.0 (WorkflowEngine + per-step gates).
- [x] **`up` v1 implemented** (design + amendments: `docs/design-up.md` §12):
      pipeline `[clone?] → audit → prepare (trust embedded) → run` reusing
      the whole isolated commands (same code, same gates); `run_prepare` →
      `PrepareOutcome::{Completed,Aborted}` — denied trust/toolchain/aborted
      install interrupts BEFORE `run` (never executes code after an abort);
      `--until/--skip/--dry-run`; safe clone (`--depth 1`, no submodules,
      `-c core.hooksPath=/dev/null`, never overwrites the destination;
      `file://` or ending in `.git`; `--skip clone` is always an error); JSONL
      on `--json` (real audit, prepare=plan, run denies without a TTY — AI
      mode doesn't execute risk). 17 new tests (10 unit + 7 integration), real
      E2E with a local git repo: clone + full pipeline exited 0; denied trust
      interrupted before `run`; existing destination → honest error + exit 1;
      non-TTY → denies.
- [x] **CLI i18n (decided, implement BEFORE PH)** ✅ **DONE 18/10
      (phases 1–3)** — **PHASE 1 ✅ (17/10**, design
      `docs/design-i18n.md`): infra in shared (`Lang`, `t`/`t_fmt` — pt-BR is
      the key, `EN` is the catalog, honest fallback = identity), resolution
      `--lang` > `ZAVIK_LANG` > locale > default; `--lang` extracted
      **pre-dispatch** (applies to `license`/`data`), respects value flags and
      stops at `--`; migrated in phase 1: `section()` (17 headers), banner,
      `env` human output, permission-gate prompts, `up` step titles; +14 tests
      (catalog parity and placeholders, resolution order, integration: flag/
      env/default/invalid/independent-JSON/post-`--`). **PHASE 2 IN PROGRESS
      (17/10)**: `audit` body (incl. anti-false-positive "NO DATA") + full
      `help` migrated (consts `HELP_PT`/`HELP_EN` — the key is the emitted
      text, no drift), registered decision of JSON prose forced to EN via
      `t_en` (dogfooding finding, `docs/dogfooding.md`); ✅ `diagnose`
      (17/10, incl. plugins), ✅ `run` (17/10) and ✅ `data`/`license` + SARIF
      to EN (17/10, `message_in(lang)` for compound errors); `up` also
      migrated (body/errors/clone construction-lang for JSON), `analyze`,
      `print_stacks`/`print_execution`, `context` (envelopes) and the
      `prepare` body (trust/toolchain/execution); remaining dispatch/parse
      migrated (logger, plugins, policy, parse) — residual sweep in main.rs:
      ZERO. **Content reauthoring ✅ (18/10)**: `evidence` +
      `finding/risk message` rewritten in EN (content class — never translated
      at runtime; a test locks the split shell-`--lang`/EN-content).
      **PHASE 3 ✅ (18/10)**: final sweep (`KERNEL:`/`EVENTBUS:` from the
      kernel/events crates + the shared `Display` error prefixes) + **default
      flip to `en`** + updated tests (default pinned as EN, the pt side
      covered by explicit `--lang pt-BR`, test helpers remove locale — a BR
      machine doesn't become a flake) + README. Hand-rolled catalog (`pt-BR` +
      `en`, zero dependency — keeps offline-first), automatic language by
      locale (`LC_ALL`/`LANG`) with `--lang pt-BR|en` override, **default
      `en`** in force since the flip (the default was `pt-BR` during the
      migration, to never emit mixed text). Rules: **JSON/SARIF/exit codes
      never translate** (contract locked in en); identifiers, comments and
      rustdoc **in English from the 1st commit** (migration done before
      versioning). Cost: extracting ~100+ scattered messages from `println!` +
      a continuous 2-entry tax per new message. Rationale for the en default:
      technical errors in English are searchable; PH/issues are in English.
- [x] **README in English as primary** ✅ (16/10): `README.md` translated (EN)
      + a pt-BR copy (`README.pt-BR.md`, synced at **milestones**, not every
      commit — the honest cost of maintaining two texts). **Agent-agnostic**
      positioning: the "Works with any agent" section (Claude Code, Codex,
      OpenCode, Cursor, Antigravity, aider, Copilot) as **compatibility, not
      affiliation** — the old title said "OpenCode Agent" (fixed; association
      with a single agent would limit the market and mislead).
      Honest note on the README: human messages follow pt-BR until the i18n.
- [ ] **Launch on Product Hunt** (open repo + LICENSE + GIF at the top + tag
      v0.1.0; Tue–Thu US morning; first comment with honest limitations).
- [x] **Pre-check of registry names** (19/10, brought forward) ✅: `zavik` is
      **free** on npm **and** crates.io, and so are the fallbacks `zavik-cli`
      (npm + crates) and `@jhongamers/zavik` (npm) — all 404 "does not
      exist". Reliable signal from this machine (control `express`/`serde` →
      200, so the network path is not the phantom-404 already observed in the
      browser). **Still re-confirm in the browser on publish day** —
      registration is first-come-first-served and can change between now and
      launch.
- [x] **GitHub org `zavik` — unavailable; decision: no org** (19/10) ✅
      recorded: the **authenticated** org-creation form says "The name 'zavik'
      is already taken" even though `github.com/zavik` → 404 (reliable 200/404
      controls, same protocol as the npm/crates checks) — reserved-name
      pattern / deleted-org cooldown. **Consequence:** the root URL
      `zavik.github.io` doesn't exist without the exact org; a variant org
      (`zavikhq` etc.) wouldn't pay for itself (repo transfer + sweep of
      baked URLs — `install.sh` `REPO`, `dataset.rs` `DEFAULT_DATASET_URL` in
      the binary, READMEs — + re-E2E) for a gain equivalent to the current
      URL. **Founder decision:** the repo stays `Jhongamers/zavik`; the site
      stays at `jhongamers.github.io/zavik` (`gh-pages` branch, Lote C),
      activated on the day the repo opened. A future transfer remains possible
      at any time (redirect preserves the old URL; the sweep playbook already
      exists — Lote B); **recheck `zavik` in the browser on publish day** — if
      it frees up, reevaluate cost-benefit with the facts on the table.
- [ ] **Publish to npm + crates.io on the day** (new account, name `zavik`;
      if taken by then → fallback `zavik-cli` / `@jhongamers/zavik`).
      `cargo publish -p zavik` and `npm publish` pointing at the most recent
      Release binary.
- [ ] **Update-notice channel for installed binaries (MoR sync batch) — $0**
      (registered 20/10): unit = **release/tag, not commit**. (a) Version
      notice = `GET releases/latest` (GitHub free: 60 req/h per IP), at most
      1×/week cached in `XDG_CONFIG_HOME`, interactive TTY only, on **stderr**
      (stdout/`--json` stays byte-identical), silent when `CI=true`/offline,
      opt-out `ZAVIK_NO_UPDATE_CHECK=1`, message only PROPOSES the re-install
      command — never downloads/executes (download + `sha256` stays in
      `install.sh`; auto-update would violate "product only proposes" and add
      attack surface). (b) Paid-plan notice = honest one-liner in `data
      status` + the `audit` "NO DATA" section pointing at `zavik.dev` (no
      price in binary or repo). (c) Free email channel = GitHub Watch →
      Releases (built-in). Code only after PH, together with site pricing +
      README + E2E purchase→mint.
- [ ] **Distribution matrix (F0–F3) — 3 doors: installer, prompt (manager),
      AI/Action**. Recommended by the marked audience ✅; the Chocolatey page
      was read (standard convention `choco install zavik` — no extra
      inspiration needed):
  - **F0 (founder — blocks everything)** ✅ **DONE** (19/10): annotated tag
    `v0.1.0` at `936c4fb` → `release.yml` published binaries (linux
    x64/arm64, macOS arm, windows) + `.sha256` — 4/4 green targets, checksum
    verified on the binary downloaded from the release.
  - **F1**: `install.sh` ✅ **recommended (end user, incl. WSL and macOS)**
    — **DONE** (17/10): detects OS/arch, downloads from the Release, checks
    sha256 (no skip flag — tampered is refused, with a test), non-interactive;
    overrides `ZAVIK_VERSION`/`ZAVIK_INSTALL_DIR`/`ZAVIK_BASE_URL`; 4 tests
    (POSIX syntax, E2E via `file://` installing a working binary, divergent
    checksum refusal, unknown OS with a clear message). The 2-line manual
    version documented next to it (we ate our own pastel: our own `audit`
    classifies `curl|sh` as a risk) + the README "Install" section written for
    humans AND for AI (a canonical copyable line per OS — the AI uses the
    same prompt). Binaries already published (F0 ✅ 19/10); the anonymous URL
    download resolves when the repo opened (private asset used to → 404).
  - **F2**: `cargo install` ✅ **recommended (dev with Rust already
    installed)** + test `action.yml` in the release E2E — the zero-install CI
    path (it's what the AI writes in a workflow).
  - **F3** (as adoption grows): brew tap, `npx` (per-platform binary),
    winget/scoop, **`choco install zavik`** ✅ **recommended (native
    Windows)** — until then the Release `.zip` is the Windows door.
- [x] **5.1 `zavik run` v1 — DONE**: resolves start (declared > convention >
      ask), heuristic readiness (port heuristic + `/proc` with descendants —
      catches the grandchild npm→node), SIGTERM→SIGKILL on the **group**,
      graceful Ctrl-C, early crash → `diagnose`, critical gate (a human
      decides), `--dry-run`/`--timeout`/`--health`/`--`/`--json`. 23 new tests
      (18 unit + 5 integration). Design: `docs/design-run.md`.
- [x] **5.1.1 `run` v1.1 — `--detach` DONE** (17/10): starts in its **own
      session** (`setsid` — survives the terminal), returns `pid` + `log`
      (`.zavik/run.log`) + `stop_command` in the `--json`; no readiness
      supervision (documented honest limitation); same critical gate (not even
      `--yes` covers it — a test locks the invariant); on Windows it stays
      tied to the console (known limitation). Tests: 3 unit (own session
      proven via `getsid`, log captured, failure becomes an error) + 2
      integration (denied without a human even with `--yes`, dry-run doesn't
      execute). Remaining in v1.1: https health.
- [ ] **5.2 integrated `git clone`** — **absorbed by `up <url>`** (the
      pipeline's clone step replaces an isolated `from` command; see
      `docs/design-up.md`). Safe clone: no hooks (`core.hooksPath=/dev/null`),
      `--depth 1`, no submodules, via CommandRunner (no shell) + trust check
      before any execution.
- [ ] **5.3 `zavik why <package>`** (analyzed, awaiting design/approval):
      reverse dependency tree + origin + the `audit` risk factors in a single
      JSON. Real demand exists (`npm why`, `cargo tree -i`,
      `why-is-installed`); humans debug by hand, AI replaces 5 commands with 1
      call. Read-only, zero execution, reinforces the supply-chain wedge.
- [x] **5.4 `zavik env --json`** ✅ (16/10): environment fingerprint in one
      call — `os`/`arch` + toolchains per detected stack with presence (PATH)
      and **real version** (`<tool> --version`, argv without shell; failure →
      `version: null`, never invents) + a `missing` list. Kills "works on my
      machine", it's 1 AI call instead of probing 10 commands, and feeds
      `diagnose` with environment evidence. Read-only, no gate (installs
      nothing, doesn't read `.env`). Human: per-stack sections + install
      suggestions for what's missing. 4 new tests (presence+version, JS/TS
      dedupe, version never invented, integration --json/empty PATH).

---

## Notes / principles already established (keep)
- **Automatic fixing is OUT** (founder decision, Oct/2026): executing fixes
  can break something outside our control — multi-language × framework =
  unpredictable blast radius. The product only **proposes** (the `diagnose`
  already delivers the "fix"); any future `--fix` would be restricted to
  unambiguous rules behind an individual gate, never automatic execution.
- **Language**: identifiers, tests and comments in **English**; CLI human
  messages in **pt-BR + en** with default **en** since the 18/10 i18n
  (ROADMAP §4); machine contract (JSON/SARIF/exit codes) **always en**.
- Command injection is **impossible by construction**: no shell
  (`Command::new(prog).args([...])`), Docker image sanitized `[a-z0-9._-]`.
- Code-execution risks require **individual** confirmation — "yes to all"
  and `--yes` **never** cover it (protects the AI-driving case).
- Non-terminal (non-interactive) → **denies by default**.
- Existing files are **never** overwritten without `--force`.
- Be honest about limitations: never promise "runs on everything without
  errors".
