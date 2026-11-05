---
name: spec-rails
description: >-
  Spec-Driven, Testing-Railed Development (Spec-Rails) workflow for gated feature
  delivery, multi-agent Git worktree streams, Red-Green-Mutation testing rails,
  and zero-contamination bugfix isolation. Activate when installing Spec-Rails on an OpenSpec project,
  proposing features, triaging bugs, or executing gated vertical slices.
---

# Spec-Rails Skill

Follow `openspec/spec-rails/WORKFLOW.md`. Normalize roadmap IDs to lowercase hyphens (`W1.2` → `w1-2-<slug>`) and create a change with `openspec new change <id>-<slug>`. For gated planning, generate one artifact at a time with `openspec instructions <artifact> --change <id>-<slug>`, pausing for a human to tick and commit its `## Review Gate` checkbox before generating the next file (note: `/opsx-propose` drafts all artifacts at once and will block commits until Gates 0–3 are reviewed and ticked). Run `/opsx-apply` and `/opsx-archive` only after Gate 4 is ticked by a human.

## 0. The Ponytail Mindset & Strict YAGNI

- **Default answer is "No"**: Push back on vague or unneeded requirements before building.
- **Boring > Clever**: Standard library over new dependencies. Simple logic over complex acrobatics.
- **Code is a liability**: Smallest diff that satisfies specifications and passes gates wins.
- **Zero speculative architecture**: No unused abstractions, premature plugin frameworks, or hypothetical configuration flags.
- **Minimal blast radius**: Touch only files strictly required for the slice.
- **Ruthless deletion**: Kill mutants or delete dead code. Any surviving mutant triggers deletion of unneeded code.

## 1. Pick a Lane First

- **Feature** (`feat/<id>-<slug>`): Follow Gates 0–6.
- **Bugfix** (`fix/<slug>`): Follow Gates B1–B3.
- **Chore** (`chore/<slug>`): Zero behavior change; run pre-commit & tests; no gates.

## 2. Gate Execution Checklist

1. **Gate 0 (Research & Tradeoffs)**: Generate `research.md` in `openspec/changes/<id>-<slug>/`. Present 2–3 options, tradeoffs, and breaking changes. Stop for human developer decision.
   - If **Go**: Update `docs/ROADMAP.md` to `🚀 In Progress` via `chore/roadmap-<id>`.
   - If **No-Go**: Commit `research.md` to `openspec/changes/archive/<id>-<slug>/`, mark `❌ Dropped` in `docs/ROADMAP.md` with rationale via `chore/roadmap-<id>`, and prune feature branch.
2. **Gate 1 (Proposal)**: Generate `proposal.md` for the chosen option. Stop for human review.
3. **Gate 2 (Specs)**: OpenSpec delta specs (`### Requirement: REQ-X-N name`, MUST/SHALL, at least one scenario). MODIFIED/REMOVED repeat the existing header verbatim (for deprecated requirements, keep the exact header and put `**[DEPRECATED by <change>]**` at the start of the body). Stop for human review.
4. **Gate 3 (Design)**: Generate `design.md`. Stop for human review.
5. **Gate 4 (Tasks)**: Generate `tasks.md` with DAG and worktree streams. Stop for human review before running `/opsx-apply` or spawning slice worktrees.
6. **Apply (Gate 5.x)**: Only after Gate 4 is ticked by a human, execute per vertical slice:
   - **5.x-A**: Write BDD `.feature` scenarios $\rightarrow$ **Stop for Gate 5.x human approval** (never self-tick Gate 5.x sign-off boxes).
   - **5.x-B**: Write failing unit tests (`Red`) tested locally, then implement minimal code (`Green`). Commit tests + code together. Line & branch coverage $\ge 95\%$.
   - **5.x-C**: Run mutation testing $\rightarrow$ achieve $> 90\%$ kill score on core logic (kill, delete dead code, or restructure equivalent mutants).
7. **Gate 6 (Archive & Merge)**: Ensure all stream worktrees are merged, removed, and pruned (`git worktree prune`). Reviewer verifies line-by-line understanding of all code (delete ununderstood code). Tick completed slice items and the final integration task in `tasks.md` (with `Evidence:` on Red/Green/Coverage/Mutation tasks), rebase onto `main`, re-verify rails, run `openspec validate <change>`, then run `openspec archive <change> --yes` (or `/opsx-archive`; syncs deltas into `openspec/specs/` and archives the change), merge `--ff-only`, and update `docs/ROADMAP.md` to `✅ Shipped`.

Never tick a Gate 0–4 Review Gate box or Gate 5.x sign-off box yourself. Run `openspec validate` after stream rebases, and run `spec-rails-check` before committing. See `openspec/spec-rails/WORKFLOW.md` §7.
