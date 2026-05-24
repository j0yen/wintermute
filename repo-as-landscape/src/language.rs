//! Language detection by file extension.
//!
//! Pure mapping from the file path's lowercased extension to a stable
//! language label. Unknown extensions return `"unknown"`.

use std::path::Path;

/// Return the language label for `path`, based on its extension.
///
/// Falls back to `"unknown"` when there is no extension or the
/// extension is not in the recognised set.
#[must_use]
pub fn language_for_path(path: &Path) -> &'static str {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return "unknown";
    };
    let ext_lower = ext.to_ascii_lowercase();
    match ext_lower.as_str() {
        "rs" => "Rust",
        "py" => "Python",
        "ts" | "tsx" => "TypeScript",
        "js" | "jsx" => "JavaScript",
        "md" => "Markdown",
        "json" => "JSON",
        "toml" => "TOML",
        "sh" | "bash" | "zsh" => "Shell",
        "yml" | "yaml" => "YAML",
        "html" => "HTML",
        "css" => "CSS",
        "go" => "Go",
        "c" | "h" => "C",
        "cpp" | "hpp" => "C++",
        "java" => "Java",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn known_extensions() {
        assert_eq!(language_for_path(&PathBuf::from("a.rs")), "Rust");
        assert_eq!(language_for_path(&PathBuf::from("a.PY")), "Python");
        assert_eq!(language_for_path(&PathBuf::from("a.tsx")), "TypeScript");
        assert_eq!(language_for_path(&PathBuf::from("a.cpp")), "C++");
        assert_eq!(language_for_path(&PathBuf::from("a.h")), "C");
    }

    #[test]
    fn unknown_extension() {
        assert_eq!(language_for_path(&PathBuf::from("Makefile")), "unknown");
        assert_eq!(language_for_path(&PathBuf::from("a.xyz")), "unknown");
    }
}
