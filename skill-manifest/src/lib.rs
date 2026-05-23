//! skill-manifest — parse and validate the optional `manifest:` block in
//! a SKILL.md frontmatter.
//!
//! Backwards-compatible by design: a SKILL.md with no `manifest:` key
//! returns `Verdict::Pass` with `manifest_present: false`.

#![cfg_attr(not(test), forbid(unsafe_code))]

use serde::{Deserialize, Serialize};

/// Top-level validation verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Pass,
    Fail,
    Error,
}

/// Structured validation output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationOutput {
    pub verdict: Verdict,
    pub manifest_present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<serde_yml::Value>,
    pub warnings: Vec<String>,
    pub errors: Vec<ValidationError>,
}

/// One validation error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub id: String,
    pub path: String,
    pub message: String,
}

/// Validate a SKILL.md text. Returns a verdict and structured output.
#[must_use]
pub fn validate(content: &str) -> ValidationOutput {
    let frontmatter = match parse_frontmatter(content) {
        Ok(Some(yaml)) => yaml,
        Ok(None) => {
            return ValidationOutput {
                verdict: Verdict::Pass,
                manifest_present: false,
                manifest: None,
                warnings: vec!["no YAML frontmatter present".to_string()],
                errors: vec![],
            };
        }
        Err(message) => {
            return ValidationOutput {
                verdict: Verdict::Fail,
                manifest_present: false,
                manifest: None,
                warnings: vec![],
                errors: vec![ValidationError {
                    id: "yaml_parse_error".to_string(),
                    path: "frontmatter".to_string(),
                    message,
                }],
            };
        }
    };

    let manifest = match frontmatter
        .as_mapping()
        .and_then(|m| m.get(serde_yml::Value::String("manifest".to_string())).cloned())
    {
        Some(m) => m,
        None => {
            return ValidationOutput {
                verdict: Verdict::Pass,
                manifest_present: false,
                manifest: None,
                warnings: vec![],
                errors: vec![],
            };
        }
    };

    let errors = validate_manifest(&manifest);
    let verdict = if errors.is_empty() { Verdict::Pass } else { Verdict::Fail };

    ValidationOutput {
        verdict,
        manifest_present: true,
        manifest: Some(manifest),
        warnings: vec![],
        errors,
    }
}

const KNOWN_MANIFEST_KEYS: &[&str] = &["version", "requires", "exports", "inputs", "tests"];

fn validate_manifest(manifest: &serde_yml::Value) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let map = match manifest.as_mapping() {
        Some(m) => m,
        None => {
            errors.push(ValidationError {
                id: "type_mismatch".to_string(),
                path: "manifest".to_string(),
                message: "manifest must be a mapping".to_string(),
            });
            return errors;
        }
    };

    // additionalProperties: false on the top level.
    for (k, _) in map {
        if let Some(key_str) = k.as_str() {
            if !KNOWN_MANIFEST_KEYS.contains(&key_str) {
                errors.push(ValidationError {
                    id: "unknown_field".to_string(),
                    path: format!("manifest.{key_str}"),
                    message: format!("unknown top-level manifest key: {key_str}"),
                });
            }
        }
    }

    // version (if present) must be a semver string.
    if let Some(v) = map.get(serde_yml::Value::String("version".to_string())) {
        match v.as_str() {
            Some(s) if is_semver(s) => {}
            Some(s) => errors.push(ValidationError {
                id: "invalid_semver".to_string(),
                path: "manifest.version".to_string(),
                message: format!("not a valid semver: {s}"),
            }),
            None => errors.push(ValidationError {
                id: "type_mismatch".to_string(),
                path: "manifest.version".to_string(),
                message: "version must be a string".to_string(),
            }),
        }
    }

    // requires.binaries[].name required; requires.skills[].name required.
    if let Some(requires) = map.get(serde_yml::Value::String("requires".to_string())) {
        if let Some(req_map) = requires.as_mapping() {
            check_named_list(req_map, "binaries", &mut errors);
            check_named_list(req_map, "skills", &mut errors);
        }
    }

    errors
}

fn check_named_list(
    req_map: &serde_yml::Mapping,
    section: &str,
    errors: &mut Vec<ValidationError>,
) {
    let key = serde_yml::Value::String(section.to_string());
    let Some(list) = req_map.get(key) else { return };
    let Some(arr) = list.as_sequence() else { return };
    for (i, entry) in arr.iter().enumerate() {
        if let Some(entry_map) = entry.as_mapping() {
            if !entry_map.contains_key(serde_yml::Value::String("name".to_string())) {
                errors.push(ValidationError {
                    id: "missing_required_field".to_string(),
                    path: format!("manifest.requires.{section}[{i}].name"),
                    message: "name is required".to_string(),
                });
            }
        }
    }
}

fn is_semver(s: &str) -> bool {
    // X.Y.Z optionally followed by `-PRE` where PRE is [A-Za-z0-9.-]+
    let (core, pre) = match s.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (s, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    if !parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())) {
        return false;
    }
    if let Some(p) = pre {
        if p.is_empty() {
            return false;
        }
        if !p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') {
            return false;
        }
    }
    true
}

/// Extract the YAML frontmatter from a SKILL.md.
fn parse_frontmatter(content: &str) -> Result<Option<serde_yml::Value>, String> {
    let after_open = if let Some(rest) = content.strip_prefix("---\n") {
        rest
    } else if let Some(rest) = content.strip_prefix("---\r\n") {
        rest
    } else {
        return Ok(None);
    };

    let close_marker_lf = "\n---\n";
    let close_marker_crlf = "\n---\r\n";
    let close_eof_lf = "\n---";
    let yaml_text = if let Some(idx) = after_open.find(close_marker_lf) {
        &after_open[..idx]
    } else if let Some(idx) = after_open.find(close_marker_crlf) {
        &after_open[..idx]
    } else if let Some(stripped) = after_open.strip_suffix(close_eof_lf) {
        stripped
    } else {
        return Err("frontmatter missing closing '---'".to_string());
    };

    serde_yml::from_str::<serde_yml::Value>(yaml_text)
        .map(Some)
        .map_err(|e| format!("yaml parse error: {e}"))
}
