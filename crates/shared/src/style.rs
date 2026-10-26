//! Terminal presentation styling — colors, symbols and box drawing.
//!
//! Hand-rolled ANSI (zero dependencies, per the supply-chain stance: every
//! dependency we ship is attack surface). Colors are **presentation only**:
//! they never touch the machine contract (`--json`, SARIF, exit codes) and
//! they self-disable when stdout is not a terminal (pipes, CI, `NO_COLOR`).

use std::io::IsTerminal;

/// Whether ANSI colors should be emitted on stdout right now.
///
/// Resolution order (industry convention):
/// 1. `NO_COLOR` set (any value, per no-color.org) → off
/// 2. `CLICOLOR_FORCE` set and non-"0" → on (even without a TTY)
/// 3. stdout is a terminal → on, else off
pub fn colors_enabled() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if std::env::var("CLICOLOR_FORCE")
        .map(|v| v != "0")
        .unwrap_or(false)
    {
        return true;
    }
    std::io::stdout().is_terminal()
}

fn paint(code: &str, text: &str) -> String {
    if colors_enabled() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

/// Bold (headers, primary labels).
pub fn bold(text: &str) -> String {
    paint("1", text)
}

/// Dim (secondary info: paths, manifests, hints).
pub fn dim(text: &str) -> String {
    paint("2", text)
}

/// Cyan (section titles, primary accent).
pub fn cyan(text: &str) -> String {
    paint("36", text)
}

/// Bold cyan (banner title).
pub fn cyan_bold(text: &str) -> String {
    paint("1;36", text)
}

/// Green (success, OK statuses).
pub fn green(text: &str) -> String {
    paint("32", text)
}

/// Yellow (warnings, attention).
pub fn yellow(text: &str) -> String {
    paint("33", text)
}

/// Red (critical, failures).
pub fn red(text: &str) -> String {
    paint("31", text)
}

/// Status symbols (width-stable Unicode — replacing emoji that render
/// inconsistently across terminals and wrap badly in logs/CI output).
pub const OK: &str = "✓";
pub const WARN: &str = "⚠";
pub const CRIT: &str = "✗";
pub const INFO: &str = "ℹ";
pub const STEP: &str = "▸";
pub const RUN: &str = "▶";
pub const SKIP: &str = "↷";

/// Renders a status symbol colored by severity kind
/// ("ok" / "warn" / "crit" / anything else = info).
pub fn symbol(kind: &str) -> String {
    match kind {
        "ok" => green(OK),
        "warn" => yellow(WARN),
        "crit" => red(CRIT),
        _ => cyan(INFO),
    }
}

/// Box-drawing frame for the banner:
/// ```text
/// ┌─────────────────────────────────────┐
/// │ ZAVIK — context                     │
/// │ project: /path                      │
/// └─────────────────────────────────────┘
/// ```
/// Frame is dim, the title bold cyan, the rows plain. Width adapts to the
/// content (clamped to a sane 58 inner cols).
pub fn framed(title: &str, rows: &[String]) -> String {
    let inner = rows
        .iter()
        .map(|r| r.chars().count())
        .chain(std::iter::once(title.chars().count()))
        .max()
        .unwrap_or(24)
        .clamp(24, 58);

    let mut out = String::new();
    out.push_str(&dim(&format!("┌{}┐", "─".repeat(inner + 2))));
    out.push('\n');
    out.push_str(&dim("│"));
    out.push(' ');
    out.push_str(&cyan_bold(title));
    out.push_str(&" ".repeat(inner.saturating_sub(title.chars().count())));
    out.push(' ');
    out.push_str(&dim("│"));
    out.push('\n');
    for row in rows {
        out.push_str(&dim("│"));
        out.push(' ');
        out.push_str(row);
        out.push_str(&" ".repeat(inner.saturating_sub(row.chars().count())));
        out.push(' ');
        out.push_str(&dim("│"));
        out.push('\n');
    }
    out.push_str(&dim(&format!("└{}┘", "─".repeat(inner + 2))));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_is_noop_without_terminal() {
        // cargo test runs with piped stdout and no CLICOLOR_FORCE → off.
        assert!(!colors_enabled(), "stdout piped nos testes: sem cor");
        assert_eq!(green(OK), OK, "sem ANSI em ambiente sem TTY");
        assert_eq!(framed("t", &[]), framed("t", &[]), "idempotente");
    }

    #[test]
    fn symbols_are_single_cell() {
        // Width-stable: no wide emoji (they break alignment in logs).
        for symbol in [OK, WARN, CRIT, INFO, STEP, RUN, SKIP] {
            assert_eq!(symbol.chars().count(), 1, "1 char: {symbol}");
        }
    }

    #[test]
    fn framed_contains_title_and_rows_and_closes() {
        let out = framed("ZAVIK — context", &["project: /tmp/x".to_string()]);
        assert!(out.contains("ZAVIK — context"), "{out}");
        assert!(out.contains("project: /tmp/x"), "{out}");
        assert_eq!(out.lines().count(), 4, "moldura + título + 1 row: {out}");
        assert!(
            out.starts_with('┌') && out.trim_end().ends_with('┘'),
            "{out}"
        );
    }

    #[test]
    fn framed_narrows_for_tiny_content() {
        // Content smaller than the 24-col floor still gets the floor.
        let out = framed("ab", &["cd".to_string()]);
        assert_eq!(out.lines().next().unwrap().chars().count(), 28, "{out}");
    }
}
