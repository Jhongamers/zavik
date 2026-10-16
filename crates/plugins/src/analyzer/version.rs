//! Version utilities used by the analysis.

use std::cmp::Ordering;

/// Extracts the "base version" from a version requirement.
///
/// - `">=4.2"`   -> `Some("4.2")`
/// - `"^18.2.0"` -> `Some("18.2.0")`
/// - `"~1.0"`    -> `Some("1.0")`
/// - `"*"`       -> `None`
pub(crate) fn base_version(requirement: Option<&str>) -> Option<String> {
    let requirement = requirement?.trim();
    let start = requirement.find(|c: char| c.is_ascii_digit())?;
    let rest = &requirement[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    let value = rest[..end].trim_end_matches('.');
    (!value.is_empty()).then(|| value.to_string())
}

/// If the requirement is an **exact version** (not a range), return it.
/// `"=1.0.0"`/`"v1.0.0"`/`"1.0.21"` → `Some`; `"^1.0.0"`/`"~1.0"`/`"1"` → `None`.
pub(crate) fn exact_version(requirement: Option<&str>) -> Option<String> {
    let raw = requirement?.trim();
    let t = raw.strip_prefix('=').unwrap_or(raw).trim();
    let t = t.strip_prefix('v').unwrap_or(t);
    // Requires a dot (avoids ambiguous "1"/"18") and only digits/dots.
    if t.contains('.')
        && t.chars().all(|c| c.is_ascii_digit() || c == '.')
        && t.chars().any(|c| c.is_ascii_digit())
    {
        Some(t.to_string())
    } else {
        None
    }
}

/// Compares two versions simply (semver style, ignores pre-release).
pub(crate) fn compare(a: &str, b: &str) -> Ordering {
    let left = numeric_parts(a);
    let right = numeric_parts(b);
    let len = left.len().max(right.len());
    for index in 0..len {
        let x = left.get(index).copied().unwrap_or(0);
        let y = right.get(index).copied().unwrap_or(0);
        match x.cmp(&y) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}

fn numeric_parts(version: &str) -> Vec<u64> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap_or(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_base() {
        assert_eq!(base_version(Some(">=4.2")), Some("4.2".to_string()));
        assert_eq!(base_version(Some("^18.2.0")), Some("18.2.0".to_string()));
        assert_eq!(base_version(Some("*")), None);
        assert_eq!(base_version(None), None);
    }

    #[test]
    fn compares_versions() {
        assert_eq!(compare("1.2.0", "1.2.0"), Ordering::Equal);
        assert_eq!(compare("1.10.0", "1.9.0"), Ordering::Greater);
        assert_eq!(compare("1.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare("0.9", "1.0.0"), Ordering::Less);
    }
}
