# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability, please do **not** file a public issue.

Contact the maintainers through a private channel. Do not disclose the issue publicly until it has been addressed and a release has been published.

## Dependencies

- **cargo-audit** runs on a nightly schedule to detect known vulnerabilities in the dependency tree.
- **cargo-deny** runs on every pull request to audit dependency licenses and flag advisories.
