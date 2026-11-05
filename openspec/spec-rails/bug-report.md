# Bug Report: <Short Defect Title>

## Status
- **Status**: Open | In Progress | Resolved
- **Date**: `<YYYY-MM-DD>`
- **Severity**: Critical | High | Medium | Low
- **Component**: `<affected module/file>`
- **Worktree / Branch**: `.worktrees/fix-<slug>` (`fix/<slug>`)
- **Regression Tests Added**:
  - `<test_path::test_name>`

---

## Summary
<1–3 sentence description of the defect and its user/system impact.>

---

## Steps to Reproduce
1. <Step 1>
2. <Step 2>

### Expected Result
<What should happen according to spec.>

### Actual Result
<What actually happens (error, panic, stale state, wrong output).>

---

## Root Cause Analysis
<Exact trace of why the defect occurs in the code.>

---

## Remediation Plan
1. **Red Test**: <Regression test to write that fails on unmodified `main`.>
2. **Surgical Fix**: <Minimal code change to resolve the root cause.>
3. **Verification**: <Full test + mutation check.>

---

## Review Gates
- [ ] **Gate B1 (Triage & Root Cause)**: Approved by human before code changes.
- [ ] **Gate B2 (Red Reproduction)**: Regression test confirmed failing on unmodified code.
- [ ] **Gate B3 (Green Sign-off & Merge)**:
  - [ ] Regression test passing (`Green`) + full suite green.
  - [ ] Docs & Living Specs Updated (if defect was caused by or impacts existing documentation/specs).
  - [ ] Fast-forward merged (`--ff-only`) into `main`, worktree removed, branch deleted, and `git worktree prune` executed.
