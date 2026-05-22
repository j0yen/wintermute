//! recall — local-first agentic memory.
//!
//! See `PRD-agentic-memory.md` in the autobuilder repo for the motivation
//! and goals. This crate implements Phase 0 + Phase 1 + Phase 2a: a
//! file-backed memory store under `~/.claude/recall/memories/`, a
//! `SQLite`/FTS5 keyword index, and a hashed-feature embedder with hybrid
//! retrieval. A real semantic model (BGE-small) is the Phase 2b swap.

pub mod embeddings;
pub mod index;
pub mod memory;
pub mod paths;
pub mod retrieval;
pub mod store;
