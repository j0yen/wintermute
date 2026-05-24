//! self-portrait — extract a single file's git history as structured JSON.
//!
//! Phase 1a surface: walk `git log --follow`-style history for a target
//! file under a git repository and emit, per commit that touched it,
//! a record carrying hash/date/author/subject/body/diff plus a
//! top-level `deleted_at` field if the file no longer exists at HEAD.
//!
//! The output is an [`Extract`] which serialises to the canonical JSON
//! consumed by downstream rendering passes (LLM reflections, Typst
//! typesetting). The extractor is deterministic and does not editorialize:
//! trivial commits are emitted verbatim (consumers decide whether to
//! collapse them via `--collapse-trivial`).
//!
//! Privacy clause: commits whose body contains a `private: true` trailer
//! at column 0 have their `diff` field redacted to the literal string
//! `"[redacted]"` — hash/date/author/subject still appear so the
//! timeline is complete.

#![cfg_attr(not(test), forbid(unsafe_code))]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate
)]

pub mod error;
pub mod extract;

pub use error::Error;
pub use extract::{CommitEntry, Extract, ExtractOptions, extract};
