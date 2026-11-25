# Security Policy

## Supported versions

Only the latest release receives security fixes. Please update before reporting.

| Version        | Supported |
| -------------- | --------- |
| Latest release | Yes       |
| Older releases | No        |

## Reporting a vulnerability

Please do not open a public issue, discussion, or pull request for security problems.

Report privately through GitHub instead:
[Report a vulnerability](https://github.com/kunkka19xx/look/security/advisories/new)

Include:

- The affected platform (macOS, Linux, Windows) and Look version
- Steps to reproduce, or a proof of concept
- The impact you expect (code execution, data exposure, privilege escalation, etc.)

## What to expect

- Acknowledgement within 7 days
- A fix or mitigation plan once the report is confirmed
- Credit in the published advisory, unless you prefer to stay anonymous

## Scope

In scope:

- The Look apps for macOS, Linux, and Windows, and the shared Rust core
- Local data handling: the index, clipboard history, and config files
- Private context reaching a non-local AI provider
- Release artifacts and install paths (Homebrew, Nix, installers)

Out of scope:

- Attacks that require an already compromised user account
- Vulnerabilities in third-party dependencies with no demonstrated impact on Look; report those upstream
