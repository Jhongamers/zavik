//! Pro-tier risk dataset: signed envelope, local (XDG) storage, staleness
//! and the **pure matching** that turns a dataset into advisory findings.
//!
//! FP policy (docs/design-data.md §2.1): advisory matches by package NAME
//! only, at `low` ("possible exposure — check your version"); known-bad name
//! matches are exact, at `high`. Neither touches the confluence score,
//! `passed`, nor `--fail-on`: the free tier's output stays byte-identical.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use devforge_shared::{Language, ProjectStack};

/// Envelope + payload schema this build understands.
pub const DATASET_SCHEMA: u32 = 2;
/// Installations older than this are honestly flagged as stale.
pub const STALE_AFTER_DAYS: i64 = 14;
/// Hard cap on dataset size (a few MB is the expectation; 32 MB is generous).
pub const MAX_DATASET_BYTES: usize = 32 * 1024 * 1024;

/// Public key (base64url, no padding) of the **dataset** signer — separate
/// from the license key on purpose (blast radius: a leaked dataset key cannot
/// mint licenses). Filled via `license_tool genkey-dataset`. Rotation = new
/// CLI release.
pub const DATASET_PUBLIC_KEY_B64: &str = "jSunlm4BzG3Q3-THwoaG4z-IeM4XjpGb4MzekC1UWns";

/// Default origin of the dataset (a GitHub Release asset). Empty until the
/// repository exists (ROADMAP F0); then fill
/// `https://github.com/<owner>/<repo>/releases/download/dataset-latest/dataset.json`.
/// Override per call with `--url` or `DF_DATASET_URL` (also how tests and
/// air-gapped mirrors point at local files: `file://<path>` or a bare path).
pub const DEFAULT_DATASET_URL: &str = "";

/// The signed envelope as downloaded (one Release asset, one file).
#[derive(Debug, Serialize, Deserialize)]
pub struct DatasetEnvelope {
    pub schema: u32,
    /// base64url of the dataset JSON bytes (what actually gets signed).
    pub payload: String,
    /// base64url Ed25519 signature over the raw payload bytes.
    pub signature: String,
}

/// The dataset payload (schema 2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Dataset {
    pub schema: u32,
    /// Human-facing version (usually the generation date).
    pub version: String,
    /// Generation date `YYYY-MM-DD` (drives staleness).
    pub generated: String,
    /// Recent advisories (built from `github/advisory-database`, ~90d window).
    #[serde(default)]
    pub advisories: Vec<Advisory>,
    /// Exact-match kill list of reported typosquat/malicious package names.
    #[serde(default)]
    pub bad_names: Vec<BadName>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Advisory {
    pub package: String,
    /// GHSA ecosystem name (`npm`, `pypi`, `crates.io`, ...).
    pub ecosystem: String,
    pub id: String,
    pub url: String,
    pub published: String,
    /// `"advisory"` (default) or `"malicious"`.
    #[serde(default)]
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BadName {
    pub package: String,
    pub ecosystem: String,
    /// Why it's on the list (short, human).
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Why a dataset was refused (honest, pt-BR messages for humans).
#[derive(Debug, PartialEq)]
pub enum DatasetError {
    /// This build has no public key embedded (not yet generated/rotated).
    PublicKeyMissing,
    /// Not a valid envelope (malformed JSON / bad base64).
    Envelope,
    /// Signature does not verify: tampered file or foreign key. Refused.
    Signature,
    /// Envelope or payload declares a schema this build doesn't understand.
    Schema(u32),
    /// Signed bytes are not a valid dataset payload.
    Payload,
    /// Above [`MAX_DATASET_BYTES`] — refused without buffering further.
    TooLarge,
    Io(String),
}

impl DatasetError {
    /// Message in the target language: JSON prose is always English
    /// (docs/design-i18n.md); human output goes through `Display`, which
    /// renders in the current language.
    pub(crate) fn message_in(&self, lang: devforge_shared::i18n::Lang) -> String {
        use devforge_shared::i18n::{t_fmt_in, translate};
        match self {
            DatasetError::PublicKeyMissing => translate(
                lang,
                "chave pública do dataset não configurada nesta build",
            )
            .to_string(),
            DatasetError::Envelope => {
                translate(lang, "formato do dataset inválido (envelope malformado)").to_string()
            }
            DatasetError::Signature => translate(
                lang,
                "assinatura do dataset inválida — arquivo adulterado ou de chave desconhecida; recusado",
            )
            .to_string(),
            DatasetError::Schema(found) => t_fmt_in(
                lang,
                "schema do dataset não suportado: {} (este build entende {})",
                &[&found.to_string(), &DATASET_SCHEMA.to_string()],
            ),
            DatasetError::Payload => {
                translate(lang, "conteúdo do dataset inválido (JSON ou campos)").to_string()
            }
            DatasetError::TooLarge => t_fmt_in(
                lang,
                "dataset acima do limite de {} bytes — recusado",
                &[&MAX_DATASET_BYTES.to_string()],
            ),
            DatasetError::Io(error) => t_fmt_in(lang, "erro de arquivo: {}", &[error]),
        }
    }
}

impl std::fmt::Display for DatasetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Human path: renders in the CURRENT language; JSON sites call
        // `message_in(Lang::En)` instead.
        f.write_str(&self.message_in(devforge_shared::i18n::lang()))
    }
}

impl std::error::Error for DatasetError {}

/// Path of the installed dataset (XDG on Linux — same base as the license).
pub fn dataset_file() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    base.join("devforge").join("dataset.json")
}

/// Embedded public key, already decoded (mirrors the license's helper).
fn embedded_public_key() -> Result<[u8; 32], DatasetError> {
    if DATASET_PUBLIC_KEY_B64.is_empty() {
        return Err(DatasetError::PublicKeyMissing);
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(DATASET_PUBLIC_KEY_B64)
        .map_err(|_| DatasetError::PublicKeyMissing)?;
    decoded.try_into().map_err(|_| DatasetError::PublicKeyMissing)
}

/// Core of the verification (split out so tests can use another key):
/// size cap → envelope → signature FIRST → then interpret the payload.
pub fn verify_envelope_with(
    bytes: &[u8],
    public_key: &[u8; 32],
) -> Result<Dataset, DatasetError> {
    if bytes.len() > MAX_DATASET_BYTES {
        return Err(DatasetError::TooLarge);
    }
    let envelope: DatasetEnvelope =
        serde_json::from_slice(bytes).map_err(|_| DatasetError::Envelope)?;
    if envelope.schema != DATASET_SCHEMA {
        return Err(DatasetError::Schema(envelope.schema));
    }

    let payload = URL_SAFE_NO_PAD
        .decode(&envelope.payload)
        .map_err(|_| DatasetError::Envelope)?;
    let signature_bytes: [u8; 64] = URL_SAFE_NO_PAD
        .decode(&envelope.signature)
        .map_err(|_| DatasetError::Envelope)?
        .try_into()
        .map_err(|_| DatasetError::Envelope)?;

    let key = VerifyingKey::from_bytes(public_key).map_err(|_| DatasetError::Signature)?;
    key.verify(&payload, &Signature::from_bytes(&signature_bytes))
        .map_err(|_| DatasetError::Signature)?;

    let dataset: Dataset = serde_json::from_slice(&payload).map_err(|_| DatasetError::Payload)?;
    if dataset.schema != DATASET_SCHEMA {
        return Err(DatasetError::Schema(dataset.schema));
    }
    Ok(dataset)
}

/// Reads + verifies the installed dataset.
/// `Ok(None)` = nothing installed; `Err` = installed but unusable (never used).
pub fn load_installed() -> Result<Option<Dataset>, DatasetError> {
    let path = dataset_file();
    if !path.exists() {
        // Absence is checked before the key: even a keyless build can honestly
        // say "nothing installed".
        return Ok(None);
    }
    load_installed_at(&path, &embedded_public_key()?)
}

fn load_installed_at(
    path: &Path,
    public_key: &[u8; 32],
) -> Result<Option<Dataset>, DatasetError> {
    match std::fs::read(path) {
        Ok(bytes) => verify_envelope_with(&bytes, public_key).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(DatasetError::Io(error.to_string())),
    }
}

/// Verifies FIRST, then atomically replaces the installed dataset.
/// Never writes bytes that didn't verify.
pub fn install(bytes: &[u8]) -> Result<Dataset, DatasetError> {
    let path = dataset_file();
    let parent = path
        .parent()
        .ok_or_else(|| DatasetError::Io("caminho do dataset sem diretório".into()))?;
    std::fs::create_dir_all(parent).map_err(|error| DatasetError::Io(error.to_string()))?;
    install_at(&path, bytes, &embedded_public_key()?)
}

fn install_at(path: &Path, bytes: &[u8], public_key: &[u8; 32]) -> Result<Dataset, DatasetError> {
    let dataset = verify_envelope_with(bytes, public_key)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes).map_err(|error| DatasetError::Io(error.to_string()))?;
    // Windows: rename does not overwrite — replace explicitly.
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    std::fs::rename(&tmp, path).map_err(|error| {
        let _ = std::fs::remove_file(&tmp);
        DatasetError::Io(error.to_string())
    })?;
    Ok(dataset)
}

/// Days between the dataset generation and `today` (both `YYYY-MM-DD`),
/// or `None` when a date is unparseable (honest unknown beats a fake number).
pub fn age_days(generated: &str, today: &str) -> Option<i64> {
    let generated_days = days_from_iso(generated)?;
    let today_days = days_from_iso(today)?;
    Some(today_days - generated_days)
}

pub fn is_stale(age: i64) -> bool {
    age > STALE_AFTER_DAYS
}

/// Dataset ecosystem name for a detected stack, or `None` when we don't
/// trust a mapping (honest skip beats a wrong-ecosystem hit).
pub fn ecosystem_of(language: Language) -> Option<&'static str> {
    Some(match language {
        Language::JavaScript | Language::TypeScript => "npm",
        Language::Python => "pypi",
        Language::Rust => "crates.io",
        Language::Go => "go",
        Language::Php => "packagist",
        Language::Ruby => "rubygems",
        Language::Java | Language::Kotlin => "maven",
        Language::CSharp => "nuget",
        Language::Dart => "pub",
        Language::Elixir => "hex",
        Language::C | Language::Cpp | Language::Swift | Language::Unknown => return None,
    })
}

/// One advisory finding proposed by the dataset (never confirmed — no
/// version range matching in v1, see design §2.1).
#[derive(Debug, Clone, PartialEq)]
pub struct DatasetFinding {
    pub package: String,
    pub ecosystem: String,
    /// `"bad_name"` (exact kill-list hit) or `"advisory"` (name in window).
    pub kind: &'static str,
    /// `"high"` for kill-list hits, `"low"` for possible exposure.
    pub severity: &'static str,
    /// Advisory id (`GHSA-...`) when there is one.
    pub id: Option<String>,
    pub detail: String,
    pub url: Option<String>,
}

/// What `audit` embeds when a valid local dataset exists (section 4.3).
#[derive(Debug, Clone)]
pub struct DatasetReport {
    pub version: String,
    pub generated: String,
    pub age_days: i64,
    pub stale: bool,
    pub advisories_total: usize,
    pub bad_names_total: usize,
    pub findings: Vec<DatasetFinding>,
}

/// Pure matching: project dependencies × dataset. FP policy in the module
/// docs; deterministic output (kill-list hits first, both groups sorted).
pub fn findings_for(
    stacks: &[ProjectStack],
    dataset: &Dataset,
    lang: devforge_shared::i18n::Lang,
) -> Vec<DatasetFinding> {
    use devforge_shared::i18n::t_fmt_in;
    let normalize_for = |ecosystem: &str, name: &str| normalize_name(ecosystem, name);

    let mut bad_index: HashMap<(String, String), &BadName> = HashMap::new();
    for bad in &dataset.bad_names {
        let ecosystem = bad.ecosystem.to_lowercase();
        bad_index.insert(
            (ecosystem.clone(), normalize_for(&ecosystem, &bad.package)),
            bad,
        );
    }

    let mut advisory_index: HashMap<(String, String), Vec<&Advisory>> = HashMap::new();
    for advisory in &dataset.advisories {
        let ecosystem = advisory.ecosystem.to_lowercase();
        advisory_index
            .entry((
                ecosystem.clone(),
                normalize_for(&ecosystem, &advisory.package),
            ))
            .or_default()
            .push(advisory);
    }

    let mut bad_hits: Vec<DatasetFinding> = Vec::new();
    let mut advisory_hits: Vec<DatasetFinding> = Vec::new();
    let mut seen_bad: HashSet<(String, String)> = HashSet::new();
    let mut seen_advisory: HashSet<(String, String)> = HashSet::new();

    for stack in stacks {
        let Some(ecosystem) = ecosystem_of(stack.language) else {
            continue;
        };
        for dependency in &stack.dependencies {
            let key = (
                ecosystem.to_string(),
                normalize_for(ecosystem, &dependency.name),
            );

            if let Some(bad) = bad_index.get(&key) {
                if seen_bad.insert(key.clone()) {
                    bad_hits.push(DatasetFinding {
                        package: dependency.name.clone(),
                        ecosystem: ecosystem.to_string(),
                        kind: "bad_name",
                        severity: "high",
                        id: None,
                        detail: t_fmt_in(
                            lang,
                            "nome consta na kill list como malicioso/typosquat: {}",
                            &[&bad.reason],
                        ),
                        url: bad.url.clone(),
                    });
                }
            }

            if let Some(list) = advisory_index.get(&key) {
                // ONE finding per package: several GHSAs for the same package
                // are common (the E2E on real data showed 9 for one name) —
                // grouping keeps the section actionable instead of repeating
                // the same advice. `list` preserves dataset order = sorted
                // by id at build time (deterministic).
                if seen_advisory.insert(key.clone()) {
                    let first = list[0];
                    let extra = list.len() - 1;
                    let detail = if extra == 0 {
                        t_fmt_in(
                            lang,
                            "possível exposição ({}) publicada em {} — confira se sua versão está coberta",
                            &[&first.id, &first.published],
                        )
                    } else {
                        t_fmt_in(
                            lang,
                            "possível exposição ({} + mais {} advisories para este pacote; este publicado em {}) — confira se sua versão está coberta",
                            &[&first.id, &extra.to_string(), &first.published],
                        )
                    };
                    advisory_hits.push(DatasetFinding {
                        package: dependency.name.clone(),
                        ecosystem: ecosystem.to_string(),
                        kind: "advisory",
                        severity: "low",
                        id: Some(first.id.clone()),
                        detail,
                        url: Some(first.url.clone()),
                    });
                }
            }
        }
    }

    bad_hits.sort_by(|a, b| a.package.cmp(&b.package));
    advisory_hits.sort_by(|a, b| {
        (&a.package, &a.id).cmp(&(&b.package, &b.id))
    });
    bad_hits.extend(advisory_hits);
    bad_hits
}

/// Builds the report `audit` embeds (meta + findings) for `today`.
pub fn build_report(
    stacks: &[ProjectStack],
    dataset: &Dataset,
    today: &str,
    lang: devforge_shared::i18n::Lang,
) -> DatasetReport {
    let age = age_days(&dataset.generated, today).unwrap_or(-1);
    DatasetReport {
        version: dataset.version.clone(),
        generated: dataset.generated.clone(),
        age_days: age,
        stale: age >= 0 && is_stale(age),
        advisories_total: dataset.advisories.len(),
        bad_names_total: dataset.bad_names.len(),
        findings: findings_for(stacks, dataset, lang),
    }
}

/// Package-name normalization per ecosystem: everything lowercase; PyPI also
/// folds `_`/`.` into `-` (PEP 503). RubyGems does NOT fold — `-` and `_` are
/// distinct gems there, so folding would create false positives.
fn normalize_name(ecosystem: &str, name: &str) -> String {
    let lowered = name.to_lowercase();
    if ecosystem == "pypi" {
        let mut out = String::with_capacity(lowered.len());
        let mut last_was_separator = false;
        for character in lowered.chars() {
            if character == '_' || character == '.' || character == '-' {
                if !last_was_separator {
                    out.push('-');
                }
                last_was_separator = true;
            } else {
                out.push(character);
                last_was_separator = false;
            }
        }
        out
    } else {
        lowered
    }
}

/// `YYYY-MM-DD` → days since epoch (Hinnant), strict field validation.
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

/// Signs a payload into the on-disk/on-wire envelope (tests only — the
/// shipping binary never signs, and the vendor tool signs with its own key).
#[cfg(test)]
pub fn sign_envelope_for_test(
    payload: &[u8],
    signing: &ed25519_dalek::SigningKey,
) -> Vec<u8> {
    use ed25519_dalek::Signer;
    let signature = signing.sign(payload);
    serde_json::to_vec(&DatasetEnvelope {
        schema: DATASET_SCHEMA,
        payload: URL_SAFE_NO_PAD.encode(payload),
        signature: URL_SAFE_NO_PAD.encode(signature.to_bytes()),
    })
    .expect("envelope serializa")
}

#[cfg(test)]
mod tests {
    use super::*;
    use devforge_shared::{Dependency, DependencyKind};
    use ed25519_dalek::SigningKey;
    use std::path::PathBuf;

    fn sample_dataset() -> Dataset {
        Dataset {
            schema: DATASET_SCHEMA,
            version: "2026-10-10".into(),
            generated: "2026-10-10".into(),
            advisories: vec![
                Advisory {
                    package: "lodash".into(),
                    ecosystem: "npm".into(),
                    id: "GHSA-test-1".into(),
                    url: "https://example.invalid/GHSA-test-1".into(),
                    published: "2026-09-01".into(),
                    kind: "advisory".into(),
                },
                Advisory {
                    package: "requests".into(),
                    ecosystem: "pypi".into(),
                    id: "GHSA-test-2".into(),
                    url: "https://example.invalid/GHSA-test-2".into(),
                    published: "2026-09-02".into(),
                    kind: "malicious".into(),
                },
            ],
            bad_names: vec![BadName {
                package: "lodahs".into(),
                ecosystem: "npm".into(),
                reason: "typosquat de lodash (relato de remoção)".into(),
                url: Some("https://example.invalid/bad-lodahs".into()),
            }],
        }
    }

    fn stack(language: Language, deps: &[(&str, &str)]) -> ProjectStack {
        ProjectStack {
            language,
            name: Some("fixture".into()),
            frameworks: vec![],
            package_manager: None,
            manifest: PathBuf::from("manifest"),
            dependencies: deps
                .iter()
                .map(|(name, version)| Dependency::new(*name, Some(version.to_string()), DependencyKind::Runtime))
                .collect(),
        }
    }

    fn signed(dataset: &Dataset, signing: &SigningKey) -> Vec<u8> {
        let payload = serde_json::to_vec(dataset).expect("payload serializa");
        sign_envelope_for_test(&payload, signing)
    }

    // --- envelope verification -------------------------------------------

    #[test]
    fn roundtrip_verifies_and_preserves_fields() {
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let public_key = signing.verifying_key().to_bytes();
        let bytes = signed(&sample_dataset(), &signing);
        let got = verify_envelope_with(&bytes, &public_key).expect("deveria verificar");
        assert_eq!(got, sample_dataset());
    }

    #[test]
    fn tampered_payload_is_rejected() {
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let public_key = signing.verifying_key().to_bytes();
        let bytes = signed(&sample_dataset(), &signing);
        let mut envelope: DatasetEnvelope = serde_json::from_slice(&bytes).unwrap();
        // Flip a byte inside the payload: signature must fail.
        envelope.payload.push('A');
        let tampered = serde_json::to_vec(&envelope).unwrap();
        assert_eq!(
            verify_envelope_with(&tampered, &public_key),
            Err(DatasetError::Signature)
        );
    }

    #[test]
    fn foreign_key_is_rejected() {
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let attacker = SigningKey::from_bytes(&[9u8; 32]);
        let bytes = signed(&sample_dataset(), &signing);
        assert_eq!(
            verify_envelope_with(&bytes, &attacker.verifying_key().to_bytes()),
            Err(DatasetError::Signature)
        );
    }

    #[test]
    fn garbage_and_wrong_schemas_are_rejected() {
        let public_key = SigningKey::from_bytes(&[7u8; 32]).verifying_key().to_bytes();
        assert_eq!(
            verify_envelope_with(b"not json at all", &public_key),
            Err(DatasetError::Envelope)
        );

        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let payload = serde_json::to_vec(&sample_dataset()).unwrap();
        let bad_envelope = serde_json::to_vec(&DatasetEnvelope {
            schema: 99,
            payload: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&payload),
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0u8; 64]),
        })
        .unwrap();
        assert_eq!(
            verify_envelope_with(&bad_envelope, &public_key),
            Err(DatasetError::Schema(99))
        );

        // Valid envelope, wrong payload schema.
        let mut wrong = sample_dataset();
        wrong.schema = 99;
        let bytes = signed(&wrong, &signing);
        assert_eq!(
            verify_envelope_with(&bytes, &public_key),
            Err(DatasetError::Schema(99))
        );
    }

    #[test]
    fn oversized_dataset_is_refused_before_parsing() {
        let public_key = SigningKey::from_bytes(&[7u8; 32]).verifying_key().to_bytes();
        let oversized = vec![0u8; MAX_DATASET_BYTES + 1];
        assert_eq!(
            verify_envelope_with(&oversized, &public_key),
            Err(DatasetError::TooLarge)
        );
    }

    #[test]
    fn embedded_key_is_present_and_decodes() {
        // The const was filled by `genkey-dataset`; a keyless build would
        // refuse everything honestly (DatasetError::PublicKeyMissing).
        assert_eq!(
            embedded_public_key().map(|bytes| bytes.len()),
            Ok(32),
            "chave pública do dataset embutida e decodificável (32 bytes)"
        );
    }

    // --- install / load (no env vars: explicit paths, unit-safe) ---------

    #[test]
    fn install_then_load_roundtrip_and_refuses_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dataset.json");
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let public_key = signing.verifying_key().to_bytes();
        let bytes = signed(&sample_dataset(), &signing);

        assert!(
            load_installed_at(&path, &public_key).unwrap().is_none(),
            "nada instalado ainda"
        );
        install_at(&path, &bytes, &public_key).expect("instala");
        let loaded = load_installed_at(&path, &public_key)
            .unwrap()
            .expect("deveria carregar");
        assert_eq!(loaded, sample_dataset());

        // Garbage on disk = invalid (Err), never trusted, never silently None.
        std::fs::write(&path, b"lixeira").unwrap();
        assert!(load_installed_at(&path, &public_key).is_err());
    }

    // --- staleness ---------------------------------------------------------

    #[test]
    fn age_and_staleness_boundaries() {
        assert_eq!(age_days("2026-10-10", "2026-10-10"), Some(0));
        assert_eq!(age_days("2026-10-01", "2026-10-15"), Some(14));
        assert!(!is_stale(14), "limite de 14 dias ainda é fresco");
        assert!(is_stale(15), "15 dias é atrasado");
        assert!(is_stale(age_days("2026-10-01", "2026-10-16").unwrap()));
        // Unparseable = honest unknown, not a fake zero.
        assert_eq!(age_days("2026-99-01", "2026-10-10"), None);
        assert_eq!(age_days("", "2026-10-10"), None);
    }

    // --- FP policy of the matching -----------------------------------------

    #[test]
    fn bad_name_exact_match_is_high_and_case_insensitive() {
        let dataset = sample_dataset();
        let stacks = [stack(Language::JavaScript, &[],)];
        // dependency added manually to control casing:
        let mut s = stacks[0].clone();
        s.dependencies
            .push(Dependency::new("LoDaHs", None, DependencyKind::Runtime));
        let findings = findings_for(&[s], &dataset, devforge_shared::i18n::Lang::PtBr);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, "bad_name");
        assert_eq!(findings[0].severity, "high");
        assert!(findings[0].detail.contains("kill list"));
        assert_eq!(findings[0].url.as_deref(), Some("https://example.invalid/bad-lodahs"));
    }

    #[test]
    fn advisory_name_match_is_low_with_link_even_with_version() {
        let dataset = sample_dataset();
        let stacks = [stack(Language::JavaScript, &[("lodash", "4.17.21")])];
        let findings = findings_for(&stacks, &dataset, devforge_shared::i18n::Lang::PtBr);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, "advisory");
        assert_eq!(findings[0].severity, "low", "v1 propõe, não confirma");
        assert!(findings[0].detail.contains("GHSA-test-1"));
        assert!(findings[0].detail.contains("confira"));
    }

    #[test]
    fn multiple_advisories_for_one_package_group_into_one_finding() {
        // Real data (staging-4 E2E): one npm package had 9 GHSAs in-window —
        // nine identical-looking lines are noise; the action is the same.
        let mut dataset = sample_dataset();
        dataset.advisories.push(Advisory {
            package: "lodash".into(),
            ecosystem: "npm".into(),
            id: "GHSA-test-9".into(),
            url: "https://example.invalid/GHSA-test-9".into(),
            published: "2026-09-15".into(),
            kind: "advisory".into(),
        });
        let stacks = [stack(Language::JavaScript, &[("lodash", "4.17.21")])];
        let findings = findings_for(&stacks, &dataset, devforge_shared::i18n::Lang::PtBr);
        assert_eq!(findings.len(), 1, "um pacote = um achado");
        assert!(findings[0].detail.contains("GHSA-test-1"), "id principal citado");
        assert!(
            findings[0].detail.contains("mais 1 advisories"),
            "o resto é contado, não repetido: {}",
            findings[0].detail
        );
    }

    #[test]
    fn near_miss_names_do_not_flag() {
        // FP test: similar-but-unlisted names must stay silent.
        let dataset = sample_dataset();
        let stacks = [stack(
            Language::JavaScript,
            &[("lodash-x", "1.0.0"), ("notlodahs", "1.0.0"), ("lo-dahs", "1.0.0")],
        )];
        assert!(findings_for(&stacks, &dataset, devforge_shared::i18n::Lang::PtBr).is_empty());
    }

    #[test]
    fn ecosystem_mismatch_does_not_flag() {
        // `lodash` advisory is npm-only; same name in a Go project = nothing.
        let dataset = sample_dataset();
        let stacks = [stack(Language::Go, &[("lodash", "v1.2.3")])];
        assert!(findings_for(&stacks, &dataset, devforge_shared::i18n::Lang::PtBr).is_empty());
    }

    #[test]
    fn pypi_normalization_matches_but_rubygems_does_not_fold() {
        let dataset = Dataset {
            schema: DATASET_SCHEMA,
            version: "2026-10-10".into(),
            generated: "2026-10-10".into(),
            advisories: vec![],
            bad_names: vec![
                BadName {
                    package: "foo-bar".into(),
                    ecosystem: "pypi".into(),
                    reason: "teste".into(),
                    url: None,
                },
                BadName {
                    package: "foo-bar".into(),
                    ecosystem: "rubygems".into(),
                    reason: "teste".into(),
                    url: None,
                },
            ],
        };

        // PyPI folds case + separators: `Foo_Bar` == `foo-bar` → matches.
        let py = [stack(Language::Python, &[("Foo_Bar", "1.0")])];
        assert_eq!(findings_for(&py, &dataset, devforge_shared::i18n::Lang::PtBr).len(), 1);

        // PyPI near-miss (`Foo_Bar_Baz` → `foo-bar-baz`) stays silent.
        let py_near = [stack(Language::Python, &[("Foo_Bar_Baz", "1.0")])];
        assert!(findings_for(&py_near, &dataset, devforge_shared::i18n::Lang::PtBr).is_empty());

        // RubyGems does NOT fold separators: `foo_bar` ≠ `foo-bar` → no match
        // (folding there would invent findings for distinct gems).
        let rb = [stack(Language::Ruby, &[("foo_bar", "1.0")])];
        assert!(findings_for(&rb, &dataset, devforge_shared::i18n::Lang::PtBr).is_empty());

        // ...while the exact rubygems name does match (case-only fold).
        let rb_exact = [stack(Language::Ruby, &[("Foo-Bar", "1.0")])];
        assert_eq!(findings_for(&rb_exact, &dataset, devforge_shared::i18n::Lang::PtBr).len(), 1);
    }

    #[test]
    fn findings_are_deterministic_bad_first_then_advisories_sorted() {
        let mut dataset = sample_dataset();
        dataset.bad_names.push(BadName {
            package: "zzz-typosquat".into(),
            ecosystem: "npm".into(),
            reason: "outro".into(),
            url: None,
        });
        let stacks = [stack(
            Language::JavaScript,
            &[("zeta-lib", "1.0.0"), ("lodahs", "1.0.0"), ("lodash", "4.0.0"), ("aaa-lib", "1.0.0")],
        )];
        // Add second advisory to check sorting.
        dataset.advisories.push(Advisory {
            package: "aaa-lib".into(),
            ecosystem: "npm".into(),
            id: "GHSA-test-3".into(),
            url: "https://example.invalid/GHSA-test-3".into(),
            published: "2026-09-03".into(),
            kind: "advisory".into(),
        });

        let a = findings_for(&stacks, &dataset, devforge_shared::i18n::Lang::PtBr);
        let b = findings_for(&stacks, &dataset, devforge_shared::i18n::Lang::PtBr);
        assert_eq!(a, b, "saída determinística");
        let kinds: Vec<&str> = a.iter().map(|f| f.kind).collect();
        assert_eq!(kinds, ["bad_name", "advisory", "advisory"]);
        assert_eq!(a[1].package, "aaa-lib", "advisories ordenadas por pacote");
        assert_eq!(a[2].package, "lodash");
    }

    #[test]
    fn ecosystem_mapping_is_conservative() {
        assert_eq!(ecosystem_of(Language::JavaScript), Some("npm"));
        assert_eq!(ecosystem_of(Language::TypeScript), Some("npm"));
        assert_eq!(ecosystem_of(Language::Python), Some("pypi"));
        assert_eq!(ecosystem_of(Language::Rust), Some("crates.io"));
        assert_eq!(ecosystem_of(Language::Unknown), None, "sem chute honesto");
        assert_eq!(ecosystem_of(Language::C), None);
    }
}
