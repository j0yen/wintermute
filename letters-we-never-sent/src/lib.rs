//! letters-we-never-sent — curation library for Markdown letters with
//! YAML frontmatter.
//!
//! The library splits each letter into a parsed [`frontmatter::Frontmatter`]
//! plus a verbatim body byte buffer, and provides the mutation helpers used
//! by the `letter` binary. Body bytes are preserved across edits.

#![cfg_attr(not(test), forbid(unsafe_code))]

pub mod cli;
pub mod error;
pub mod frontmatter;
pub mod letter;
pub mod listing;
pub mod state;

pub use error::{LetterError, LetterResult};
pub use state::State;
