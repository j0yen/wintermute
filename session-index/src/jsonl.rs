//! Parse Claude Code session JSONL into indexable records.
//!
//! One JSONL line can yield zero or more [`IndexableRecord`]s — a user message
//! is one row; an assistant message can contribute one text row plus one row
//! per `tool_use` block; a user `tool_result` content block becomes its own row.

use serde::Deserialize;
use serde_json::Value;

const TOOL_RESULT_BUDGET: usize = 4096;

/// One row that will land in the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexableRecord {
    pub ts: Option<String>,
    /// `user` | `assistant` | `tool_use` | `tool_result`
    pub role: String,
    /// Set for `tool_use` / `tool_result`.
    pub tool_name: Option<String>,
    pub text: String,
}

#[derive(Debug, Deserialize)]
struct Line {
    #[serde(default, rename = "type")]
    ty: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    message: Option<Message>,
}

#[derive(Debug, Deserialize)]
struct Message {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<Value>,
}

/// Parse one JSONL line into zero or more rows. A malformed line yields
/// no rows; we never fail the whole reindex on one bad line.
pub fn parse_line(line: &str) -> Vec<IndexableRecord> {
    let Ok(parsed) = serde_json::from_str::<Line>(line) else {
        return Vec::new();
    };
    let ts = parsed.timestamp.clone();
    let ty = parsed.ty.as_deref().unwrap_or("");
    match ty {
        "user" | "assistant" => extract_from_message(ty, ts.as_deref(), parsed.message.as_ref()),
        // system / ai-title / attachment / file-history-snapshot / last-prompt /
        // permission-mode / queue-operation are skipped in Phase 0.
        _ => Vec::new(),
    }
}

fn extract_from_message(ty: &str, ts: Option<&str>, msg: Option<&Message>) -> Vec<IndexableRecord> {
    let Some(m) = msg else { return Vec::new() };
    let role = m.role.as_deref().unwrap_or(ty);
    let Some(content) = m.content.as_ref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    match content {
        Value::String(s) => {
            let t = s.trim();
            if !t.is_empty() {
                out.push(IndexableRecord {
                    ts: ts.map(str::to_owned),
                    role: role.to_owned(),
                    tool_name: None,
                    text: t.to_owned(),
                });
            }
        }
        Value::Array(blocks) => {
            for block in blocks {
                push_block(role, ts, block, &mut out);
            }
        }
        _ => {}
    }
    out
}

fn push_block(role: &str, ts: Option<&str>, block: &Value, out: &mut Vec<IndexableRecord>) {
    let bt = block.get("type").and_then(Value::as_str).unwrap_or("");
    match bt {
        "text" => {
            if let Some(text) = block.get("text").and_then(Value::as_str) {
                let t = text.trim();
                if !t.is_empty() {
                    out.push(IndexableRecord {
                        ts: ts.map(str::to_owned),
                        role: role.to_owned(),
                        tool_name: None,
                        text: t.to_owned(),
                    });
                }
            }
        }
        "tool_use" => {
            let name = block
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let input = block.get("input").cloned().unwrap_or(Value::Null);
            let text = flatten_tool_input(&input);
            out.push(IndexableRecord {
                ts: ts.map(str::to_owned),
                role: "tool_use".to_owned(),
                tool_name: Some(name),
                text,
            });
        }
        "tool_result" => {
            let text = stringify_tool_result(block.get("content").unwrap_or(&Value::Null));
            let truncated = truncate_chars(&text, TOOL_RESULT_BUDGET);
            out.push(IndexableRecord {
                ts: ts.map(str::to_owned),
                role: "tool_result".to_owned(),
                tool_name: None,
                text: truncated,
            });
        }
        // "thinking" content is not user-facing; PRD §4.1 doesn't list it. Skip.
        _ => {}
    }
}

/// Flatten a `tool_use` `input` dict into FTS-searchable `key:"value"` pairs.
/// Per PRD §4.5.
fn flatten_tool_input(v: &Value) -> String {
    match v {
        Value::Object(map) => {
            let mut parts = Vec::with_capacity(map.len());
            for (k, val) in map {
                let s = match val {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                parts.push(format!("{k}:\"{s}\""));
            }
            parts.join(" ")
        }
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Tool-result `content` is either a string or an array of content blocks
/// (each typically `{type:"text", text:"…"}`). Collapse to plain text.
fn stringify_tool_result(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => {
            let mut acc = String::new();
            for b in blocks {
                if let Some(t) = b.get("text").and_then(Value::as_str) {
                    if !acc.is_empty() {
                        acc.push('\n');
                    }
                    acc.push_str(t);
                }
            }
            acc
        }
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_owned();
    }
    // Char-boundary-safe truncation.
    let mut end = max;
    while !s.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    s[..end].to_owned()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn user_string_content() {
        let line =
            r#"{"type":"user","timestamp":"t","message":{"role":"user","content":"hello world"}}"#;
        let rows = parse_line(line);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].role, "user");
        assert_eq!(rows[0].text, "hello world");
    }

    #[test]
    fn assistant_text_and_tool_use() {
        let line = r#"{"type":"assistant","timestamp":"t","message":{"role":"assistant","content":[
            {"type":"text","text":"sure"},
            {"type":"tool_use","name":"Bash","input":{"command":"ls","description":"list"}}
        ]}}"#;
        let rows = parse_line(line);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].role, "assistant");
        assert_eq!(rows[0].text, "sure");
        assert_eq!(rows[1].role, "tool_use");
        assert_eq!(rows[1].tool_name.as_deref(), Some("Bash"));
        assert!(rows[1].text.contains("command:\"ls\""));
    }

    #[test]
    fn tool_result_in_user_array() {
        let line = r#"{"type":"user","timestamp":"t","message":{"role":"user","content":[
            {"type":"tool_result","tool_use_id":"x","content":"ok\nresult"}
        ]}}"#;
        let rows = parse_line(line);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].role, "tool_result");
        assert_eq!(rows[0].text, "ok\nresult");
    }

    #[test]
    fn skips_unknown_type() {
        let line = r#"{"type":"system","content":"x"}"#;
        assert!(parse_line(line).is_empty());
    }

    #[test]
    fn ignores_malformed_line() {
        assert!(parse_line("not json").is_empty());
    }

    #[test]
    fn truncates_large_tool_result() {
        let big = "x".repeat(10_000);
        let line = format!(
            r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","content":"{big}"}}]}}}}"#
        );
        let rows = parse_line(&line);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text.len(), TOOL_RESULT_BUDGET);
    }

    #[test]
    fn thinking_block_is_skipped() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"hmm"}]}}"#;
        assert!(parse_line(line).is_empty());
    }
}
