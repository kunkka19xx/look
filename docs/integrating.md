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

Uninstall by deleting the file. Everything else a block can declare is in
[Your own sources](user-sources.md#the-keys); `connect` is the only key
specific to this document.

A manifest may declare several blocks, and one program may serve all of them.

## Speaking

One JSON object per line, on stdin and stdout. Flush after every line: a
buffered frame is a frame that never arrives. Write your own logging to stderr,
which Look passes through to its own.

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
| `{"op":"bye"}` | Look is closing you down |

### Frames you send

| Frame | Meaning |
| ----- | ------- |
| `{"op":"ready","capabilities":[...]}` | you are up |
| `{"op":"rows","block":"myapp.records","rows":[...]}` | these are the block's rows now |
| `{"op":"preview","block":"...","id":"r1","req":7,"content_type":"markdown","text":"..."}` | the answer to a `preview` ask |
| `{"op":"planned","block":"...","plan":7,"title":"Archive 2 notes","detail":"a.md, b.md"}` | what the verb would do, which is the question the user reads |
| `{"op":"result","block":"...","plan":7,"ok":true,"summary":"Archived 2 notes"}` | how running it went |
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

## The rules

- **A `rows` frame nobody asked for is a push.** That is the whole liveness
  story, and it costs nothing extra. Send one whenever your data changes.
- **Send the same rows again as often as you like.** Look compares before it
  writes, so an unchanged push costs one comparison and nothing else. You do
  not need to track what you last sent.
- **Echo the number on every answer**, `req` on a preview and `plan` on a
  `planned` or `result`. It is how Look tells your answer from the one before
  it. An answer without it is dropped, and Look says so on stderr.
- **You may only speak for blocks in your own manifest.** One app cannot
  rewrite another's rows. Look drops frames that try, and logs it.
- **An `op` Look does not know is ignored, never fatal.** Hold yourself to the
  same rule: a Look newer than your connector will send frames you have not
  heard of.
- **Nothing blocks on you.** The panel waits about 400ms for a preview and then
  moves on. A later answer is not shown, so answer from memory rather than
  going to the network.
- **If you exit, your rows stay** and Look starts you again the next time the
  launcher opens.

## Limits

| | |
| --- | --- |
| One frame | 1 MiB. Past that you are a protocol breach rather than a big message, and Look drops you |
| Rows per block | 2,000. The rest are not stored |
| Preview text | 64 KiB, truncated on a character boundary |
| Preview answer | about 400ms before the panel gives up on it |
| Plan answer | about 1.5s, since the user is waiting on a menu |
| Run answer | about 5s, after which Look assumes it is still going |

## What it costs

Mostly you, not Look. Keeping one connector running costs Look about 130 KB and
a thread. Your own program is the real figure: a small compiled one is a
fraction of a megabyte, a Python one is 10 to 15 MB before it does anything.

Push when your data changes rather than on a timer, because every push makes
Look re-read the declared sources. It rate-limits that, but it cannot make it
free.

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
