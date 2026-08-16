# Four takeaways from Substage - plan

> **Status: PLANNED.** Nothing built. Written 2026-08-16 after looking at
> [Substage](https://selkie.design/substage/), a natural-language command bar
> for Finder selections.

Substage overlaps Look on purpose-built ground: instant actions that bypass the
model for common cases, and safety over ambition. Those parts confirm the
existing doctrine rather than teaching it. Four things it does that Look does
not, ordered by value.

**One thing Look must not copy**, stated once here because it shapes all four:
Substage has a model WRITE a shell command that the user audits. Look's
contract (`ai-action-contracts.md`) is the opposite - the model is another
parser that "can never reach an execution path the deterministic parser can't".
Auditing generated shell asks the user to review a language they may not read.
Every item below keeps generation out of the execution path.

---

## 1. Predict what `/shell` will do, before it does it

**The gap.** `/shell` is the one thing Look does that mutates anything, reaches
the network, and asks nobody. Everything else destructive already previews and
confirms (calendar tools, Empty Trash, file delete).
`LauncherView+CommandMode.runCommandModeAction` runs it on Enter with a single
cue: a warning when the input contains `sudo`. That is a string match, not an
understanding, and it says nothing about `rm -rf`, `curl | sh`, `> file`, or
`git push`.

**What to take.** Substage's headline is a prediction that CATEGORISES an
operation: what will be "created, changed, moved, deleted, or sent over the
network". The network category is the one Look is uniquely entitled to, because
"no network calls" is a promise Look already makes everywhere else.

**Design.** A `core/shell` crate: text in, ordered effects out. No model, no
execution, fully testable.

```rust
pub struct ShellPrediction {
    pub effects: Vec<Effect>,   // ordered by consequence, worst first
    pub unparsed: bool,         // saw something it could not classify
}

pub enum Effect {
    Creates { path: String },      //  >file, tee, mkdir, touch
    Changes { path: String },      //  >>file, sed -i, chmod, mv target
    Deletes { path: String },      //  rm, rmdir, trash, mv source
    Network { host: String },      //  curl, wget, ssh, scp, git push/pull
    Elevates,                      //  sudo, doas
    PipesToShell,                  //  curl ... | sh - the one that matters most
    RunsUnknown { binary: String },
}
```

A lexer for words, quotes, pipes, redirects and `&&`/`;`, then a table keyed on
each segment's leading binary. Not a parser for all of POSIX sh.

Two rules that carry the whole feature:

- **`unparsed` must be honest.** A prediction that silently under-reports is
  worse than none, because it launders the command as safe.
- **Order by consequence, not position.** `PipesToShell` and `Elevates` first,
  then `Deletes`, then `Network`. A user scanning one line meets the worst thing
  first.

```
Will run:  curl -fsSL https://get.example.sh | sh
  network  get.example.sh
  runs     whatever that host returns
```

Enter still runs it. The preview informs; it never blocks.

**Steps.** (1) `core/shell` with a fixture corpus asserting effects AND
`unparsed` - a convert, an `rm -rf` with a variable, a piped installer, a `git
push`, a heredoc. (2) `look_shell_predict_json`. (3) The preview block in the
`/shell` panel. (4) `features.md`, `user-guide.md`, and a line in
`ai-action-contracts.md` saying where this sits (it is not a ladder tier -
`/shell` is command mode, not AI mode).

**Out of scope.** Generating commands from language; blocking or sandboxing;
pretending to know what `./deploy.sh` does - name the binary, say the effects
are unknown, stop.

---

## 2. Take the target from the OS, not from the query

**The gap.** Substage acts on whatever is selected in Finder. Look's text-ops
need `Cmd+P` picks or an `@`-mention first, so "summarize this" with a file
already selected in Finder does nothing until the user re-selects it inside
Look.

**Design.** A new rung on the ladder `TextOpSource.resolve` already implements,
below the explicit ones so nothing changes for anyone using them:

```
@-mention  >  Cmd+P picks  >  frontmost Finder selection  >  clipboard
```

Read via Apple events (`tell application "Finder" to get selection`), which
needs Automation - the same grant Empty Trash already asks for, and one more
entry in `PermissionItem.all`.

**Two rules.**

- **Read on demand, never poll.** Look asks Finder what is selected at the
  moment a text-op needs a target. A background watcher of what the user has
  selected is surveillance, not a feature.
- **Say where the target came from.** The bar shows `from Finder: report.pdf`,
  because an implicit target the user cannot see is worse than no target: it
  transforms the wrong file silently.

**Steps.** `FinderSelectionService` (Swift, Apple events); extend
`TextOpSource.resolve` with the new rung and its tests; show the source in the
attachment bar; Automation chip in the permissions row; docs.

**Out of scope.** Any window other than the frontmost Finder window; watching
selection changes; using it for anything but text-ops and file ops.

---

## 3. Replay a command onto whatever is picked now

**Mostly already true.** Substage's up arrow replays a command against
different files. Look resolves the target at SUBMIT, not at recall:
`ActionController.textOpSource()` runs inside the route dispatch, so `Opt+Up` to
"summarize", then picking another file, already transforms the new one. The
`@`-token is consumed out of the recalled text (`MentionQuery.consume`), so
history holds clean prose with no stale filename in it.

**What is actually missing: visibility.** Nothing tells the user what a recalled
command is about to act on until after it runs. The whole of this item is one
line in the composer:

```
summarize                    → report.pdf   (picked)
```

Resolve `TextOpSource` as the input changes and render its target, so the answer
to "which file will this hit" is on screen before Enter, not after.

**Steps.** Expose the resolved source as a published value; render it beside the
input; nothing else. If item 2 ships, this is also what makes an implicit Finder
target safe.

---

## 4. Rules: teach it your shorthand

**The gap.** Substage lets a user teach it "shorthand, folders, formats, and
conventions you use every day". Look's `memory` stores durable FACTS for chat
context, but nothing the deterministic tiers read. So "my exports folder" means
nothing to the file tier, and every user's vocabulary is the one the lexicon
shipped with.

**Design.** Typed rules rather than free text, stored the way memory already is
- tier-1 only, user-written, **never model-written**, for the reason
`ai-action-contracts.md` already gives about memory: a weak planner must not be
able to pollute durable state.

```
remember exports means ~/Work/exports
remember convert means 1080p mp4
```

Expansion runs in core BEFORE the tier-1 grammars parse, so every tier inherits
it at once: file recall gets a location, text-ops get a format, `call` gets a
nickname.

**The rule that keeps it safe.** Aliases expand only where a slot is EXPECTED,
never globally. Otherwise a user with a folder called "mom" turns `call mom`
into a file search. Slot-scoped expansion is the difference between a shorthand
system and a booby trap.

**Steps.** Rules store in core beside `memory`; an expansion pass in `route.rs`
with tests for the shadowing case; FFI; a Settings list to see and remove rules
(a rule you cannot find is a rule you cannot fix); docs.

**Out of scope.** Model-written rules; rules that expand to commands rather than
values - that is generation again, and item 1's reasoning applies.

---

## Suggested order

1. **`/shell` prediction** - the biggest safety gap in Look today, and a
   differentiator Look has already earned by making the no-network promise.
2. **Target visibility** (item 3) - one line of UI, and a prerequisite for
   making item 2 safe.
3. **Finder selection** - removes a whole step from text-ops; costs a grant.
4. **Rules** - the largest surface, and the one whose value depends most on
   users actually having conventions worth teaching.
