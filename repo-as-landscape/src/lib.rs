//! repo-as-landscape — extract topographic primitives from a git repo.
//!
//! Phase 0 surface: walk the HEAD tree of a git repository and emit, per
//! tracked file, a small struct describing language (by extension),
//! commit count touching that path, last-touched committer date,
//! primary author, and size in bytes.
//!
//! The output is a [`Landscape`] which serialises to the canonical JSON
//! consumed by downstream rendering passes.

#![cfg_attr(not(test), forbid(unsafe_code))]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate
)]

pub mod error;
pub mod language;
pub mod walk;

pub use error::Error;
pub use language::language_for_path;
pub use walk::{FileEntry, Landscape, WalkOptions, walk_repo};
