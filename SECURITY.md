# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

**Do not file a public issue.**  Use GitHub's **private vulnerability reporting**:

1. Go to the [Security tab](https://github.com/mcunha/tomlini/security) → **Report a vulnerability**.
2. Describe the issue.  Include a minimal reproduction if possible.
3. The maintainers will respond within **72 hours**.
## Enabling Private Reporting

The repository maintainer must enable private vulnerability reporting:
- Go to **Settings → Code security → Private vulnerability reporting → Enable**.

If GitHub's private reporting is not yet enabled, open a **private security advisory**:
- Go to [Security → Advisories → New draft security advisory](https://github.com/mcunha/tomlini/security/advisories/new) and select "Request CVE" for critical issues.
We will acknowledge your report within 72 hours, keep you updated on progress,
and credit you in the release notes (unless you prefer to remain anonymous).

## Dependencies

- **cargo-audit** runs nightly to detect known vulnerabilities in the dependency tree.
- **cargo-deny** runs on every pull request to audit dependency licenses and flag advisories.
