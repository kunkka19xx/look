# Plan: Open URL-like input (shared core)

Spec: `specs/open-url-input.md` (GitHub issue #232, history deferred).

Detection lives once in Rust (`look_answers`); macOS and linows each render a row.

## Shared core (single source of truth)

**1. `core/answers/src/url.rs`** (new module) — pure classifier
`pub fn classify_url(query: &str) -> Option<UrlMatch>` where
`UrlMatch { url: String, tier: UrlTier }`, `UrlTier { Structural, BareHost }`.
- Trim; reject interior whitespace, empty, and leading launcher prefixes.
- Tier 1 Structural: `http(s)://`; `host/path`; `host:port`; `localhost`/IP
  (`http://` for localhost/IP, else `https://`).
- Tier 2 BareHost: `host.tld` only if `tld` in curated gTLD allowlist
  (`com org net dev app io ai co xyz` ...); reject ccTLD/extension collisions
  (`rs py sh md ml pl` ...); no `/`, no port.
- Return `None` if the assembled string is not a valid URL.
- `#[cfg(test)]` table test: assert tier + url for `github.com`,
  `github.com/pulls`, `localhost:3000`, `https://x.com`; `None` for `main.rs`,
  `readme.md`, `look up docs`, `ratio 3:2`, `github.`.

**2. `core/answers/src/lib.rs`** — `mod url;` and re-export `classify_url`,
`UrlMatch`, `UrlTier`.

## macOS

**3. `bridge/ffi/src/answers_api.rs` + `lib.rs`** — add
`look_classify_url_json(query) -> *mut c_char` returning `UrlMatch` JSON or
`null`, mirroring `look_web_suggestions_json_impl`.

**4. `EngineBridge.swift`** — `@_silgen_name` decl + `classifyURL(query:) ->
UrlMatch?` decoding the JSON.

**5. `AppConstants.swift`** — `Launcher.WebURL`: `resultIDPrefix = "weburl:"`,
`resultID(url:)` / `url(fromResultID:)`.

**6. `LauncherView.swift`** — computed `urlResult` from the bridge; in
`displayedResults` (line 373) prepend when `.structural`, append after backend
results when `.bareHost` (never default while local results exist).

**7. `LauncherView+Results.swift`** — in `openSelectedApp()` before the `kind`
switch (~line 28) decode `WebURL.url(fromResultID:)`, call `openURLScheme(...)`
+ `hideLauncherWindow(restorePreviousApp: false)`, return.

## linows

**8. `apps/linows/src-tauri/src/answers.rs`** — Tauri command `classify_url`
over `look_answers::classify_url` (mirror `web_suggestions`); register in
`main.rs` invoke_handler.

**9. linows frontend (`apps/linows/src/`)** — call `classify_url`, render the
row, apply the same tier ranking, open via the existing shell-open path.

No refactors; existing search / Enter paths untouched on both platforms.
