# Security Policy

Zavik is a supply-chain security tool. Reports about Zavik itself are held
to the same standard we ask of the dependencies we analyze.

## Supported versions

Only the **latest release** receives security fixes. Older binaries are not
patched — upgrade to the newest version before reporting (the `install.sh`
installer always fetches the latest release).

| Version                | Supported |
| ---------------------- | --------- |
| latest release         | ✅        |
| older releases / builds from source | ❌ |

## Reporting a vulnerability

Report through **GitHub's private vulnerability reporting**:
the repository's **Security tab → Report a vulnerability**. This opens a
private advisory thread visible only to the maintainer.

Please include, when possible:

- Zavik version (`zavik --version`) and OS/arch.
- The impact (what an attacker gains: RCE, false negatives in audit
  results, signature bypass, data exposure...).
- Minimal reproduction steps or a proof of concept.
- Anything you already tried.

Please **do not** open a public issue for a vulnerability, and do not
disclose details publicly before a fix or an agreed timeline.

## Honest expectations (no SLA theater)

This is a solo-maintained project. The commitment is honest, not
guaranteed: reports are read, and valid ones get a fix or an honest
explanation. Response time depends on availability — a critical issue
(RCE, signature-verification bypass) is treated as priority one when seen.

## Scope

In scope: the `zavik` CLI, the released binaries and checksums, the
dataset signature verification (`data update`), the `install.sh` installer,
and this repository's CI/publishing pipeline.

Out of scope, honestly stated:

- Vulnerabilities **in analyzed dependencies** that are already publicly
  disclosed upstream (that is what the tool reports — not a Zavik bug).
- Denial of service against local CLI runs (the user runs it on their own
  machine).
- Findings in projects you do not have permission to test — run Zavik
  only on code you are allowed to analyze.

## Safe harbor

Good-faith security research on this project is welcome and will not be
treated as an attack. As long as you: do not access or destroy data that
is not yours, do not degrade the service for other users (this project has
no user data by design — offline-first), and report findings privately
first. Research stays within the law and these bounds, and no legal action
will be taken against you for it.
