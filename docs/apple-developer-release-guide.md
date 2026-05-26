# Apple Developer Release Guide (look)

Use this guide after joining Apple Developer to ship signed + notarized macOS releases for `look`.

## Goal

- remove first-run Gatekeeper workaround (`Open Anyway`)
- ship Developer ID signed + notarized release zip
- keep Homebrew install flow clean

## 1) Create Developer ID certificate

Recommended: create from Xcode (fastest)

1. Open `Xcode` -> `Settings` -> `Accounts`.
2. Select your Apple account/team.
3. Click `Manage Certificates...`.
4. Create `Developer ID Application` certificate.

Alternative (manual CSR path):

1. Open `Keychain Access`.
2. `Certificate Assistant` -> `Request a Certificate From a Certificate Authority...`.
3. Save CSR to disk.
4. On Apple Developer portal, create `Developer ID Application` cert using that CSR.
5. Download `.cer` and double-click to import into Keychain.

Web-created cert note:

- creating the certificate from Apple Developer website is fully supported
- the key pair must be generated on your Mac (via CSR), then that same Mac must import the issued `.cer`
- if cert is created on web with a CSR from a different machine, this Mac will not have the matching private key and signing identities will be invalid

Notes:

- choose `G2 Sub-CA (Xcode 11.4.1 or later)` when prompted
- cert should appear in `login` keychain under `My Certificates`
- `System Roots` is not where you export your cert

## 2) Export certificate for CI

1. In `Keychain Access` -> `login` -> `My Certificates`.
2. Find `Developer ID Application: <Name> (<TEAMID>)` and confirm private key exists.
3. Right-click -> `Export` -> save as `.p12`.
4. Set export password (store safely).

Convert `.p12` to base64:

```bash
base64 -i /path/to/developer-id.p12
```

The command prints one long string. Copy the full output into GitHub secret `APPLE_DEVELOPER_ID_CERT_BASE64`.

Tip (copy directly to clipboard):

```bash
base64 -i /path/to/developer-id.p12 | pbcopy
```

## 3) Collect required values

### Signing identity

```bash
security find-identity -v -p codesigning
```

Copy the value inside quotes, for example from:

```text
1) <hash> "Developer ID Application: Your Name (TEAMID)"
```

Use only:

```text
Developer ID Application: Your Name (TEAMID)
```

### Notary credentials

- Apple ID email
- app-specific password from `appleid.apple.com`
- Apple team ID (10 chars)

How to create app-specific password:

1. Open `https://appleid.apple.com` and sign in.
2. Go to `Sign-In and Security`.
3. Under `App-Specific Passwords`, click `Generate`.
4. Create one (for example label: `look-notary-gha`).
5. Copy it immediately and save as `APPLE_NOTARY_APP_PASSWORD`.

How to find Team ID (`APPLE_NOTARY_TEAM_ID`):

1. Open `https://developer.apple.com/account` and sign in.
2. Go to `Membership` (account details).
3. Copy `Team ID` (10-character value, for example `AB12C3D4E5`).
4. Save it as GitHub secret `APPLE_NOTARY_TEAM_ID`.

If your Apple ID belongs to multiple teams, use the Team ID that matches the team owning your `Developer ID Application` certificate.

## 4) Add GitHub Actions secrets

Repo -> `Settings` -> `Secrets and variables` -> `Actions`.

Required secrets:

| Secret | What it is | Where to get it | Used for |
|---|---|---|---|
| `APPLE_DEVELOPER_ID_CERT_BASE64` | Base64-encoded `.p12` file that contains your Developer ID Application cert + private key | `base64 -i /path/to/developer-id.p12` | Imported into temporary CI keychain before codesign |
| `APPLE_DEVELOPER_ID_CERT_PASSWORD` | Password you set when exporting the `.p12` | Chosen during Keychain export | Decrypts `.p12` during CI import |
| `APPLE_KEYCHAIN_PASSWORD` | Password for the temporary CI keychain | Generate a strong random string | Creates/unlocks keychain in GitHub Actions runner |
| `APPLE_DEVELOPER_ID_APPLICATION` | Exact signing identity string | `security find-identity -v -p codesigning` | Passed to `codesign --sign` |
| `APPLE_NOTARY_APPLE_ID` | Apple ID email for notarization auth | Your Apple Developer account email | Auth for `xcrun notarytool submit` |
| `APPLE_NOTARY_APP_PASSWORD` | Apple app-specific password (not your normal Apple ID password) | `appleid.apple.com` -> Security -> App-Specific Passwords | Auth for `xcrun notarytool submit` |
| `APPLE_NOTARY_TEAM_ID` | 10-character Apple Team ID | Apple Developer account membership details | Team context for notarization submission |

Quick notes:

- keep all values in GitHub **Secrets** (not plain env vars in workflow YAML)
- if one value is wrong, the related step fails clearly (`codesign` for signing vars, `notarytool` for notary vars)

## 5) Run a test release workflow

1. Open GitHub Actions -> `Release macOS App`.
2. Run `workflow_dispatch` with a test version.
   - include `strict` in the version text (for example `1.0.0-strict`) to require signing + notarization
   - omit `strict` for best-effort test runs that can continue if Apple notarization queue is delayed
3. Confirm logs show:
   - keychain setup success
   - `codesign --verify` success
   - notarization accepted
   - stapling success

Workflow structure:

- entrypoint: `.github/workflows/release-macos.yml`
- implementation: `.github/workflows/reusable-release-macos.yml`
- release workflow triggers only on manual dispatch or `v*` tags (not PR)

## 6) Create production release

Tag and push:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

Strict release option:

- use tag text that includes `strict` (for example `v1.2.3-strict`) to enforce signing + notarization prerequisites and fail if notarization does not complete

Workflow publishes:

- `Look-X.Y.Z-macOS.zip`
- `Look-X.Y.Z-manifest.txt`

## 7) Update Homebrew cask

1. Copy SHA256 from manifest.
2. Update `look` cask in tap repo.
3. Publish tap update.

## 8) Verify on clean machine

Install released app and validate:

```bash
spctl -a -vv "/Applications/Look.app"
codesign --verify --deep --strict --verbose=2 "/Applications/Look.app"
```

Expected: accepted/signed/notarized app launches without bypass flow.

## Troubleshooting quick hits

- cert missing private key: recreate/import cert into same Mac keychain
- `security find-identity -v -p codesigning` shows `0 valid identities`:
  - confirm cert is in `login` -> `My Certificates` with private key attached
  - set cert trust to `Use System Defaults` (avoid `Never Trust`)
  - confirm `Developer ID Certification Authority` + `Apple Root CA` are present
  - unlock keychain and recheck:

  ```bash
  security unlock-keychain ~/Library/Keychains/login.keychain-db
  security find-identity -v -p codesigning ~/Library/Keychains/login.keychain-db
  ```

  - if still zero, recreate `Developer ID Application` cert from Xcode and remove broken duplicates
- wrong identity string: re-run `security find-identity -v -p codesigning`
- notary fails auth: regenerate app-specific password
- existing users still blocked: verify they installed new notarized build, not old artifact

## Scope reminder

This guide is for direct distribution (GitHub Releases/Homebrew).

- current path: `Developer ID Application` + notarization
- Mac App Store later uses different cert/profile flow (Mac App Distribution + App Store Connect)
