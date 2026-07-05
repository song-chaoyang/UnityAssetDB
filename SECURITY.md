# Security Policy

## Supported Versions

We currently provide security updates for the latest release.

| Version | Supported |
|---------|-----------|
| Latest  | ✅ |
| Older   | ❌ |

## Reporting a Vulnerability

Please do **not** open a public issue for security vulnerabilities.

Instead, report security issues privately via GitHub's **Security Advisories**:

1. Go to **Security → Advisories → New advisory**
2. Provide a detailed description including:
   - Affected version(s)
   - Steps to reproduce
   - Impact description (if known)
   - Suggested fix (if any)

You can expect an acknowledgement within **48 hours**. We will investigate and, if the vulnerability is confirmed, we will release a fix as soon as possible and disclose it responsibly.

## Scope

This project analyzes local Unity project files. It does **not**:

- Send data to any network service (all analysis is local, the optional web server binds to localhost unless configured otherwise)
- Execute arbitrary code from project assets (parsers are pure analysis, no script execution)
- Read files outside the indexed Unity project directory

Utilities that ingest untrusted input (Unity project YAML, C# source) should still treat that input as untrusted: crafted YAML/JSON edge cases should never cause memory unsafe behavior. If you find a case where parsing malformed input causes a crash, panic, or memory issue, please report it.