//! The shared AI brain (see docs/ai-architecture.md): logic both shells
//! must agree on lives here so prompts, matchers, and parsers can never drift.
//! Data-only across the FFI boundary: JSON in, JSON out, no closures.

pub mod calling;
pub mod chat;
pub mod context;
pub mod conversations;
pub mod domain;
pub mod explicit;
pub mod files;
pub mod lexicon;
pub mod markdown;
pub mod matcher;
pub mod meeting;
pub mod memory;
pub mod ollama;
pub mod plan;
pub mod planner;
pub mod referent;
pub mod resolve;
pub mod route;
mod store;
pub mod textops;
pub mod window;
