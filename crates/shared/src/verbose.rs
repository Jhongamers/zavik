//! Internal trace gating — what reaches the terminal besides real output.
//!
//! The kernel/core/events crates print diagnostic traces (`KERNEL: …`,
//! `CORE: …`, `EVENTBUS: …`, `[logger] …`) that describe engine internals,
//! not project data. Modern CLIs keep those quiet by default, so they are
//! **hidden unless `ZAVIK_VERBOSE` is set** (opt-in, mirroring the
//! `ZAVIK_NO_UPDATE_CHECK` precedent).
//!
//! Why a gate and not deletion:
//!   - the traces are the first thing we ask users for in bug reports;
//!   - i18n keeps them translated (pt-BR + en) — see `docs/design-i18n.md`.
//!
//! Traces go to **stderr only** (`CORE: …` used to leak into stdout, which
//! polluted `zavik detect > file` and any `2>&1` pipeline into
//! `zavik diagnose`). See `docs/design-style.md` §7 for the decision record.
//!
//! The env read is process-wide and happens per call (a few calls per run, no
//! measurable cost) — which also keeps behavior testable via child processes,
//! like `style::colors_enabled`.

/// Whether internal traces are enabled for this process.
///
/// `ZAVIK_VERBOSE` set and not `"0"` → traces on; unset/empty/`"0"` → off.
pub fn enabled() -> bool {
    from_value(std::env::var("ZAVIK_VERBOSE").ok().as_deref())
}

/// Truth table of the flag, extracted as a pure function so it can be unit
/// tested without mutating process-wide env (which would race under
/// parallel tests).
fn from_value(raw: Option<&str>) -> bool {
    match raw {
        Some(value) => !value.is_empty() && value != "0",
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::from_value;

    #[test]
    fn unset_means_traces_off() {
        assert!(!from_value(None));
    }

    #[test]
    fn empty_means_traces_off() {
        assert!(!from_value(Some("")));
    }

    #[test]
    fn zero_means_traces_off() {
        assert!(!from_value(Some("0")));
    }

    #[test]
    fn one_means_traces_on() {
        assert!(from_value(Some("1")));
    }

    #[test]
    fn any_other_value_means_traces_on() {
        assert!(from_value(Some("true")));
    }
}
