//! VENDOR tool — generates the signing key pair and mints
//! licenses. It is an `example`: it is NOT part of the shipped `devforge` binary.
//!
//! Usage:
//!   DF_SIGNING_KEY=<seed> cargo run --example license_tool -- \
//!       mint --plan team --holder "Acme" --expires 2027-01-01
//!
//!   cargo run --example license_tool -- genkey     # generates a new pair
//!
//!   cargo run --example license_tool -- genkey-dataset  # dataset pair (separate key)
//!   DF_DATASET_KEY=<seed> cargo run --example license_tool -- \
//!       sign-dataset --in payload.json --out dataset.json
//!
//! The private key (seed) lives in `DF_SIGNING_KEY`/`DF_DATASET_KEY` and NEVER
//! goes to the repo.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::{Signer, SigningKey, Verifier};
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};

fn b64u(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        // Generates a new pair. The (private) seed comes out here ONCE and goes to
        // the vendor's vault; the public one is embedded in the binary.
        Some("genkey") => {
            let mut rng = rand::rngs::OsRng;
            let signing = SigningKey::generate(&mut rng);
            println!("DF_SIGNING_KEY={}", b64u(&signing.to_bytes()));
            println!("PUBLIC_KEY_B64={}", b64u(signing.verifying_key().as_bytes()));
        }

        // Mints a license from the seed in DF_SIGNING_KEY.
        Some("mint") => {
            let plan = flag(&args, "--plan").unwrap_or("team");
            let holder = flag(&args, "--holder").unwrap_or("unknown");
            let expires = flag(&args, "--expires").unwrap_or("2027-01-01");

            let seed_b64 = env::var("DF_SIGNING_KEY")
                .expect("exporte DF_SIGNING_KEY=<seed b64url> antes de cunhar");
            let seed: [u8; 32] = URL_SAFE_NO_PAD
                .decode(seed_b64.trim())
                .expect("seed em base64url inválido")
                .try_into()
                .expect("seed deve ter 32 bytes");
            let signing = SigningKey::from_bytes(&seed);

            // Payload SEMANTICALLY equal to what the binary expects.
            let payload = serde_json::json!({
                "plan": plan,
                "holder": holder,
                "issued": chrono_today(),
                "expires": expires,
            });
            let payload_bytes = serde_json::to_vec(&payload).expect("payload serializa");
            let signature = signing.sign(&payload_bytes);

            println!(
                "DF1.{}.{}",
                b64u(&payload_bytes),
                b64u(&signature.to_bytes())
            );
        }

        // Generates the DATASET signing pair — deliberately SEPARATE from the
        // license key (a leaked dataset key cannot mint licenses). The seed
        // goes to `~/.devforge-dataset.key` + the GitHub Actions secret
        // `DF_DATASET_KEY`; the public half is pasted into `dataset.rs`.
        Some("genkey-dataset") => {
            let mut rng = rand::rngs::OsRng;
            let signing = SigningKey::generate(&mut rng);
            println!("DF_DATASET_KEY={}", b64u(&signing.to_bytes()));
            println!("DATASET_PUBLIC_KEY_B64={}", b64u(signing.verifying_key().as_bytes()));
        }

        // Signs a dataset payload (`--in <file>` or stdin) into the envelope
        // that ships as the Release asset. CI usage:
        //   DF_DATASET_KEY=<seed> license_tool sign-dataset \
        //       --in payload.json --out dataset.json
        Some("sign-dataset") => {
            let seed_b64 = env::var("DF_DATASET_KEY")
                .expect("exporte DF_DATASET_KEY=<seed b64url> antes de assinar");
            let seed: [u8; 32] = URL_SAFE_NO_PAD
                .decode(seed_b64.trim())
                .expect("seed em base64url inválido")
                .try_into()
                .expect("seed deve ter 32 bytes");
            let signing = SigningKey::from_bytes(&seed);

            let payload: Vec<u8> = match flag(&args, "--in") {
                Some("-") | None => {
                    use std::io::Read;
                    let mut buffer = Vec::new();
                    std::io::stdin()
                        .read_to_end(&mut buffer)
                        .expect("não li o payload do stdin");
                    buffer
                }
                Some(path) => {
                    std::fs::read(path).unwrap_or_else(|e| panic!("não li {path}: {e}"))
                }
            };
            let signature = signing.sign(&payload);
            let envelope = serde_json::json!({
                "schema": 2,
                "payload": b64u(&payload),
                "signature": b64u(&signature.to_bytes()),
            });
            let encoded = serde_json::to_string(&envelope).expect("envelope serializa");
            match flag(&args, "--out") {
                Some(path) => {
                    std::fs::write(path, &encoded).unwrap_or_else(|e| panic!("não escrevi {path}: {e}"));
                    eprintln!("assinado: {path} ({} bytes)", encoded.len());
                }
                None => println!("{encoded}"),
            }
        }

        // Debug/CI: verifies an envelope against a public key (does NOT ship
        // in the binary — the CLI verifies with its embedded key).
        Some("verify-dataset") => {
            use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
            let path = flag(&args, "--in").expect("verify-dataset --in <arquivo>");
            let pk_b64 = flag(&args, "--pubkey")
                .map(str::to_string)
                .or_else(|| env::var("DATASET_PUBLIC_KEY_B64").ok())
                .expect("chave pública via --pubkey <b64u> ou env DATASET_PUBLIC_KEY_B64");
            let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("não li {path}: {e}"));
            let envelope: serde_json::Value =
                serde_json::from_slice(&bytes).expect("envelope é JSON válido");
            assert_eq!(envelope["schema"], 2, "schema do envelope");
            let payload = B64
                .decode(envelope["payload"].as_str().expect("payload b64u"))
                .expect("payload decodifica");
            let signature: [u8; 64] = B64
                .decode(envelope["signature"].as_str().expect("signature b64u"))
                .expect("assinatura decodifica")
                .try_into()
                .expect("assinatura tem 64 bytes");
            let pk: [u8; 32] = B64
                .decode(pk_b64.trim())
                .expect("chave b64u")
                .try_into()
                .expect("chave tem 32 bytes");
            let verifying = ed25519_dalek::VerifyingKey::from_bytes(&pk).expect("chave válida");
            verifying
                .verify(&payload, &ed25519_dalek::Signature::from_bytes(&signature))
                .expect("assinatura VÁLIDA");
            let dataset: serde_json::Value = serde_json::from_slice(&payload).expect("payload é JSON");
            println!(
                "OK: schema={} version={} advisories={} bad_names={}",
                dataset["schema"],
                dataset["version"],
                dataset["advisories"].as_array().map_or(0, |a| a.len()),
                dataset["bad_names"].as_array().map_or(0, |a| a.len()),
            );
        }

        // Builds the dataset PAYLOAD (unsigned) from a github/advisory-database
        // checkout: recent advisories (window) + kill list of malicious names
        // (precise title patterns measured against the full repo, all years).
        // Signing is a SEPARATE step (sign-dataset).
        //   license_tool build-dataset --src <dir com GHSA-*.json> \
        //       --out payload.json [--window-days 90] [--date YYYY-MM-DD] [--bad-list <f>]
        Some("build-dataset") => {
            let src = Path::new(flag(&args, "--src").expect("build-dataset --src <dir com GHSA-*.json>"));
            let out = flag(&args, "--out").expect("build-dataset --out <payload.json>");
            let window_days: i64 = flag(&args, "--window-days")
                .unwrap_or("90")
                .parse()
                .expect("--window-days deve ser um inteiro");
            let reference = flag(&args, "--date").map(str::to_string).unwrap_or_else(chrono_today);
            let bad_list = flag(&args, "--bad-list").map(PathBuf::from);

            let (payload, stats) =
                build_payload(src, window_days, &reference, bad_list.as_deref());
            let encoded = serde_json::to_vec(&payload).expect("payload serializa");
            std::fs::write(out, &encoded).unwrap_or_else(|e| panic!("não escrevi {out}: {e}"));
            eprintln!(
                "payload: {out} ({} bytes)\n  {} arquivos GHSA | {} na janela de {window_days}d | \
                 {} fora/sem data | {} withdraweds | {} afetados sem ecossistema mapeado | \
                 {} títulos → kill list | {} extras do --bad-list | {} parse errado",
                encoded.len(),
                stats.scanned,
                stats.window_hits,
                stats.out_of_window,
                stats.withdrawn,
                stats.ecosystem_skipped,
                stats.kill_list,
                stats.bad_list_extra,
                stats.parse_errors
            );
        }

        _ => {
            eprintln!(
                "uso: license_tool genkey | genkey-dataset | mint --plan <p> --holder <h> --expires <YYYY-MM-DD>\n\
                 \x20    license_tool build-dataset --src <dir> --out <payload> [--window-days N] [--date D] [--bad-list <f>]\n\
                 \x20    license_tool sign-dataset [--in <f>|-] [--out <f>]   (env DF_DATASET_KEY)\n\
                 \x20    license_tool verify-dataset --in <f> [--pubkey <b64u>]"
            );
            std::process::exit(2);
        }
    }
}

/// Today's date in ISO — no date dep (same algorithm as the CLI).
fn chrono_today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil_from_days(secs.div_euclid(86_400))
}

fn civil_from_days(z: i64) -> String {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

// ==========================================================================
// build-dataset — GHSA reviewed → dataset payload (schema 2, UNSIGNED)
// ==========================================================================

#[derive(Default)]
struct BuildStats {
    scanned: usize,
    parse_errors: usize,
    withdrawn: usize,
    window_hits: usize,
    out_of_window: usize,
    ecosystem_skipped: usize,
    kill_list: usize,
    bad_list_extra: usize,
}

/// The subset of a GHSA JSON file the builder needs (OSV schema).
#[derive(serde::Deserialize)]
struct Ghsa {
    id: Option<String>,
    published: Option<String>,
    withdrawn: Option<serde_json::Value>,
    summary: Option<String>,
    affected: Option<Vec<GhsaAffected>>,
}

#[derive(serde::Deserialize)]
struct GhsaAffected {
    package: Option<GhsaPackage>,
}

#[derive(serde::Deserialize)]
struct GhsaPackage {
    name: Option<String>,
    ecosystem: Option<String>,
}

/// GHSA ecosystem → the canonical name the CLI matches against its stack
/// mapping (dataset.rs `ecosystem_of`). Anything else (`SwiftURL`,
/// `GitHub Actions`, unknown) is honestly skipped: no stack maps to it.
fn canonical_ecosystem(raw: &str) -> Option<&'static str> {
    Some(match raw {
        "npm" => "npm",
        "PyPI" => "pypi",
        "crates.io" => "crates.io",
        "Go" => "go",
        "Packagist" => "packagist",
        "RubyGems" => "rubygems",
        "Maven" => "maven",
        "NuGet" => "nuget",
        "Pub" => "pub",
        "Hex" => "hex",
        _ => return None,
    })
}

/// Precise kill-list extraction from a GHSA **title** (summary).
///
/// Grounded in a full-repo measurement (16/10, 389,683 files):
///   1. "`X` was removed from crates.io for malicious code" — package REMOVED
///      (nobody can depend on it legitimately → exact match ≈ true positive)
///   2. "Malicious code in `X` ..." — takedown convention (npm/PyPI)
///   3. "X is malware" — npm takedown titles
///   4. "X typosquatting malware ..." — requires BOTH signals
///
/// Deliberately NOT `/malicious/` bare: that matches "malicious inputs to
/// function" titles of 100% legitimate packages (would blacklist `jsonata`).
fn kill_list_name(summary: &str) -> Option<String> {
    let s = summary.trim();
    let lower = s.to_lowercase();

    if lower.contains(" was removed from ") && lower.contains(" for malicious") {
        return plausible_name(backtick_name(s).or(Some(before_phrase(s, " was removed from "))));
    }
    if let Some(rest) = strip_prefix_ci(s, "malicious code in ") {
        // Version-scoped titles ("Malicious code in guardrails-ai 0.10.1
        // (supply chain compromise)") are about specific VERSIONS — a
        // name-level kill-list entry would FP for patched users. Those fall
        // through to the window-advisory path instead.
        if mentions_version(rest) {
            return None;
        }
        return plausible_name(backtick_name(rest).or(Some(first_token(rest))));
    }
    if let Some(head) = strip_suffix_ci(s, " is malware") {
        return plausible_name(Some(head));
    }
    if lower.contains("typosquat") && lower.contains("malware") {
        return plausible_name(Some(first_token(s)));
    }
    None
}

/// A extracted name is only accepted when it could be a real package name:
/// non-empty, no whitespace, no quotes/backticks, sane length, stripped of
/// title punctuation. Garbage in the kill list = bad data shipped to Pro.
fn plausible_name(candidate: Option<&str>) -> Option<String> {
    let name = candidate?
        .trim()
        .trim_end_matches([':', ',', '.'])
        .trim();
    if name.is_empty()
        || name.len() > 256
        || name.chars().any(|c| c.is_whitespace() || c == '`' || c == '"' || c == '\'')
    {
        return None;
    }
    Some(name.to_string())
}

fn backtick_name(s: &str) -> Option<&str> {
    let start = s.find('`')? + 1;
    let end = start + s[start..].find('`')?;
    s.get(start..end)
}

fn before_phrase<'a>(s: &'a str, phrase: &str) -> &'a str {
    s.split(phrase).next().unwrap_or("")
}

fn first_token(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    // Byte-wise ASCII case-insensitive compare against an ASCII prefix: the
    // matched bytes are provably ASCII, so slicing is UTF-8 safe.
    if s.len() >= prefix.len()
        && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
    {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

fn strip_suffix_ci<'a>(s: &'a str, suffix: &str) -> Option<&'a str> {
    if s.len() >= suffix.len()
        && s.as_bytes()[s.len() - suffix.len()..].eq_ignore_ascii_case(suffix.as_bytes())
    {
        Some(&s[..s.len() - suffix.len()])
    } else {
        None
    }
}

/// True when the text mentions a concrete version (`0.10.1`): digit '.' digit.
/// (`crates.io` does NOT match — no digit before the dot.)
fn mentions_version(text: &str) -> bool {
    text.as_bytes()
        .windows(3)
        .any(|w| w[0].is_ascii_digit() && w[1] == b'.' && w[2].is_ascii_digit())
}

/// `YYYY-MM-DD...` → the date part, when well-formed.
fn iso_date(text: &str) -> Option<&str> {
    let part = text.get(..10)?;
    let bytes = part.as_bytes();
    let pattern_ok = bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            _ => c.is_ascii_digit(),
        });
    pattern_ok.then_some(part)
}

/// `YYYY-MM-DD` → days since epoch (Hinnant) — mirrors dataset.rs.
fn days_from_iso(text: &str) -> Option<i64> {
    let mut parts = text.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// Collects every `GHSA-*.json` under `dir` (recursive, sorted for
/// deterministic runs).
fn collect_ghsa_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_ghsa_files(&path, out);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("GHSA-") && name.ends_with(".json"))
        {
            out.push(path);
        }
    }
    out.sort();
}

/// Builds the unsigned dataset payload from a GHSA checkout.
/// Advisory = within `[reference - window_days, reference]`; kill list =
/// precise title patterns over ALL years (a 2019 takedown is still bad).
fn build_payload(
    src: &Path,
    window_days: i64,
    reference: &str,
    bad_list: Option<&Path>,
) -> (serde_json::Value, BuildStats) {
    let mut stats = BuildStats::default();
    let ref_days = days_from_iso(reference).expect("referência em YYYY-MM-DD válida");

    let mut files = Vec::new();
    collect_ghsa_files(src, &mut files);
    if files.is_empty() {
        panic!(
            "nenhum GHSA-*.json em {} — --src deve apontar pra .../advisories/github-reviewed",
            src.display()
        );
    }

    let mut advisories: Vec<serde_json::Value> = Vec::new();
    let mut seen_advisories: HashSet<(String, String, String)> = HashSet::new();
    let mut bad_names: HashMap<(String, String), serde_json::Value> = HashMap::new();

    for file in files {
        stats.scanned += 1;
        let bytes = match std::fs::read(&file) {
            Ok(bytes) => bytes,
            Err(_) => {
                stats.parse_errors += 1;
                continue;
            }
        };
        let entry: Ghsa = match serde_json::from_slice(&bytes) {
            Ok(entry) => entry,
            Err(_) => {
                stats.parse_errors += 1;
                continue;
            }
        };
        let (Some(id), Some(summary)) = (entry.id, entry.summary) else {
            stats.parse_errors += 1;
            continue;
        };
        // GHSA withdrew the record → nothing enters (advisory AND kill list).
        if entry.withdrawn.is_some() {
            stats.withdrawn += 1;
            continue;
        }

        let url = format!("https://github.com/advisories/{id}");
        let kill_reason: Option<String> = kill_list_name(&summary).map(|_| {
            stats.kill_list += 1;
            // chars() → nunca corta no meio de um codepoint UTF-8.
            summary.trim().chars().take(140).collect()
        });

        let published = entry.published.as_deref().and_then(iso_date);
        let in_window = published
            .and_then(days_from_iso)
            .map(|days| (0..=window_days).contains(&(ref_days - days)))
            .unwrap_or(false);
        let published_date = published.unwrap_or("");

        for affected in entry.affected.iter().flatten() {
            let Some(package) = &affected.package else {
                continue;
            };
            let (Some(raw_name), Some(raw_ecosystem)) = (&package.name, &package.ecosystem)
            else {
                continue;
            };
            let Some(ecosystem) = canonical_ecosystem(raw_ecosystem) else {
                stats.ecosystem_skipped += 1;
                continue;
            };

            if let Some(reason) = &kill_reason {
                // Kill list wins: never double-list the same package as a
                // `low` advisory (the exact hit is the stronger signal).
                bad_names
                    .entry((ecosystem.to_string(), raw_name.to_lowercase()))
                    .or_insert_with(|| {
                        serde_json::json!({
                            "package": raw_name,
                            "ecosystem": ecosystem,
                            "reason": reason,
                            "url": url,
                        })
                    });
            } else if in_window {
                let key = (ecosystem.to_string(), raw_name.to_string(), id.clone());
                if seen_advisories.insert(key) {
                    advisories.push(serde_json::json!({
                        "package": raw_name,
                        "ecosystem": ecosystem,
                        "id": id,
                        "url": url,
                        "published": published_date,
                        "kind": "advisory",
                    }));
                    stats.window_hits += 1;
                }
            } else {
                stats.out_of_window += 1;
            }
        }
    }

    // Curated extra kill-list entries (vendor-maintained, cited sources).
    if let Some(path) = bad_list {
        let text = std::fs::read(path)
            .unwrap_or_else(|e| panic!("não li o --bad-list {}: {e}", path.display()));
        let entries: Vec<serde_json::Value> =
            serde_json::from_slice(&text).expect("--bad-list deve ser um array JSON");
        for entry in entries {
            let name = entry
                .get("package")
                .and_then(|v| v.as_str())
                .expect("bad-list: campo package");
            let ecosystem = entry
                .get("ecosystem")
                .and_then(|v| v.as_str())
                .expect("bad-list: campo ecosystem");
            let key = (ecosystem.to_string(), name.to_lowercase());
            if !bad_names.contains_key(&key) {
                stats.bad_list_extra += 1;
            }
            bad_names.entry(key).or_insert(entry);
        }
    }

    // Deterministic output (same input → same bytes → same signature base).
    advisories.sort_by(|a, b| {
        let key = |v: &serde_json::Value| {
            (
                v["ecosystem"].as_str().unwrap_or("").to_string(),
                v["package"].as_str().unwrap_or("").to_string(),
                v["id"].as_str().unwrap_or("").to_string(),
            )
        };
        key(a).cmp(&key(b))
    });
    let mut bad_sorted: Vec<(String, String, serde_json::Value)> = bad_names
        .into_iter()
        .map(|((eco, name), value)| (eco, name, value))
        .collect();
    bad_sorted.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    let bad_sorted: Vec<serde_json::Value> = bad_sorted.into_iter().map(|(_, _, v)| v).collect();

    let payload = serde_json::json!({
        "schema": 2,
        "version": reference,
        "generated": reference,
        "advisories": advisories,
        "bad_names": bad_sorted,
    });
    (payload, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    fn write_ghsa(
        dir: &Path,
        id: &str,
        published: &str,
        summary: &str,
        ecosystem: &str,
        name: &str,
    ) {
        let sub = dir.join(id);
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join(format!("{id}.json")),
            serde_json::json!({
                "schema_version": "1.4.0",
                "id": id,
                "published": published,
                "summary": summary,
                "affected": [{"package": {"ecosystem": ecosystem, "name": name}}],
            })
            .to_string(),
        )
        .unwrap();
    }

    fn write_ghsa_withdrawn(dir: &Path, id: &str, published: &str, summary: &str) {
        let sub = dir.join(id);
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join(format!("{id}.json")),
            serde_json::json!({
                "schema_version": "1.4.0",
                "id": id,
                "published": published,
                "withdrawn": "2026-10-01T00:00:00Z",
                "summary": summary,
                "affected": [{"package": {"ecosystem": "npm", "name": "evil-pkg"}}],
            })
            .to_string(),
        )
        .unwrap();
    }

    #[test]
    fn in_window_advisory_maps_ecosystem_and_builds_url() {
        let dir = fixture_dir();
        write_ghsa(
            dir.path(),
            "GHSA-test-0001",
            "2026-10-01T10:00:00Z",
            "lodash: prototype pollution",
            "PyPI",
            "Django",
        );
        let (payload, stats) = build_payload(dir.path(), 90, "2026-10-16", None);
        assert_eq!(payload["schema"], 2);
        assert_eq!(payload["generated"], "2026-10-16");
        let advisories = payload["advisories"].as_array().unwrap();
        assert_eq!(advisories.len(), 1);
        assert_eq!(advisories[0]["ecosystem"], "pypi", "PyPI → pypi canônico");
        assert_eq!(advisories[0]["package"], "Django", "nome original preservado");
        assert_eq!(advisories[0]["published"], "2026-10-01");
        assert_eq!(
            advisories[0]["url"],
            "https://github.com/advisories/GHSA-test-0001"
        );
        assert_eq!(stats.window_hits, 1);
        assert_eq!(stats.parse_errors, 0);
    }

    #[test]
    fn window_boundaries_are_exact() {
        let dir = fixture_dir();
        // exactly 90 days before 2026-10-16 → in; 91 → out; future → out.
        write_ghsa(dir.path(), "GHSA-test-0090", "2026-07-18T00:00:00Z", "a b", "npm", "in-window");
        write_ghsa(dir.path(), "GHSA-test-0091", "2026-07-17T00:00:00Z", "a b", "npm", "out-window");
        write_ghsa(dir.path(), "GHSA-test-future", "2026-12-01T00:00:00Z", "a b", "npm", "future-pkg");
        let (payload, stats) = build_payload(dir.path(), 90, "2026-10-16", None);
        let packages: Vec<&str> = payload["advisories"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["package"].as_str().unwrap())
            .collect();
        assert_eq!(packages, vec!["in-window"]);
        assert_eq!(stats.out_of_window, 2, "91 dias e futuro ficam fora");
    }

    #[test]
    fn withdrawn_records_enter_nothing() {
        let dir = fixture_dir();
        write_ghsa_withdrawn(
            dir.path(),
            "GHSA-test-w001",
            "2026-10-01T00:00:00Z",
            "`evil-pkg` was removed from crates.io for malicious code",
        );
        let (payload, stats) = build_payload(dir.path(), 90, "2026-10-16", None);
        assert_eq!(payload["advisories"].as_array().unwrap().len(), 0);
        assert_eq!(payload["bad_names"].as_array().unwrap().len(), 0);
        assert_eq!(stats.withdrawn, 1);
    }

    #[test]
    fn unmapped_ecosystem_is_skipped_honestly() {
        let dir = fixture_dir();
        write_ghsa(
            dir.path(),
            "GHSA-test-0002",
            "2026-10-01T00:00:00Z",
            "Action: injection",
            "GitHub Actions",
            "evil/action",
        );
        let (payload, stats) = build_payload(dir.path(), 90, "2026-10-16", None);
        assert!(payload["advisories"].as_array().unwrap().is_empty());
        assert_eq!(stats.ecosystem_skipped, 1);
    }

    #[test]
    fn kill_list_precise_titles_extract_real_names() {
        // Every title below is a REAL GHSA convention (measured 16/10).
        assert_eq!(
            kill_list_name("`exploration` was removed from crates.io for malicious code"),
            Some("exploration".to_string())
        );
        assert_eq!(
            kill_list_name("Malicious code in `loadyaml`"),
            Some("loadyaml".to_string())
        );
        assert_eq!(
            kill_list_name("Malicious code in `electorn`"),
            Some("electorn".to_string())
        );
        assert_eq!(
            kill_list_name("node-fabric is malware"),
            Some("node-fabric".to_string())
        );
        assert_eq!(
            kill_list_name("symfont/process typosquatting malware spoofs symfony/process"),
            Some("symfont/process".to_string())
        );
    }

    #[test]
    fn kill_list_never_matches_generic_malicious_titles() {
        // The FP trap: titles ABOUT legitimate packages + attack vectors.
        // A bare /malicious/ regex would blacklist these (they are real GHSAs
        // for `jsonata`, `surrealdb`, `nltk` — all legit).
        assert_eq!(kill_list_name("jsonata: Malicious inputs to \"$toMillis\" function can cause resource exhaustion"), None);
        assert_eq!(
            kill_list_name("SurrealDB: Crafting malicious LIVE queries writes to the database"),
            None
        );
        assert_eq!(
            kill_list_name("NLTK: Missing Post-Download Integrity Verification Allows Malicious Package Injection"),
            None
        );
        assert_eq!(kill_list_name("Wings: Maliciously crafted packet causes DoS"), None);
        // Version-scoped titles: the PACKAGE is legit, specific versions were
        // compromised → name-level kill list would FP for patched users.
        // (Real titles; the advisory path still covers them in-window.)
        assert_eq!(
            kill_list_name("Malicious code in guardrails-ai 0.10.1 (supply chain compromise)"),
            None
        );
        assert_eq!(
            kill_list_name("Malicious code in @beproduct/nestjs-auth (0.1.2 through 0.1.19) — Mini Shai-Hulud worm"),
            None
        );
        // ...and `crates.io` in a removal title is NOT a version.
        assert!(kill_list_name("`exploration` was removed from crates.io for malicious code").is_some());
    }

    #[test]
    fn kill_list_titles_become_bad_names_even_out_of_window() {
        let dir = fixture_dir();
        write_ghsa(
            dir.path(),
            "GHSA-test-0003",
            "2019-01-01T00:00:00Z",
            "old-typosquat is malware",
            "npm",
            "old-typosquat",
        );
        let (payload, stats) = build_payload(dir.path(), 90, "2026-10-16", None);
        let bad = payload["bad_names"].as_array().unwrap();
        assert_eq!(bad.len(), 1, "kill list não tem janela");
        assert_eq!(bad[0]["package"], "old-typosquat");
        assert_eq!(bad[0]["ecosystem"], "npm");
        assert!(bad[0]["reason"].as_str().unwrap().contains("malware"));
        assert_eq!(stats.kill_list, 1);
        assert!(payload["advisories"].as_array().unwrap().is_empty());
    }

    #[test]
    fn kill_list_package_is_not_double_listed_as_advisory() {
        let dir = fixture_dir();
        write_ghsa(
            dir.path(),
            "GHSA-test-0004",
            "2026-10-01T00:00:00Z",
            "sneaky-pkg is malware",
            "npm",
            "sneaky-pkg",
        );
        let (payload, _) = build_payload(dir.path(), 90, "2026-10-16", None);
        assert_eq!(payload["bad_names"].as_array().unwrap().len(), 1);
        assert!(
            payload["advisories"].as_array().unwrap().is_empty(),
            "hit exato é high só — não duplica como low"
        );
    }

    #[test]
    fn output_is_deterministic_and_sorted() {
        let dir = fixture_dir();
        write_ghsa(dir.path(), "GHSA-test-zzz", "2026-10-01T00:00:00Z", "x y", "npm", "zeta");
        write_ghsa(dir.path(), "GHSA-test-aaa", "2026-10-02T00:00:00Z", "x y", "npm", "alpha");
        write_ghsa(dir.path(), "GHSA-test-mmm", "2026-10-03T00:00:00Z", "a b", "PyPI", "mid");
        let (a, _) = build_payload(dir.path(), 90, "2026-10-16", None);
        let (b, _) = build_payload(dir.path(), 90, "2026-10-16", None);
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap(),
            "mesma entrada → mesmos bytes (base de assinatura estável)"
        );
        let packages: Vec<&str> = a["advisories"]
            .as_array()
            .unwrap()
            .iter()
            .map(|adv| adv["package"].as_str().unwrap())
            .collect();
        assert_eq!(packages, vec!["alpha", "zeta", "mid"], "chave (ecossistema, pacote): npm < pypi");
    }

    #[test]
    fn bad_list_merges_and_dedupes() {
        let dir = fixture_dir();
        write_ghsa(
            dir.path(),
            "GHSA-test-0005",
            "2026-10-01T00:00:00Z",
            "curated-pkg is malware",
            "npm",
            "curated-pkg",
        );
        let list_path = dir.path().join("bad-list.json");
        std::fs::write(
            &list_path,
            serde_json::json!([
                {"package": "curated-pkg", "ecosystem": "npm", "reason": "dup"},
                {"package": "manual-one", "ecosystem": "pypi", "reason": "fonte: relato manual"},
            ])
            .to_string(),
        )
        .unwrap();
        let (payload, stats) = build_payload(dir.path(), 90, "2026-10-16", Some(&list_path));
        let bad = payload["bad_names"].as_array().unwrap();
        assert_eq!(bad.len(), 2, "dup não duplica");
        assert_eq!(stats.bad_list_extra, 1, "só o manual é extra");
    }

    #[test]
    fn ghsa_url_and_plausible_names_hold_up() {
        assert_eq!(kill_list_name("npm package \nnewline is malware"), None, "espaço = rejeito");
        assert_eq!(kill_list_name("Empty: "), None);
        // Trailing punctuation of a title is stripped.
        assert_eq!(
            kill_list_name("left-pad: typosquatting malware in npm"),
            Some("left-pad".to_string())
        );
    }
}
