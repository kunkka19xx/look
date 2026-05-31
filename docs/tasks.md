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

- **macOS titlebar hairline on first show** — *fixed*. macOS Sequoia honors `titlebarSeparatorStyle = .none` only after the first real frame resize, so a 1px line appeared on the first paint. Fixed in `WindowConfigurator.suppressTitlebarHairline(in:)`: on first show (and on each `updateNSView`) we apply a deferred 1px frame round-trip — grow the height by 1pt this runloop tick, restore it on the next — which forces AppKit to re-evaluate and drop the separator, plus a fallback that walks the theme-frame hierarchy hiding any private `…TitlebarSeparator…` / `NSTitlebarDecorationView` subview. The earlier same-tick grow/restore was a no-op because AppKit coalesced it; splitting it across two ticks is what makes it register.
