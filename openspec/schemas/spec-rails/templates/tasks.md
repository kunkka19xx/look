# Tasks

## 1. Dependency DAG
<!-- Mermaid DAG of slices and streams. -->

## 2. Streams & Worktrees
| Stream | Worktree Path | Branch | Prerequisite |
| :--- | :--- | :--- | :--- |

## 3. Slice 1: <!-- name -->

- [ ] 3.0 Contract scaffolding: interfaces, types, schemas, and empty signatures committed
- [ ] 3.1 BDD scenarios for the slice (verify: `.feature` committed)
- [ ] 3.2 Gate 5.1 human sign-off on BDD scenarios
- [ ] 3.3 Red unit tests fail for the expected reason (verify: output shown; append trailing evidence when ticking)
- [ ] 3.4 Green implementation, tests and code committed together (verify: suite passes; append trailing evidence when ticking)
- [ ] 3.5 Coverage >= 95% and mutation > 90% kill (verify: reports; append trailing evidence when ticking)

## 4. Integration & Rebase Plan
- [ ] 4.1 Human Comprehension Verification: reviewer can explain all generated logic line-by-line
- [ ] 4.2 Merge streams, rebase on main, re-verify rails, `openspec archive`

## Review Gate
- [ ] **Gate 4 Approval**: Tasks breakdown approved by human before writing test code.
