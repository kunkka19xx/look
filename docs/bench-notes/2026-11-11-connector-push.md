# What a connector's push costs (2026-11-11)

Command:

```bash
cargo run --release --manifest-path core/Cargo.toml --example connector_push
```

Environment:

- machine: Apple Silicon, macOS 25.5
- profile: `release`
- rows: `2000` (the per-block cap), payload `236 KiB`, `1000` pushes

## The unchanged push

A connector watching something that has not moved sends the same rows over and
over, so this is the state it spends most of its life in. `connectors::store`
answers it by reading the cached rows back and comparing.

| | per push |
| --- | ---: |
| read the cache file and compare | `0.0335 ms` |
| hash the payload instead (SipHash, `DefaultHasher`) | `0.0708 ms` |

## Why this note exists

`docs/integrating.md` used to tell connector authors that an unchanged push
"costs one comparison and nothing else", which was not true: it reads 236 KiB
off disk and compares it. The plan was to remember a hash of what was last
written and skip the read.

**The measurement killed the change.** Hashing the payload is a little over
twice the cost of reading it back, because the cache file is in the page cache
by the time the second push arrives, so the read is a `memcpy` at roughly
7 GB/s while `DefaultHasher` manages about 3.3 GB/s. The read was never the
expensive part.

So `store` keeps asking the cache, which also keeps the property its comment
defends: the answer stays true whatever else touched the file. The doc now
quotes the real figure instead of promising a free lunch.

A faster hash would win on CPU, but it would mean a new dependency to save
30us on a background thread, and at 50 Hz with a full block the current cost is
under 2 ms of CPU per second. Not worth it.

## What to watch instead

The push path's real cost is downstream, in the index refresh the push triggers.
That is already coalesced: one read per 300ms of settling, and never more often
than once every 2s however fast the pushes arrive.
