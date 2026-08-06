//! The shared AI brain (see docs/ai-rust-core-plan.md): logic both shells must
//! agree on lives here so prompts, matchers, and parsers can never drift.
//! Ported module by module from the macOS Swift package; the Swift tests are
//! the parity spec and are deleted as their Rust replacements land.

pub mod markdown;
pub mod referent;
