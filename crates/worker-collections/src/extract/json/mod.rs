use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader};

use crate::extract::config::ExtractionOptions;

/// Result of JSON extraction with text and optional structure metadata
#[derive(Debug)]
pub struct JsonExtractionResult {
    pub text: String,
    pub metadata: Option<Value>,
}

/// Extract text from JSON with full options
pub(crate) fn extract_with_options(
    bytes: &[u8],
    options: &ExtractionOptions,
) -> Result<JsonExtractionResult> {
    let content =
        std::str::from_utf8(bytes).map_err(|e| anyhow!("Invalid UTF-8 in JSON: {}", e))?;

    let value: Value = serde_json::from_str(content).map_err(|e| anyhow!("Invalid JSON: {}", e))?;

    let mut text_parts = Vec::new();
    let mut paths = Vec::new();

    extract_strings_recursive(&value, String::new(), &mut text_parts, &mut paths, options);

    let metadata = if options.include_metadata {
        Some(json!({
            "format": "json",
            "paths": paths,
            "is_array": value.is_array(),
            "is_object": value.is_object(),
        }))
    } else {
        None
    };

    Ok(JsonExtractionResult {
        text: text_parts.join("\n"),
        metadata,
    })
}

/// Extract text from NDJSON with full options
pub(crate) fn extract_ndjson_with_options(
    bytes: &[u8],
    options: &ExtractionOptions,
) -> Result<JsonExtractionResult> {
    let reader = BufReader::new(bytes);
    let mut all_text = Vec::new();
    let mut record_count = 0;
    let mut all_paths = Vec::new();

    for (line_num, line_result) in reader.lines().enumerate() {
        let line =
            line_result.map_err(|e| anyhow!("Read error on line {}: {}", line_num + 1, e))?;
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => {
                let mut text_parts = Vec::new();
                let mut paths = Vec::new();

                extract_strings_recursive(
                    &value,
                    format!("[{}]", record_count),
                    &mut text_parts,
                    &mut paths,
                    options,
                );

                if !text_parts.is_empty() {
                    all_text.push(text_parts.join(" "));
                }
                all_paths.extend(paths);
                record_count += 1;
            }
            Err(e) => {
                // Log warning but continue processing other lines
                tracing::warn!("Skipping invalid JSON on line {}: {}", line_num + 1, e);
            }
        }
    }

    let metadata = if options.include_metadata {
        Some(json!({
            "format": "ndjson",
            "record_count": record_count,
            "unique_paths": all_paths.into_iter().collect::<std::collections::HashSet<_>>().into_iter().collect::<Vec<_>>(),
        }))
    } else {
        None
    };

    Ok(JsonExtractionResult {
        text: all_text.join("\n"),
        metadata,
    })
}

/// Recursively extract string values from JSON, tracking paths
fn extract_strings_recursive(
    value: &Value,
    current_path: String,
    text_parts: &mut Vec<String>,
    paths: &mut Vec<String>,
    options: &ExtractionOptions,
) {
    match value {
        Value::String(s) => {
            if !s.trim().is_empty() {
                // Optionally include path as prefix
                if options.preserve_formatting && !current_path.is_empty() {
                    text_parts.push(format!("{}: {}", current_path, s));
                } else {
                    text_parts.push(s.clone());
                }
                if !current_path.is_empty() {
                    paths.push(current_path);
                }
            }
        }
        Value::Number(n) => {
            // Include numbers as text (useful for data context)
            if options.preserve_formatting && !current_path.is_empty() {
                text_parts.push(format!("{}: {}", current_path, n));
                paths.push(current_path);
            }
        }
        Value::Bool(b) => {
            // Include booleans only if preserving formatting
            if options.preserve_formatting && !current_path.is_empty() {
                text_parts.push(format!("{}: {}", current_path, b));
                paths.push(current_path);
            }
        }
        Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                let new_path = if current_path.is_empty() {
                    format!("[{}]", i)
                } else {
                    format!("{}[{}]", current_path, i)
                };
                extract_strings_recursive(item, new_path, text_parts, paths, options);
            }
        }
        Value::Object(obj) => {
            for (key, val) in obj {
                let new_path = if current_path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", current_path, key)
                };
                extract_strings_recursive(val, new_path, text_parts, paths, options);
            }
        }
        Value::Null => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_json_object() {
        let json = r#"{"name": "John", "message": "Hello, world!"}"#;
        let result = extract_with_options(json.as_bytes(), &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap().text;
        assert!(text.contains("John"));
        assert!(text.contains("Hello, world!"));
    }

    #[test]
    fn test_extract_json_array() {
        let json = r#"[{"text": "First"}, {"text": "Second"}]"#;
        let result = extract_with_options(json.as_bytes(), &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap().text;
        assert!(text.contains("First"));
        assert!(text.contains("Second"));
    }

    #[test]
    fn test_extract_nested_json() {
        let json = r#"{"user": {"profile": {"bio": "Software developer"}}}"#;
        let result = extract_with_options(json.as_bytes(), &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap().text;
        assert!(text.contains("Software developer"));
    }

    #[test]
    fn test_extract_json_with_metadata() {
        let json = r#"{"title": "Test", "content": "Hello"}"#;
        let mut options = ExtractionOptions::default();
        options.include_metadata = true;

        let result = extract_with_options(json.as_bytes(), &options);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        assert!(extraction.metadata.is_some());
        let meta = extraction.metadata.unwrap();
        assert_eq!(meta["format"], "json");
    }

    #[test]
    fn test_extract_ndjson() {
        let ndjson = r#"{"name": "Alice"}
{"name": "Bob"}
{"name": "Charlie"}"#;
        let result = extract_ndjson_with_options(ndjson.as_bytes(), &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap().text;
        assert!(text.contains("Alice"));
        assert!(text.contains("Bob"));
        assert!(text.contains("Charlie"));
    }

    #[test]
    fn test_extract_ndjson_with_empty_lines() {
        let ndjson = r#"{"name": "First"}

{"name": "Second"}
"#;
        let result = extract_ndjson_with_options(ndjson.as_bytes(), &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap().text;
        assert!(text.contains("First"));
        assert!(text.contains("Second"));
    }

    #[test]
    fn test_extract_ndjson_record_count() {
        let ndjson = r#"{"id": 1}
{"id": 2}
{"id": 3}"#;
        let mut options = ExtractionOptions::default();
        options.include_metadata = true;

        let result = extract_ndjson_with_options(ndjson.as_bytes(), &options);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        // Check record_count in metadata
        let meta = extraction.metadata.unwrap();
        assert_eq!(meta["record_count"], 3);
    }

    #[test]
    fn test_extract_json_preserving_paths() {
        let json = r#"{"user": {"name": "Test", "email": "test@example.com"}}"#;
        let mut options = ExtractionOptions::default();
        options.preserve_formatting = true;
        options.include_metadata = true;

        let result = extract_with_options(json.as_bytes(), &options);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        // With preserve_formatting, paths should be included
        assert!(extraction.text.contains("user.name"));
        assert!(extraction.text.contains("user.email"));
    }

    #[test]
    fn test_extract_invalid_json() {
        let invalid = b"not valid json {";
        let result = extract_with_options(invalid, &ExtractionOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_empty_json_object() {
        let json = r#"{}"#;
        let result = extract_with_options(json.as_bytes(), &ExtractionOptions::default());
        assert!(result.is_ok());
        assert!(result.unwrap().text.is_empty());
    }

    #[test]
    fn test_extract_json_with_numbers_and_bools() {
        let json = r#"{"count": 42, "active": true, "label": "test"}"#;
        let mut options = ExtractionOptions::default();
        options.preserve_formatting = true;

        let result = extract_with_options(json.as_bytes(), &options);
        assert!(result.is_ok());
        let text = result.unwrap().text;
        assert!(text.contains("test"));
        assert!(text.contains("42"));
        assert!(text.contains("true"));
    }
}
