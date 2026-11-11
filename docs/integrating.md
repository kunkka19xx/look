# Integrating your app with Look

This is the contract for connecting an app to Look. If you are adding rows for
yourself rather than shipping an app, you want
[Your own sources](user-sources.md) instead: everything there works without
writing a program.

A **connector** is a program Look starts once and leaves running. It sends rows
whenever it likes and they appear without a reload, and it fills the panel for
the rows it sent. Look never blocks on it: slow, silent or dead, your rows are
simply the last ones you sent.

Protocol version: **1**.

## Installing

Write a TOML manifest into `~/.look/connectors/`. Name the file after your app,
because every block in it is namespaced by the file name:
`myapp.toml` declaring `[records]` is the block `myapp.records`.

```toml
# ~/.look/connectors/myapp.toml
[records]
name    = "Records"
aliases = ["rec"]
icon    = "🗂️"
connect = "/Applications/MyApp.app/Contents/Helpers/look-connector"
```

Look reads the directory at launch and on a reload (`Cmd+Shift+;` on macOS,
`Ctrl+Shift+;` on Linux and Windows); on Linux
and Windows it also notices a manifest written, changed or deleted while it
runs, and reloads by itself. On macOS a manifest installed while Look runs
waits for the next reload or launch.

Uninstall by deleting the file. A user can also turn your manifest off from
`Settings > Extend`, which writes `enabled = false` into every block in it, so
reinstalling or rewriting the file is how that choice gets undone: read it
before you overwrite one. `LOOK_CONNECTORS_DIR` repoints the directory, if
your tests want one of their own. Everything else a block can declare is in
[Your own sources](user-sources.md#key-reference); `connect` is the only key
specific to this document.

A manifest may declare several blocks, and one program may serve all of them.

## Speaking

One JSON object per line, on stdin and stdout. Flush after every line: a
buffered frame is a frame that never arrives.

Write your own logging to stderr. Look reads it line by line and repeats each
line in its own log, prefixed with your block, so `starting up` becomes
`look connectors: [myapp.records] starting up`. Nothing parses it and nothing
waits on it, so it is yours to use. A line longer than 4 KB continues on the
next one.

An `error` frame lands in that same log, written the same way. Both also show
under your block in `Settings > Extend` (see
[Finding out what went wrong](#finding-out-what-went-wrong)), but only there:
nothing pops up in the launcher, so do not use one to tell somebody their
credentials expired. Send an `error` when you have no stderr to write to, or
when you would rather say it in the protocol than in a log line.

Look opens with a hello, and you answer when you have something to say:

```json
look → {"op":"hello","v":1,"look":"0.7.5"}
you  → {"op":"ready","capabilities":["rows","preview"]}
```

### Frames Look sends

| Frame | When |
| ----- | ---- |
| `{"op":"hello","v":1,"look":"0.7.5"}` | once, on start |
| `{"op":"preview","block":"myapp.records","id":"r1","req":7}` | a row of yours is selected |
| `{"op":"plan","block":"...","verb":"archive","ids":["r1"],"plan":7}` | a verb of yours was picked on those rows |
| `{"op":"run","block":"...","plan":7}` | the user confirmed that plan |
| `{"op":"state","block":"...","control":"lamp","req":9}` | where a control of yours stands |
| `{"op":"press","block":"...","control":"lamp","req":10}` | the user pressed it |
| `{"op":"bye"}` | Look is closing you down |

### Frames you send

| Frame | Meaning |
| ----- | ------- |
| `{"op":"ready","capabilities":[...]}` | you are up |
| `{"op":"rows","block":"myapp.records","rows":[...]}` | these are the block's rows now |
| `{"op":"preview","block":"...","id":"r1","req":7,"content_type":"markdown","text":"..."}` | the answer to a `preview` ask |
| `{"op":"planned","block":"...","plan":7,"title":"Archive 2 notes","detail":"a.md, b.md"}` | what the verb would do, which is the question the user reads |
| `{"op":"result","block":"...","plan":7,"ok":true,"summary":"Archived 2 notes"}` | how running it went |
| `{"op":"state","block":"...","control":"lamp","req":9,"on":true}` | the answer to either of those |
| `{"op":"error","message":"could not reach the server"}` | written to Look's stderr |

A row is `{"id": "...", "title": "...", "subtitle": "...", "path": "...", "icon": "..."}`,
where only `id` is required. These are the same fields a JSON row from a `run`
block uses, so a script and a connector describe a row alike.

`content_type` is `text` or `markdown`. Leave it out and it is `text`.

### Offering a verb

Declare it beside the rows it acts on, and list it in `then`:

```toml
[records]
connect = "..."
then    = ["archive"]

[archive]
name = "Archive"
ask  = "archive"        # the verb Look sends you
```

It appears in the action menu (`Cmd+K`) of that block's rows, and only those:
a verb reaches the connector that owns the row, so it cannot be declared with
`applies` against rows no connector produced. Picking it asks you to plan,
and your `title` and `detail` become the question. That is the point: a
declared `confirm` string cannot say how many rows were picked or name them.

The confirmation starts on **Yes**: the question already names what will
happen, and the user picked the verb. Say so in the `title` if the verb cannot
be taken back, since the words are what protect the user, not an extra press.

There is no undo. Look does not pretend it can reverse a command you ran on a
machine it does not own. Plan carefully instead, since the plan is what the
user agreed to.

### Offering a control

A block's rows can carry a switch or a button that you supply:

```toml
[records]
connect = "..."
control = { id = "sync", kind = "toggle", title = "Syncing", on = "On", off = "Off" }
```

`kind` is `toggle` or `button`. Look asks you for its state when a row is
selected, and tells you when it is pressed. **Answer a press with where the
control ended up**, not with what was asked: you may refuse, and a switch that
shows a state you never reached is worse than a slow one.

Look moves the switch the moment it is pressed and corrects it when you
answer, so a round trip does not read as a broken control.

On a row that has nothing else to do, `Enter` presses the control rather than
failing: the control is the row's purpose.

You may only supply a control Look does not already have. A name the platform
owns (`bluetooth`, `wifi`, `shutdown`) is refused and said so on stderr, so a
manifest cannot put the machine's Bluetooth under a row that looks like a note.

A user can also put your control on the home-screen strip, by naming it
`<block>#<control>` in their own `~/.look/super-actions.toml`. Nothing is
needed from you: declare it once and it works in both places. See
[Tiles of your own](user-guide.md#tiles-of-your-own).

## The rules

- **A `rows` frame nobody asked for is a push.** That is the whole liveness
  story, and it costs nothing extra. Send one whenever your data changes.
- **Send the same rows again as often as you like.** Look compares them with
  what it last wrote and stops there. For a full 2,000-row block that is about
  30us, measured in `docs/bench-notes/`. You do not need to track what you
  last sent.
- **Echo the number on every answer**, `req` on a preview and `plan` on a
  `planned` or `result`. It is how Look tells your answer from the one before
  it. An answer without it is dropped, and Look says so.
- **You may only speak for blocks in your own manifest.** One app cannot
  rewrite another's rows. Look drops frames that try, and says so.
- **An `op` Look does not know is ignored, never fatal.** Hold yourself to the
  same rule: a Look newer than your connector will send frames you have not
  heard of.
- **Nothing blocks on you.** The panel waits about 400ms for a preview and then
  moves on. A later answer is not shown, so answer from memory rather than
  going to the network.
- **If you exit, your rows stay** and Look starts you again the next time the
  launcher opens. Exit on start repeatedly and it backs off instead, so a
  connector that cannot run does not fork on every open.

## Finding out what went wrong

Settings, Extend, under your block. Look shows your last few complaints there,
and a block that is down says why on the same line. Everything it shows is
also on stderr, which is the better place to read it when you started Look
from a terminal, and the only place to read it before this existed.

What reaches it: anything your program writes to its own **stderr**, an
`{"op":"error","message":"..."}` frame, a spawn that failed, a frame over the
cap, a frame for a block outside your manifest, and an answer with no number.
So the first mistake everyone makes, forgetting to echo `req`, says so instead
of showing an empty panel.

## Limits

| | |
| --- | --- |
| One frame | 1 MiB. Past that you are a protocol breach rather than a big message, and Look drops you |
| Rows per block | 2,000. The rest are not stored |
| Preview text | 64 KiB, truncated on a character boundary |
| Preview answer | about 400ms before the panel gives up on it |
| Control state | about 400ms, since it is read on every selection |
| Control press | about 1.5s, since it happens once and the user is waiting |
| Plan answer | about 1.5s, since the user is waiting on a menu |
| Run answer | about 5s, after which Look assumes it is still going |

## What it costs

Mostly you, not Look. Keeping one connector running costs Look about 130 KB and
two threads, one reading your stdout and one your stderr. Your own program is
the real figure: a small compiled one is a fraction of a megabyte, a Python one
is 10 to 15 MB before it does anything.

Push when your data changes rather than on a timer, because every push makes
Look re-read the declared sources. It rate-limits that, but it cannot make it
free.

**Read your stdin, even if you have nothing to answer.** A connector that stops
reading fills its pipe, and what happens next depends on the frame. A preview
or a control state is dropped rather than queued, so those simply stop working
while you still look like you are running. A press or a verb is not dropped:
Look waits for the pipe, so one can reach you long after the caller gave up and
told the user nothing came back. Do not treat a late `press` or `run` as a
duplicate of something you already did.

## A worked example

[`core/engine/src/refcon.rs`](../core/engine/src/refcon.rs) is a
complete connector in about a hundred lines: it answers the hello, pushes rows,
answers previews with markdown and echoes `req`. It is also the fixture the
connector tests spawn, so it cannot drift from the behaviour Look expects.

Run it by hand to see the frames:

```bash
printf '{"op":"hello","v":1,"look":"0.7.5"}\n{"op":"preview","block":"myapp.notes","id":"n1","req":1}\n{"op":"bye"}\n' \
  | cargo run --manifest-path core/Cargo.toml --bin refcon -- myapp.notes
```

### Try it inside Look

Build it, then point a manifest at the binary:

```bash
cargo build --manifest-path core/Cargo.toml --bin refcon

cat > ~/.look/connectors/reference.toml <<EOF
[notes]
name    = "Reference rows"
aliases = ["ref"]
icon    = "🔌"
connect = '"$(pwd)/core/target/debug/refcon" reference.notes serve'
then    = ["archive"]

[archive]
name = "Archive"
ask  = "archive"
EOF
```

Open Look and type `ref`. Two rows arrive as a push, selecting one shows its
markdown preview, and `Cmd+K` then Archive asks the connector to plan: the
question names the row rather than reading "Archive?".

The path points into the build output, so `cargo clean` breaks it. Delete the
file to uninstall.

## Not yet

These are designed but not sent, so do not implement them:

- `{"op":"query","block":"...","q":"..."}`. Look filters your rows locally
  today. This arrives when a connector wants to answer per keystroke from its
  own index.
- `capabilities` on `ready` is read and ignored. Send it anyway, since it is
  how Look will decide what to ask you for.
- `tile`, for a live tile on the super-actions strip.

A connector written against this document keeps working when they land.
