# GitHub Automation Notes

This folder contains workflow and repo automation config.

## Workflows

- `workflows/ci.yml`
  - regular CI checks for pushes/PRs
  - does not run Apple signing/notarization

- `workflows/release-macos.yml`
  - lightweight release entrypoint
  - triggers on:
    - manual dispatch (`workflow_dispatch`)
    - tag push (`v*`)
  - delegates to reusable release workflow

- `workflows/reusable-release-macos.yml`
  - full macOS release pipeline:
    - build artifact
    - optional Developer ID signing
    - optional notarization + stapling
    - artifact upload and optional GitHub Release attach

## Strict mode

- strict mode is enabled when release ref/version text includes `strict`
  - examples: `1.2.3-strict`, `v1.2.3-strict`
- in strict mode, missing secrets fail fast and notarization timeout/failure fails the run
- in non-strict mode, notarization queue timeout can continue with signed (unstapled) artifact for testing

## Required secrets for signed + notarized releases

- `APPLE_DEVELOPER_ID_CERT_BASE64`
- `APPLE_DEVELOPER_ID_CERT_PASSWORD`
- `APPLE_KEYCHAIN_PASSWORD`
- `APPLE_DEVELOPER_ID_APPLICATION`
- `APPLE_NOTARY_APPLE_ID`
- `APPLE_NOTARY_APP_PASSWORD`
- `APPLE_NOTARY_TEAM_ID`

See `docs/apple-developer-release-guide.md` for full setup and troubleshooting.
