# PR 0: Ollama provider (macOS) - checklist

> **Status: SHIPPED.** Also grown since: the Settings model field is a dropdown
> of installed models (`GET /api/tags`, debounced and order-safe), host+model
> share one row, and the AI toggle sits on the section header.
>
> Since superseded in one respect: the provider's own URLSession chat code is
> gone. Planning, session chat, and the answer card all ride the Rust core's
> curl transport (`core/ai/src/chat.rs`), so there is ONE Ollama client with one
> set of timeout/cancel/error semantics. What stays Swift-side here is health
> probing (`GET /api/tags`), the model list, and the warm-up.

Goal: add Ollama as a second `AIQueryProvider` so every existing AI feature
(query understanding, the answer card) can run on a capable local model. This is
the prerequisite for Act and semantic Recall (see `ai-vision.md`).

Scope: macOS only. Swift-side URLSession, self-contained like
`AppleIntelligenceProvider`. No Rust/FFI changes. No keychain (Ollama needs no
key). No new dangerous surface.

## Confirm before coding

- [ ] Target the Ollama REST API on `http://localhost:11434`
      (`GET /api/tags`, `POST /api/chat`). Confirm this is the intended surface.
- [ ] App min deployment target supports `URLSession.bytes(for:)` (macOS 12+),
      used for NDJSON streaming. Confirm the target.
- [ ] You have Ollama installed with a model pulled (e.g. `ollama pull llama3.1`)
      to verify against.
- [ ] Chosen default model string for the settings default (e.g. `llama3.1`).

## Code changes

### 1. Provider enum
`Models/ThemeSettings.swift`, `enum AIProviderKind` (~line 102):

- [ ] Add `case ollama`.
- [ ] Add its `title` case: `"Ollama (local)"`.

### 2. Settings model fields
`Models/ThemeSettings.swift`, `struct ThemeSettings` (near `aiProvider`, line 186):

- [ ] `var ollamaHost: String = "http://localhost:11434"`.
- [ ] `var ollamaModel: String = "llama3.1"` (or the confirmed default).

Both are new non-optional Codable properties, so the backfill path in
`ThemeStore` (JSON `object` merge, ~line 886) must supply defaults so old
UserDefaults blobs still decode. Verify.

### 3. Config persistence
`Support/ThemeStore.swift`:

- [ ] Persist: alongside the `ai_provider` upsert (~line 245), add
      `ollama_host` and `ollama_model` upserts.
- [ ] Parse: alongside the `ai_provider` case (~line 545), add `ollama_host` and
      `ollama_model` cases.
- [ ] Default template: alongside the `ai_provider=appleIntelligence` block
      (~line 880), add `ollama_host` and `ollama_model` commented defaults.

### 4. The provider
New file `Support/AI/OllamaProvider.swift`, conforming to `AIQueryProvider`:

- [ ] `id = AIProviderKind.ollama.rawValue`, `displayName = "Ollama (local)"`.
- [ ] Init takes host + model (read from settings at construction; see wiring
      note below).
- [ ] `availability`: `GET {host}/api/tags`. Reachable and the configured model
      present -> `.available`. Daemon down -> `.unavailable(.other("Ollama not
      running"))`. Model missing -> `.unavailable(.other("Model <name> not
      pulled"))`. (`AIProviderUnavailableReason` already has `.other(String)`.)
      Availability must be sync per the protocol, so cache the last known state
      from a short async probe; never block the UI thread on a network call.
- [ ] `understand(query:)`: `POST /api/chat`, non-streamed, with
      `format` set to a JSON schema matching the current `EngineQueryPlan`
      (kind + searchText). Decode into `AISearchIntent`. Return nil on any
      failure (AI is best-effort, never blocks search).
- [ ] `answer(query:)`: `POST /api/chat` with `stream: true`,
      `options.num_predict` capped (~220, matching the Apple provider's token
      cap). Stream via `URLSession.bytes(for:)`, parse NDJSON lines.
      **Gotcha:** Ollama streams *deltas* (`message.content` per chunk), but the
      protocol requires each yielded value to be the *cumulative* answer so far.
      Accumulate into a running string and yield the cumulative each line.
      Honor `Task.isCancelled` and `continuation.onTermination` (cancel the
      URLSession task), same shape as `AppleIntelligenceProvider.answer`.
- [ ] `prewarm()`: optional. A cheap `POST /api/chat` with `keep_alive` warms the
      model, or leave as the default no-op for v1.
- [ ] A short system prompt for `answer` mirroring the Apple provider's
      launcher-sized-answer instructions (plain text, 2-4 sentences).

### 5. Router registration
`Support/AI/AIQueryRouter.swift`, `makeProvider(for:)`:

- [ ] Add `case .ollama: return OllamaProvider(...)`. Feed it the current host +
      model. Note the router caches providers per kind; if host/model can change
      at runtime, either read settings inside the provider per-request or
      invalidate the cache on settings change. Simplest for v1: read host/model
      lazily inside the provider on each call so a settings edit takes effect
      without cache invalidation.

### 6. Settings UI
`Views/Settings/ThemeSettingsView+Advanced.swift`, AI block (~lines 44-65):

- [ ] Add a `Picker("AI provider", selection: $settings.aiProvider)` over
      `AIProviderKind.allCases`, below the existing AI toggle.
- [ ] When `.ollama` is selected, show host + model `TextField`s.
- [ ] The availability indicator (`aiInfoIndicator`, line 11) already keys off
      `settings.aiProvider` via `AIQueryRouter.shared.availability(of:)`, so it
      reflects Ollama status automatically once the provider exists.
- [ ] Generalize `aiAvailabilityTooltip` (lines 24-37): it is currently
      Apple-Intelligence-specific. Make the on-device line provider-aware
      (Apple Intelligence vs Ollama host/model status).

## Test plan (run tests)

- [ ] Unit: `understand` JSON decoding maps a sample `/api/chat` JSON response to
      the right `AISearchIntent` (kind + searchText). Test the decoder against a
      fixed JSON string, not a live daemon.
- [ ] Unit: the delta-to-cumulative accumulation for `answer` (feed a sequence of
      NDJSON delta lines, assert the yielded values are cumulative and the final
      equals the concatenation).
- [ ] Unit: `availability` state mapping from a fixed `/api/tags` payload
      (present model -> available; absent -> model-missing; put the HTTP behind a
      small injectable client so tests use a fake, no network).
- [ ] Manual smoke: with Ollama running, switch provider to Ollama in Settings,
      confirm the answer card streams and query understanding rewrites a query.
      With Ollama stopped, confirm availability shows unavailable and search is
      unaffected (AI self-skips).

## Definition of done

- [ ] Selecting Ollama in Settings routes understanding + answer through it.
- [ ] Ollama down or model missing degrades cleanly: search unaffected,
      availability indicator explains why.
- [ ] No network call on the keystroke hot path beyond what the existing AI flow
      already does (prewarm/debounce behavior unchanged).
- [ ] Tests green.

## Out of scope (later PRs)

- Cloud providers + keychain (PR after this).
- Embeddings / Recall (`/api/embeddings`).
- Action planning / the EventKit connector (needs this PR first).
- linows (Tauri) parity.
