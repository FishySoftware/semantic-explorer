//! RTF (Rich Text Format) text extraction.
//!
//! This module extracts plain text from RTF documents using rtf-parser.

use anyhow::Result;
use rtf_parser::parse_rtf;
use serde_json::{Value, json};

use crate::extract::config::ExtractionOptions;

/// Result of RTF extraction
#[derive(Debug)]
pub struct RtfExtractionResult {
    /// Extracted plain text content
    pub text: String,
    /// Document metadata if available
    pub metadata: Option<Value>,
}

/// Extract text with metadata from RTF
pub fn extract_with_metadata(
    bytes: &[u8],
    options: &ExtractionOptions,
) -> Result<RtfExtractionResult> {
    let content = String::from_utf8_lossy(bytes).to_string();
    let doc = parse_rtf(content.clone());
    let text = doc.get_text();

    let metadata = if options.include_metadata {
        Some(extract_rtf_metadata(&content, &doc))
    } else {
        None
    };

    Ok(RtfExtractionResult { text, metadata })
}

/// Extract metadata from RTF document
fn extract_rtf_metadata(content: &str, doc: &rtf_parser::RtfDocument) -> Value {
    let mut metadata = serde_json::Map::new();

    // Get header info if available
    let header = &doc.header;

    // Character set
    metadata.insert(
        "charset".to_string(),
        json!(format!("{:?}", header.character_set)),
    );

    // Font count
    metadata.insert("font_count".to_string(), json!(header.font_table.len()));

    // Color count
    metadata.insert("color_count".to_string(), json!(header.color_table.len()));

    // Detect RTF version from content
    if content.contains("\\rtf1") {
        metadata.insert("rtf_version".to_string(), json!("1"));
    }

    // Extract info group metadata if present
    if let Some(title) = extract_info_field(content, "title") {
        metadata.insert("title".to_string(), json!(title));
    }
    if let Some(author) = extract_info_field(content, "author") {
        metadata.insert("author".to_string(), json!(author));
    }
    if let Some(subject) = extract_info_field(content, "subject") {
        metadata.insert("subject".to_string(), json!(subject));
    }
    if let Some(keywords) = extract_info_field(content, "keywords") {
        metadata.insert("keywords".to_string(), json!(keywords));
    }

    json!(metadata)
}

/// Extract an info field value from RTF content
/// RTF info fields look like: {\info{\title Value}{\author Name}}
fn extract_info_field(content: &str, field: &str) -> Option<String> {
    // Look for pattern like {\title ...} or {\author ...}
    let pattern = format!("{{\\{}", field);
    if let Some(start_pos) = content.find(&pattern) {
        let remaining = &content[start_pos + pattern.len()..];
        // Skip any whitespace after the field name
        let trimmed = remaining.trim_start();
        // Find the closing brace
        if let Some(end_pos) = find_matching_brace(trimmed) {
            let value = trimmed[..end_pos].trim().to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

/// Find the position of the closing brace, handling nested braces
fn find_matching_brace(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in s.chars().enumerate() {
        match c {
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_rtf() {
        let rtf = r"{\rtf1\ansi Hello World}";
        let options = ExtractionOptions::default();
        let result = extract_with_metadata(rtf.as_bytes(), &options).unwrap();
        assert!(result.text.contains("Hello World"));
    }

    #[test]
    fn test_rtf_with_formatting() {
        let rtf = r"{\rtf1\ansi\deff0 {\fonttbl{\f0 Times;}}This is \b bold\b0  text.}";
        let options = ExtractionOptions::default();
        let result = extract_with_metadata(rtf.as_bytes(), &options).unwrap();
        assert!(result.text.contains("This is"));
        assert!(result.text.contains("text"));
    }

    #[test]
    fn test_empty_rtf() {
        let rtf = r"{\rtf1\ansi}";
        let options = ExtractionOptions::default();
        let result = extract_with_metadata(rtf.as_bytes(), &options).unwrap();
        assert!(result.text.is_empty() || result.text.trim().is_empty());
    }

    #[test]
    fn test_extract_info_field() {
        let content = r"{\info{\title Test Document}{\author John}}";
        assert_eq!(
            extract_info_field(content, "title"),
            Some("Test Document".to_string())
        );
        assert_eq!(
            extract_info_field(content, "author"),
            Some("John".to_string())
        );
        assert_eq!(extract_info_field(content, "missing"), None);
    }

    #[test]
    fn test_with_metadata() {
        let rtf = r"{\rtf1\ansi{\info{\title My Doc}}Hello}";
        let options = ExtractionOptions {
            include_metadata: true,
            ..Default::default()
        };
        let result = extract_with_metadata(rtf.as_bytes(), &options).unwrap();
        assert!(result.text.contains("Hello"));
        assert!(result.metadata.is_some());
        let meta = result.metadata.unwrap();
        assert_eq!(meta.get("title").and_then(|v| v.as_str()), Some("My Doc"));
    }
}
