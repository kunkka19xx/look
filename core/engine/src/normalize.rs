//! Moved to `look-matching` so the AI tiers fold text the same way search
//! does. Re-exported here to keep the engine's call sites unchanged.

pub(crate) use look_matching::normalize_for_search;
