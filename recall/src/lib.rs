//! recall — local-first agentic memory.
//!
//! See `PRD-agentic-memory.md` in the autobuilder repo for the motivation
//! and goals. This crate implements Phase 0 + Phase 1: a file-backed memory
//! store under `~/.claude/recall/memories/` plus a SQLite/FTS5 keyword
//! index. Embeddings, hooks, observed-write proposals, and outcome feedback
//! are deferred.

pub mod index;
pub mod memory;
pub mod paths;
pub mod retrieval;
pub mod store;
