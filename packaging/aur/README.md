# AUR release - `look-bin`

End-to-end guide for publishing `look` to the Arch User Repository as `look-bin`. The PKGBUILD consumes the `.deb` produced by `release-linux.yml` - no source rebuild on the user's machine.

## Architecture

```
git tag v0.5.1 ──► release-linux.yml ──► build .deb ──► GitHub Release
                                                ▲              │
                                                │              ▼
                                                │     publish-aur job
                                                │              │
                                                │     download .deb, sha256
                                                │     template PKGBUILD
                                                │     git push ──► AUR
                                                │                   │
                                                │                   ▼
                                                └── user runs `yay -S look-bin`
                                                    yay pulls .deb from GH Release,
                                                    extracts, installs.
```

The CI workflow (`publish-aur` job in `.github/workflows/release-linux.yml`) handles releases automatically. Everything below is **one-time setup** that an operator does once per project lifetime - except the release flow at the end which fires on every `v*` tag.

---

## One-time setup

### 1. AUR account

- Register at https://aur.archlinux.org/register
- Verify your email
- Fill the anti-bot answer with:
  ```bash
  docker run --rm --platform=linux/amd64 archlinux:latest bash -c \
    "LC_ALL=C pacman -V | sed -r 's#[0-9]+#73c#g' | md5sum | cut -c1-6"
  ```
- Skip optional fields (IRC, PGP, backup email if you don't want recovery)

### 2. Dedicated SSH key for AUR

A separate key (not your GitHub key) keeps the secret scoped - if it leaks, only AUR is affected.

```bash
ssh-keygen -t ed25519 -f ~/.ssh/aur_look -C "aur@look" -N ""
```

- `-N ""` → empty passphrase (required for CI to use it non-interactively; the private key is protected by GitHub's secret storage instead).

Paste the **public** key into AUR → Settings → SSH Public Key:

```bash
cat ~/.ssh/aur_look.pub
```

### 3. SSH config

Tell ssh to use this key when talking to AUR:

```bash
printf '\nHost aur.archlinux.org\n  IdentityFile ~/.ssh/aur_look\n  User aur\n' >> ~/.ssh/config
```

### 4. Claim the package name (seed push)

AUR registers a package name only when a valid `PKGBUILD` + `.SRCINFO` is pushed. Until then, anyone could claim `look-bin`.

```bash
cd ~/Documents/git    # or wherever you keep repos
git clone ssh://aur@aur.archlinux.org/look-bin.git
cd look-bin
./seed.sh             # this script is committed in look-bin and pulls the template from ../look
```

`seed.sh` does:

1. Copies `packaging/aur/PKGBUILD` from the `look` repo.
2. Substitutes `__VERSION__` → `0.0.0`, `__SHA256__` → `SKIP`.
3. Generates `.SRCINFO` via an Arch container (with a non-root user since `makepkg` refuses root).
4. Commits and pushes to AUR.

After it succeeds, the package is live at https://aur.archlinux.org/packages/look-bin (a fetch at version `0.0.0` will 404 the .deb - that's expected and gets fixed on the first real release).

### 5. GitHub repository secrets

In the `look` repo → Settings → Secrets and variables → Actions → New repository secret:

| Name                   | Value                                         |
| ---------------------- | --------------------------------------------- |
| `AUR_SSH_PRIVATE_KEY`  | Contents of `~/.ssh/aur_look` (private half)  |
| `AUR_USERNAME`         | Your AUR username (e.g. `kunkka19xx`)         |

Paste the **entire** private key, including the `-----BEGIN OPENSSH PRIVATE KEY-----` and `-----END OPENSSH PRIVATE KEY-----` lines.

That's the end of one-time setup.

---

## Release flow (every tag)

1. Bump version in `apps/linows/src-tauri/tauri.conf.json`.
2. Commit, then tag and push:
   ```bash
   git tag v0.5.1
   git push origin v0.5.1
   ```
3. `release-linux.yml` runs:
   - `build-release` builds `.deb` + `.AppImage` and attaches them to the GitHub Release.
   - `nix-build` updates Cachix.
   - `publish-aur` downloads the new `.deb`, computes its SHA256, templates the PKGBUILD, regenerates `.SRCINFO`, and pushes to the AUR repo.
4. Within a few minutes, `yay -S look-bin` on any Arch box installs the new version.

No manual step required after the tag push.

---

## Manual update (only if CI is broken)

If you ever need to push to the AUR by hand:

```bash
cd ~/Documents/git/look-bin
git pull

# Template a fresh PKGBUILD
VERSION=0.5.1
SHA256=$(curl -fsSL "https://github.com/kunkka19xx/look/releases/download/v${VERSION}/Look_${VERSION}_amd64.deb" | sha256sum | awk '{print $1}')
sed -e "s/__VERSION__/${VERSION}/" -e "s/__SHA256__/${SHA256}/" \
    ../look/packaging/aur/PKGBUILD > PKGBUILD

# Regenerate .SRCINFO
docker run --rm --platform=linux/amd64 -v "$PWD":/pkg -w /pkg archlinux:latest bash -c '
  useradd -m b && chown -R b /pkg && su b -c "makepkg --printsrcinfo"
' > .SRCINFO

git add PKGBUILD .SRCINFO
git commit -m "Update to ${VERSION}"
git push
```

---

## Troubleshooting

**`Permission denied (publickey)` on git clone/push**
SSH key isn't registered correctly on AUR, or the `IdentityFile` path in `~/.ssh/config` is wrong. Verify with `ssh -T aur@aur.archlinux.org` - should print `Hi <username>! You've successfully authenticated...`.

**`fatal: pathspec '.SRCINFO' did not match any files`**
The Docker container failed silently. Re-run with the `useradd` line - `makepkg` refuses to run as root.

**CI `publish-aur` job fails with `error: vendor hash mismatch` or similar**
That's the Nix `nix-build` job, not AUR. Update `cargoHash` in `apps/linows/nix/package.nix` and retag.

**CI fails with `Could not resolve host: github.com` or `curl: (22) ... 404`**
The `.deb` filename pattern doesn't match. Inspect the latest GitHub Release; if Tauri changed naming (e.g. uppercase vs lowercase, `_amd64` vs `_x86_64`), update the `source=` line in `packaging/aur/PKGBUILD` and the curl URL in `.github/workflows/release-linux.yml`'s `publish-aur` job.

**AUR shows the new version but `yay -S look-bin` says "package not found"**
User's AUR helper cache is stale. They run `yay -Syu` or `paru -Syu` to refresh.

**Need to delete the package entirely**
AUR → package page → "Disown" or, as the maintainer, "Request → Deletion". You can't delete the git repo directly.

---

## Files in this directory

- `PKGBUILD` - template consumed by CI. Placeholders: `__VERSION__`, `__SHA256__`.
- `README.md` - this file.

The companion `seed.sh` script lives in the `look-bin` AUR clone, not here, since it depends on having the AUR git checkout next door.
