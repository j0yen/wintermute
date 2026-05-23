//! agent-pipe — shared NDJSON record schema + streaming pipeline runtime.
//!
//! See PRD-agent-pipe.md for the full design. Phase 0 ships:
//!   - Record schema (kind, source, id, ts + optional fields)
//!   - Streaming subcommands: pass, top, filter, to-paths, from-paths
//!   - Buffering subcommands: sort, pretty, schema

#![cfg_attr(not(test), forbid(unsafe_code))]

use serde_json::Value;

/// Required fields for any record on the agent-pipe wire format.
pub const REQUIRED_FIELDS: &[&str] = &["kind", "source", "id", "ts"];

/// Known record kinds shipped in v1. (Schemas in `apipe schema --kind <name>`.)
pub const KNOWN_KINDS: &[&str] = &["recall_hit", "transcript_turn", "file_event"];

/// A line of input parsed as a record + its original bytes (so identity-pass
/// preserves byte-for-byte ordering of fields).
#[derive(Debug, Clone)]
pub struct ParsedLine {
    /// The unmodified line as it arrived on stdin.
    pub original: String,
    /// The same line parsed as a JSON value (for field access / filtering).
    pub value: Value,
}

/// Outcome of validating a single input line.
#[derive(Debug)]
pub enum LineOutcome {
    /// Line parsed successfully and contains all required fields.
    Ok(ParsedLine),
    /// Line failed JSON parsing.
    Malformed {
        /// The original input line text.
        line: String,
        /// Parser error description.
        error: String,
    },
    /// Line parsed but is missing a required envelope field.
    MissingField {
        /// The original input line text.
        line: String,
        /// Name of the missing field.
        field: String,
    },
}

/// Parse and validate a single line. Returns the structured outcome so the caller
/// can decide whether to emit, drop, or report.
#[must_use]
pub fn parse_and_validate(line: &str) -> LineOutcome {
    let value: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return LineOutcome::Malformed {
                line: line.to_string(),
                error: format!("json parse error: {e}"),
            };
        }
    };

    for field in REQUIRED_FIELDS {
        if !value.get(field).is_some_and(Value::is_string) {
            return LineOutcome::MissingField {
                line: line.to_string(),
                field: (*field).to_string(),
            };
        }
    }

    LineOutcome::Ok(ParsedLine {
        original: line.to_string(),
        value,
    })
}

/// Return the JSON Schema document for a known record kind.
///
/// # Errors
/// Returns `Err` with the offending kind if `name` isn't in `KNOWN_KINDS`.
pub fn schema_for_kind(name: &str) -> Result<Value, String> {
    if !KNOWN_KINDS.contains(&name) {
        return Err(format!("unknown kind: {name}"));
    }

    // Minimal kind-specific schemas; the common envelope is shared.
    let payload_schema = match name {
        "recall_hit" => serde_json::json!({
            "type": "object",
            "properties": {
                "recall_kind": { "type": "string" },
                "body_snippet": { "type": "string" },
                "confidence": { "type": "number" },
                "recall_count": { "type": "integer" }
            }
        }),
        "transcript_turn" => serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": { "type": "string" },
                "turn_index": { "type": "integer" },
                "role": { "type": "string", "enum": ["user", "assistant", "tool"] },
                "text": { "type": "string" }
            }
        }),
        "file_event" => serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "event": { "type": "string" },
                "actor": { "type": "string" }
            }
        }),
        // KNOWN_KINDS guards entry; any unmatched arm is a bug we want to surface as a
        // missing-schema error rather than panicking.
        other => {
            return Err(format!("internal: no schema for known kind: {other}"));
        }
    };

    Ok(serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": format!("agent_pipe.{name}.v1"),
        "type": "object",
        "required": REQUIRED_FIELDS,
        "properties": {
            "kind": { "const": name },
            "source": { "type": "string" },
            "id": { "type": "string" },
            "ts": { "type": "string", "format": "date-time" },
            "session_id": { "type": "string" },
            "subject": { "type": "string" },
            "score": { "type": "number" },
            "payload": payload_schema,
            "annotations": { "type": "array" }
        }
    }))
}

/// Look up a (possibly nested) field on a record. Dotted paths supported.
#[must_use]
pub fn get_field<'a>(record: &'a Value, field: &str) -> Option<&'a Value> {
    let mut current = record;
    for part in field.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}
