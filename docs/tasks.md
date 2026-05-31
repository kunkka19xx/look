## Evergreen: Search quality and performance (always-on)

- [ ] **Indexing improvement loop**: continually refine scan roots, excludes, and incremental refresh strategy
- [ ] **Matching improvement loop**: improve typo tolerance, tokenization, and relevance scoring for mixed app/file queries
- [ ] **Optimization loop**: keep reducing query latency, startup cost, and memory use as regular maintenance

## Weekly checklist: quality + performance

Run this checklist at least once per week (or before release cut):

- [ ] collect baseline metrics from the same sample dataset and keep results in a dated note (`docs/bench-notes/YYYY-MM-DD.md`)
- [ ] measure query latency (`p50`, `p95`) for empty query, short query (2-4 chars), and long query (8+ chars)
- [ ] measure startup time (app launch -> first usable search result)
- [ ] compare index size and memory usage versus last baseline
- [ ] verify top-5 relevance for a fixed smoke query set (apps, files, folders, settings)
- [ ] review at least 3 recent user-reported misses and convert into matching/indexing improvements
- [ ] add/update at least one test for any ranking/matching/indexing behavior change

Suggested guardrails (adjust as project evolves):

- `query latency p50`: <= 30ms
- `query latency p95`: <= 80ms
- `startup to first result`: <= 700ms
- `peak memory (idle window)`: <= 220MB
- `relevance smoke pass rate`: >= 90% in top-5

Escalation rule:

- if any guardrail regresses by >10% week-over-week, open a focused perf/quality issue before merging unrelated polish work

## Parked: known issues

- **macOS titlebar hairline on first show**: a 1px line appears at the top of the launcher window on the very first paint, because macOS Sequoia honors `titlebarSeparatorStyle = .none` only after the first real frame resize. Confirmed workaround for users: open Settings → Running Apps and toggle the placement once (e.g. Top → Right, or back to Top). After any real resize the line goes away permanently for that session. Tried — and reverted — fixes: hiding `NSThemeFrame` subviews (line is drawn by the parent, not a subview), dropping `frameView.layer.cornerRadius` (artifact persists), forcing a deferred 1px frame bump on first show (works but felt fragile). Likely real fix lives in window-class territory (NSPanel swap with `canBecomeKey` override); deferred until we can verify the swap doesn't regress the `didResignActive` auto-hide path.
