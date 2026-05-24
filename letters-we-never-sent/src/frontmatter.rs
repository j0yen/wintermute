//! Frontmatter parsing and serialization.
//!
//! Splits a letter file into `(frontmatter_yaml, body_bytes)` and rebuilds
//! it preserving the body byte-for-byte.

use crate::error::{LetterError, LetterResult};
use crate::state::State;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Parsed YAML frontmatter block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frontmatter {
    /// Anonymized recipient label.
    #[serde(default)]
    pub recipient: Option<String>,
    /// Year-month tag in `YYYY-MM` form.
    #[serde(default)]
    pub month: Option<String>,
    /// Triage state.
    #[serde(default)]
    pub state: State,
    /// RFC3339 timestamp of last state transition; `null` while pending.
    #[serde(default)]
    pub accepted_at: Option<String>,
    /// One-line subject.
    #[serde(default)]
    pub subject: Option<String>,
}

/// The on-disk shape of a parsed letter: frontmatter plus the raw byte body
/// that follows the closing `---` line.
#[derive(Debug, Clone)]
pub struct LetterDoc {
    /// Parsed frontmatter.
    pub front: Frontmatter,
    /// Body bytes verbatim (everything after the closing `---\n`).
    pub body: Vec<u8>,
}

const DELIM: &str = "---";

/// Parse a letter file into frontmatter + body. Body bytes are preserved.
///
/// # Errors
/// Returns `LetterError::Unparseable` if the file has no leading `---` line
/// or the YAML inside cannot be deserialized.
pub fn parse(bytes: &[u8], path: &Path) -> LetterResult<LetterDoc> {
    let text = std::str::from_utf8(bytes).map_err(|e| LetterError::Unparseable {
        path: path.to_path_buf(),
        reason: format!("not valid utf-8: {e}"),
    })?;
    let (yaml, body) = split(text, path)?;
    let front: Frontmatter = serde_yml::from_str(yaml).map_err(|e| LetterError::Unparseable {
        path: path.to_path_buf(),
        reason: format!("yaml: {e}"),
    })?;
    Ok(LetterDoc {
        front,
        body: body.as_bytes().to_vec(),
    })
}

fn split<'a>(text: &'a str, path: &Path) -> LetterResult<(&'a str, &'a str)> {
    let rest = text.strip_prefix("---\n").ok_or_else(|| LetterError::Unparseable {
        path: path.to_path_buf(),
        reason: "missing opening --- frontmatter delimiter".into(),
    })?;
    if let Some(close) = rest.find("\n---\n") {
        let yaml = rest.get(..close).unwrap_or("");
        let body = rest.get(close + 5..).unwrap_or("");
        return Ok((yaml, body));
    }
    if let Some(yaml) = rest.strip_suffix("\n---") {
        return Ok((yaml, ""));
    }
    Err(LetterError::Unparseable {
        path: path.to_path_buf(),
        reason: "missing closing --- frontmatter delimiter".into(),
    })
}

/// Serialize frontmatter + body back to disk bytes.
///
/// # Errors
/// Returns `LetterError::Serialize` if YAML serialization fails.
pub fn serialize(doc: &LetterDoc) -> LetterResult<Vec<u8>> {
    let yaml = serde_yml::to_string(&doc.front).map_err(|e| LetterError::Serialize(e.to_string()))?;
    let yaml_trimmed = yaml.trim_end_matches('\n');
    let mut out = Vec::with_capacity(yaml_trimmed.len() + doc.body.len() + 16);
    out.extend_from_slice(DELIM.as_bytes());
    out.push(b'\n');
    out.extend_from_slice(yaml_trimmed.as_bytes());
    out.push(b'\n');
    out.extend_from_slice(DELIM.as_bytes());
    out.push(b'\n');
    out.extend_from_slice(&doc.body);
    Ok(out)
}
