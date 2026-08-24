# AI Vision

Status: direction, not a commitment to specific packages or timelines. This doc
sets the north star for AI in `look` and specifies the Recall pillar in enough
detail to build against.

**What of this has shipped** (see `ai-architecture.md` for the as-built detail):

| Verb | State |
| --- | --- |
| Find | Shipped. Deterministic file recall plus a model fallback that emits a structured query, never a rewritten string. |
| Answer | Shipped. Instant answers, web sources, and a streamed local-model answer; personal schedule questions answer from the calendar itself. |
| Act | Shipped for calendar + reminders (add/move/cancel/complete/remove/snooze/block), confirm-gated with undo, reachable from both `>` and the main bar. |
| Recall | **Partly built.** The clipboard corpus is persisted (step 1 below); there are no embeddings yet, so today's "recall" is deterministic keyword+time search, useful but not semantic. |

## The wedge

`look` sits on a combination the cloud AI assistants cannot copy:

- a local index of your machine (apps, files, folders, recents, todos, clipboard),
- the ability to run locally (Apple Intelligence today, Ollama next), so data
  never has to leave the device,
- the ability to act on the system (kill processes, toggle wifi/bluetooth,
  switch apps),
- a keyboard-native, instant surface with no window and no context switch.

Raycast AI and ChatGPT are cloud, so they will never read your whole clipboard
history or file tree privately. Spotlight + Apple Intelligence can see files but
is not keyboard-programmable, not cross-platform, and does not perform multi-step
actions. `look` owns the intersection: a private, local-first, cross-platform,
keyboard-driven layer that finds, answers, acts, and recalls.

## Identity: launcher-plus

`look` stays a launcher. AI does not become a new mode or a chat window. It adds
new verbs to the same box, next to `a"`, `f"`, `c"`, `t"`, and `kill :3000`.

Four verbs:

- **find** - today's default, unchanged.
- **answer** - short factual answers (already shipped via the answer card).
- **recall** - ask your own machine (files, clipboard, todos). See below.
- **act** - describe a task, get a previewed plan, confirm with Enter.

**The `>` AI session (shipped)** is where act and conversational answers live:
an explicit prefix, so the model never runs on ordinary searches, opening a
session screen that owns the panel area (like command mode does). Actions,
questions, and streamed answers stack in one ephemeral session; Esc ends it
(archived locally), hide/recall suspends it. Not a chat app: the box stays the
only input and history is session-scoped. As-built spec: `ai-architecture.md` §7.

A user who never types the new verbs has exactly the launcher they have today.
AI is additive power, invisible until asked for.

## Non-negotiables

These are constraints, not preferences. Breaking one breaks the product.

1. **Local-first is the brand.** On-device and Ollama are the default. Cloud is
   opt-in, keyed, and always shows "this leaves your machine." API keys live in
   the OS keychain, never in `~/.look/config`.
2. **Chat is a fallback, never the surface.** The moment the main interaction is
   a conversation window, `look` becomes a worse ChatGPT.
3. **Acting is guarded.** Every action gets preview, confirm, and undo. The model
   composes only audited primitives, never raw shell by default.
4. **Small and native.** No Electron, no daemon, one binary per platform. If AI
   bloats the app, we lost the line that beats Raycast in our own table.
5. **Stays out of your way.** No proactive nagging. Silent until summoned.

## The acting model: agentic composition over a curated vocabulary

The model is free to plan and chain steps (agentic feel). It may only compose
from a hand-audited set of primitives (curated safety). The model never emits
`rm`; it emits a plan such as `[move(a,b), rename(x,y), convert(p,q)]` built from
primitives we shipped and reviewed.

Safety spine, inherited by every primitive:

- **Plan, not commands.** Structured steps over a whitelist. Raw shell is a
  separate, off-by-default capability behind its own gate.
- **Preview before Enter.** Show the literal operations (these 12 files, these
  old-to-new names).
- **Undo by journaling.** Delete means Trash, not unlink. One command reverses
  the last plan.
- **Destructive requires confirm**, exactly like kill-by-port does today.

Adding primitives over time makes the agent more capable without ever making it
less safe.

### Two levers make this robust regardless of model quality

Both proved out in the shipped planner, and they are why a 4B model scores 97%
on the eval (`ai-architecture.md` §9):

- Push work into deterministic code. The model only classifies, picks a
  primitive, and extracts slots; code does date resolution, matching,
  validation, execution, and undo.
- Let the model pass values through verbatim (e.g. time phrases like "tuesday
  10am") and resolve them in code (NSDataDetector), so the model never does date
  math, which is where weak models fail.

## The spine

All verbs ride one architecture, so this stays one product and not a pile of
features.

- **Intent router.** Text in, classify into find / answer / act / recall,
  dispatch. Shipped as `core/ai/src/route.rs`, one ladder in one place.
- **Provider layer.** `AIQueryProvider` protocol, `AIProviderKind` enum. Ollama
  and cloud providers plug in with no other changes. On-device by default.
- **Context provider.** Frontmost app, current selection, clipboard, recents.
  Not built; today's context is the calendar, the clipboard, and remembered
  facts.
- **Local memory.** Embeddings over clipboard, files, and todos. Powers Recall.
  Not built.
- **Tool layer.** Audited action primitives on the safety spine above, with one
  generic confirm/preview/undo surface. Act is NOT a command-mode palette: the
  main box plus the router plus a table of tools is what scales to many
  connectors. Adding a tool is a row in `core/ai`, not a new switch or new UI.
  See `ai-architecture.md`.

## Provider strategy and the setup-cost tradeoff

Different features need different model strength, and the strong options carry a
setup cost. This is a real product decision, not just plumbing, so it is stated
here explicitly.

The three tiers:

- **Apple Intelligence (on-device, ~3B).** Zero setup, fully private, no network.
  Strong enough for find/answer, too weak for act. Availability-gated (macOS 26+,
  supported hardware). The "works out of the box" tier, but capped.
- **Ollama (local, 7-8B+).** Private, no network egress, free to run. Strong
  enough for planning and recall synthesis. Cost: the user installs Ollama and
  pulls a multi-GB model. The local-first power tier.
- **Cloud (bring-your-own key, e.g. Claude Haiku).** Strongest and zero local
  setup. Cost: queries leave the device (opt-in, disclosed, keychain-stored key)
  and the user pays per token. The zero-friction power tier.

Enforced, not just intended: private context (calendar, clipboard, remembered
facts) is withheld from any provider that is not on this machine unless the user
turns it on in Settings. Providers must declare themselves local, and the
default is remote, so a cloud provider added later cannot silently inherit
personal data.

The honest tradeoff: **look's best AI features require the user to bring a
capable model.** The zero-setup, fully-private option (Apple Intelligence) is
also the weakest and cannot do acting. So look moves from "works out of the box"
toward "works great once you connect a brain." For a power-user, keyboard-first
tool this is an acceptable ask, but it must be surfaced honestly, never as a
silent degradation.

How this shows up to the user (graceful degradation by capability):

- **Nothing configured:** find works fully. Answer works if Apple Intelligence is
  available. Recall degrades to keyword search. Act is hidden, with a one-line
  "connect a model to enable actions" hint rather than a broken feature.
- **Ollama or cloud configured:** the full set (act, semantic recall, stronger
  understanding) unlocks.

Defaults and posture:

- Default to the most private capable option present: Apple Intelligence for
  find/answer, Ollama for act/recall when installed.
- Cloud is always opt-in and always shows "this leaves your machine." Keys live
  in the OS keychain, never in `~/.look/config`.
- Never auto-select a cloud provider or send a query off-device without an
  explicit, remembered choice.
- One clear Settings surface: which provider powers which verb, current
  availability, and the egress disclosure per provider.

## What is left

The provider layer and Act both shipped. In order:

1. **Recall v1** over clipboard history (this doc). Smallest, safest, most
   clearly-local, most differentiated. Ranking is embeddings, not a chat model.
2. Recall extends to files and todos.
3. A cloud provider (keychain-backed key, egress disclosure).
4. Context injection (frontmost app + selection feed the same box).
5. Grow the primitive vocabulary.

---

# Pillar deep dive: Recall

"Ask your own machine." Query your files, clipboard, and todos in natural
language: "what was that API key I copied last week", "which doc had the Q3
numbers", "what did I work on Tuesday".

Recall is the wedge. It is inherently local: nobody uploads their entire
clipboard history to a cloud, which is exactly why cloud assistants cannot offer
it. `look` already holds the data.

## Why clipboard first

- Smallest corpus (hundreds to low thousands of entries). Brute-force vector
  search is fine, no vector-index dependency required to start.
- Already a shipped feature (`c"` search), so users understand it.
- The most obviously-private data, which makes the local-first story land.
- Highest "oh, that's different" payoff per unit of work.

## Current state (what we build on)

- **Step 1 shipped.** Clipboard history is persisted in `look.db`
  (`clipboard_entries` in `core/storage`), owned by the Rust core so both
  shells share it. `content_hash` is UNIQUE, so re-copying the same text moves
  the row to the top instead of piling up. Concealed and transient clips are
  never written, and that gate sits at the capture site because only the shell
  can see the pasteboard's type markers.
- The DB still has no vector or FTS table.
- So what remains of Recall v1 is: embed, retrieve.

## Design

### 1. Embeddings

For each stored entry, compute and store an embedding vector.

- **Model:** local, via Ollama `/api/embeddings` (candidate: `nomic-embed-text`).
  To confirm before implementing per repo rules. Apple Intelligence does not
  currently expose an embeddings API, so embeddings are an Ollama-gated feature;
  without a local embed model, Recall degrades to keyword search (see below).
- **Storage:** an `embedding BLOB` column alongside the entry, plus the model id
  and dim so we can invalidate on model change.
- **Vector search:** start with **brute-force cosine in Rust** over the stored
  vectors. At clipboard scale this is sub-millisecond and adds zero
  dependencies. Only evaluate a vector index (e.g. sqlite-vec) if and when the
  corpus grows to files/todos and brute force stops being cheap. Do not add a
  vector-store dependency for v1.

### 2. Retrieval and answer

Query flow for a `?`-prefixed (or clearly question-like) recall query:

1. Embed the query with the same local model.
2. Cosine over stored vectors, take top-k.
3. Rank by a blend of similarity and recency (recency matters a lot for
   clipboard: "last week" is a real signal).
4. Render the top hits as normal launcher rows (copy again / open / reveal).
5. Optional synthesis: feed the top-k to the chat model to answer in one line
   ("You copied it from 1Password on Tuesday: sk-..."). Synthesis is additive and
   must never block the raw hits from showing.

### 3. Grammar

- `?query` enters recall explicitly.
- A question-like query with zero local find results can route to recall
  automatically (mirrors today's orphan-entity answer-card trigger).
- Recall over clipboard is a natural superset of today's `c"`; `c"` stays as the
  fast keyword path.

### 4. Graceful degradation

Recall must work, in reduced form, with no AI at all:

- No embed model available: fall back to FTS/substring over `content`. This is
  essentially today's `c"` search, just reachable via the recall grammar.
- Embed model present: semantic ranking as above.

This keeps Recall from being a hard dependency on Ollama and keeps the
local-first promise intact for users who run nothing extra.

## Privacy model (Recall)

- All data and all computation stay on-device. Recall never calls a cloud
  provider, even when one is configured for answering. Embeddings and search are
  local-only.
- Concealed/transient clipboard types are never stored.
- Clearable in one command (`look_clipboard_clear`) and capacity-bound
  (`MAX_CLIPBOARD_ROWS`, 5000). There is no age cap and no opt-out toggle
  today; persistence is on, which is worth revisiting before embeddings make
  the corpus searchable by meaning rather than by substring.
- Source app is recorded to enable "exclude this app from clipboard memory".

## Open decisions (confirm before building)

1. Ollama embeddings model choice and dim (`nomic-embed-text` vs alternatives).
2. Whether synthesis (LLM one-line answer over hits) ships in v1 or after raw
   ranked hits prove out.
3. Vector search: confirm brute-force-cosine-first, defer any vector-store crate.
4. macOS-only, like every other AI feature. linows AI is not being built;
   users who want it should open an issue. Note embeddings need a local model,
   so the hardware caveat applies there too.

## Recall phased plan

1. ~~Persist clipboard to `look.db`.~~ **Shipped.**
2. Add local embeddings (Ollama) + brute-force cosine + recency blend. Render
   ranked hits. No synthesis.
3. Add optional one-line synthesis over top-k.
4. Extend the same pipeline to files and todos.

---

# Connectors: acting over the OS's own apps

Calendar is the first instance of a general pattern: `look` reads and acts on
structured personal data through the OS's own system API, driven by natural
language, staying local. It does not reimplement the app; it drives the store
the built-in app already uses.

## The pattern

- **read** (answer / recall) + **act** (curated primitives) over a system data
  store.
- The system API is local and permission-gated. `look` makes no network calls.
- Whatever sync the user already set up (iCloud, Google, Exchange) keeps working,
  run by the OS, not by `look`. No new egress.

## EventKit connector (Calendar + Reminders), macOS first

What "integrate with the default Calendar app" means in practice: EventKit is the
API into the same system calendar store that Apple's Calendar app reads and
writes. There is one shared store; Calendar.app is a UI on top of it, and
EventKit is a second door into it. So a `look` write:

- appears in Calendar.app immediately (same database),
- includes every account the user already added (iCloud, Google, Exchange, or a
  local "On My Mac" calendar),
- syncs through the OS exactly as it already does. `look` does nothing special.

**Local-first is preserved.** `look` only touches the local EventKit store and
makes zero network calls. If the calendar is a Google account, that sync is the
OS's job and was already happening. The honest one-liner: "look writes to your
system calendar; whatever syncing you set up keeps working; look itself never
touches the network."

Cost: a one-time EventKit full-access permission grant, same as Calendar.app
asks for. The same framework and permission also cover **Reminders**, which ties
into the existing `:todo`.

Shipped: add, move, cancel, and block for events; add, complete, snooze, and
remove for reminders. Each previewed, confirmed, and undoable. Per-tool
contracts and the permission model: `ai-eventkit.md`.

Platform matrix (the honest, uneven part):

- **macOS**: EventKit. Local, unified, reads/writes the real store including all
  synced accounts. First-class.
- **Windows**: WinRT `AppointmentStore` exists but is limited (oriented toward
  add-via-system-UI, restricted programmatic read/write of arbitrary account
  calendars). Doable but weaker.
- **Linux**: no unified system calendar. GNOME via Evolution Data Server over
  D-Bus, KDE via Akonadi. Desktop-dependent and fragmented.

Current position: **macOS-only**, and a Windows/Linux calendar backend is not
being built. Not refused either - if a linows user asks for it, that is an
issue worth having. But the matrix above is the reason to be honest in that
conversation rather than optimistic: on Linux there is no unified store to
write to at all, so "calendar acting on linows" may be a thinner feature than
the macOS one no matter how much work goes in.

## Other apps that fit the same pattern

Ranked by value x local-API-cleanliness x cross-platform. Build these only after
EventKit proves the connector pattern.

**Tier 1 (clean local system API, high value):**

- **Contacts** (Contacts framework, `CNContactStore`): "sarah's number", "add
  contact", enriches calendar ("meeting with sarah" resolves the attendee).
  Local, permission-gated. Clean.
- **Photos** (PhotoKit, `PHPhotoLibrary`): semantic recall over the photo
  library ("receipts from June", "photos of whiteboards"). Local,
  permission-gated. Highly differentiated, pairs with the Recall pillar.
- **Shortcuts** (the multiplier, see below).

**Tier 2 (valuable but fragile API, treat with care):**

- **Mail / Notes / Messages**: no clean local framework. Access is AppleScript
  or private SQLite stores, brittle and sandbox/permission-painful. High value,
  low reliability. Defer.
- **Music** (MusicKit / MediaPlayer): pairs with existing playback features
  ("play my focus playlist"). Apple Music parts may touch the network.
- **Browser bookmarks / history**: extends the existing `url_history`.

**Tier 3 (skip for now):** per-account cloud services with no clean local API,
and anything cross-platform-blocked.

## The multiplier: Shortcuts / OS automation

Instead of building a dozen bespoke connectors, integrate once with the OS
automation layer. macOS **Shortcuts** already aggregates every app's actions and
is user-extensible, so a single integration gives `look` a natural-language
front-end to everything the user has already automated. Run a Shortcut by name
(via the `shortcuts` CLI or URL scheme) from a plain query.

This is the breadth strategy; EventKit is the depth strategy. Likely both:
EventKit deep and native, Shortcuts broad and cheap.

Caveat: a Shortcut can do anything, so this is a raw-power capability. Gate it
like raw shell: preview what will run, confirm before running, off by default.

Windows and Linux analogues exist but are weaker: Windows has Power Automate /
PowerShell; Linux automation is desktop-dependent. macOS-first again.

## Open decisions (connectors)

1. ~~EventKit macOS-first vs parity.~~ **Decided: macOS-only.** The platform
   gap is accepted rather than deferred. Revisit only on real demand (an
   issue), and expect a thinner feature there, not parity.
2. Breadth vs depth: Shortcuts (one integration, huge reach, coarse control) vs
   bespoke connectors (native feel, per-app work). Likely both.
3. Which Tier-1 connector follows EventKit: Contacts (everyday utility) or Photos
   (differentiated recall).
