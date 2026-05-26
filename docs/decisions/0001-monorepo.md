# ADR 0001: Monorepo layout

## Status

Accepted

## Context

The macOS shell, Rust engine, and FFI bridge are tightly coupled in early development.

## Decision

Use one repository with clear module boundaries:

- `apps/macos`
- `core/*`
- `bridge/ffi`
- `docs`
- crate-local examples/bench entrypoints (for example `core/engine/examples`)

## Consequences

- simple onboarding and shared versioning
- straightforward CI and release coordination
- lower repo management overhead in early phases
