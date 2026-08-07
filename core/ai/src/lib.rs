//! The shared AI brain (see docs/ai-rust-core-plan.md): logic both shells must
//! agree on lives here so prompts, matchers, and parsers can never drift.
//! Ported module by module from the macOS Swift package; the Swift tests are
//! the parity spec and are deleted as their Rust replacements land.

pub mod chat;
pub mod context;
pub mod conversations;
pub mod explicit;
pub mod files;
pub mod markdown;
pub mod matcher;
pub mod memory;
pub mod ollama;
pub mod plan;
pub mod planner;
pub mod referent;
pub mod resolve;
pub mod textops;
pub mod window;
