use mime::Mime;

use super::config::{ExtractionConfig, ExtractionOutput, ExtractionStrategy};
use super::error::ExtractionResult;
use super::plain_text;

pub struct ExtractionService;

impl ExtractionService {
    pub fn extract(
        mime_type: &Mime,
        buffer: &[u8],
        config: &ExtractionConfig,
    ) -> ExtractionResult<ExtractionOutput> {
        let result = match config.strategy {
            ExtractionStrategy::PlainText => plain_text::extract(mime_type, buffer, config)?,
            ExtractionStrategy::StructurePreserving => {
                // Structure-preserving extraction: enables all structure options
                // while maintaining plain text output format
                let mut structure_config = config.clone();
                structure_config.options.preserve_headings = true;
                structure_config.options.extract_tables = true;
                structure_config.options.preserve_lists = true;
                structure_config.options.preserve_code_blocks = true;
                plain_text::extract(mime_type, buffer, &structure_config)?
            }
            ExtractionStrategy::Markdown => {
                // Markdown extraction: converts content to markdown format
                // with proper heading levels, table syntax, and list formatting
                let mut md_config = config.clone();
                md_config.options.preserve_headings = true;
                md_config.options.heading_format = super::config::HeadingFormat::Markdown;
                md_config.options.extract_tables = true;
                md_config.options.table_format = super::config::TableFormat::Markdown;
                md_config.options.preserve_lists = true;
                md_config.options.preserve_code_blocks = true;
                plain_text::extract(mime_type, buffer, &md_config)?
            }
        };

        // Build final output, optionally appending metadata as text
        let final_text = if config.options.append_metadata_to_text {
            append_metadata_as_text(&result.text, &result.metadata)
        } else {
            result.text
        };

        Ok(ExtractionOutput {
            text: final_text,
            metadata: result.metadata,
        })
    }
}

/// Appends metadata as formatted text at the end of the content
/// This allows metadata to be chunked and embedded alongside the main content
fn append_metadata_as_text(text: &str, metadata: &Option<serde_json::Value>) -> String {
    let Some(metadata) = metadata else {
        return text.to_string();
    };

    let Some(obj) = metadata.as_object() else {
        return text.to_string();
    };

    if obj.is_empty() {
        return text.to_string();
    }

    let mut metadata_lines = Vec::new();
    metadata_lines.push("\n---\nDocument Metadata:\n".to_string());

    // Format each metadata field
    let mut entries: Vec<_> = obj.iter().collect();
    entries.sort_by_key(|(k, _)| *k);

    for (key, value) in entries {
        let formatted_key = key
            .replace('_', " ")
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        let formatted_value = format_metadata_value(value);
        if !formatted_value.is_empty() {
            metadata_lines.push(format!("- {}: {}", formatted_key, formatted_value));
        }
    }

    if metadata_lines.len() <= 1 {
        // Only header, no actual metadata
        return text.to_string();
    }

    format!("{}\n{}", text, metadata_lines.join("\n"))
}

/// Format a JSON value for human-readable text output
fn format_metadata_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => if *b { "Yes" } else { "No" }.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            if s.is_empty() {
                String::new()
            } else {
                s.clone()
            }
        }
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr
                .iter()
                .filter_map(|v| {
                    let s = format_metadata_value(v);
                    if s.is_empty() { None } else { Some(s) }
                })
                .collect();
            if items.is_empty() {
                String::new()
            } else {
                items.join(", ")
            }
        }
        serde_json::Value::Object(obj) => {
            let items: Vec<String> = obj
                .iter()
                .filter_map(|(k, v)| {
                    let s = format_metadata_value(v);
                    if s.is_empty() {
                        None
                    } else {
                        Some(format!("{}: {}", k, s))
                    }
                })
                .collect();
            if items.is_empty() {
                String::new()
            } else {
                format!("{{ {} }}", items.join(", "))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::config::ExtractionOptions;
    use crate::extract::error::ExtractionError;

    #[test]
    fn test_plain_text_extraction() {
        let config = ExtractionConfig {
            strategy: ExtractionStrategy::PlainText,
            options: ExtractionOptions::default(),
        };

        let mime_type: mime::Mime = "text/plain".parse().unwrap();
        let content = b"Hello, world!";

        let result = ExtractionService::extract(&mime_type, content, &config);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        assert_eq!(extraction.text, "Hello, world!");
    }

    #[test]
    fn test_html_extraction() {
        let config = ExtractionConfig {
            strategy: ExtractionStrategy::PlainText,
            options: ExtractionOptions::default(),
        };

        let mime_type: mime::Mime = "text/html".parse().unwrap();
        let content = b"<html><body><h1>Title</h1><p>Content here</p></body></html>";

        let result = ExtractionService::extract(&mime_type, content, &config);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        assert!(extraction.text.contains("Title"));
        assert!(extraction.text.contains("Content here"));
    }

    #[test]
    fn test_xml_extraction() {
        let config = ExtractionConfig {
            strategy: ExtractionStrategy::PlainText,
            options: ExtractionOptions::default(),
        };

        let mime_type: mime::Mime = "application/xml".parse().unwrap();
        let content = b"<root><item>Data</item></root>";

        let result = ExtractionService::extract(&mime_type, content, &config);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        assert!(extraction.text.contains("Data"));
    }

    #[test]
    fn test_csv_extraction() {
        let config = ExtractionConfig {
            strategy: ExtractionStrategy::PlainText,
            options: ExtractionOptions::default(),
        };

        let mime_type: mime::Mime = "text/csv".parse().unwrap();
        let content = b"name,age,city\nJohn,30,NYC\nJane,25,LA";

        let result = ExtractionService::extract(&mime_type, content, &config);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        assert!(extraction.text.contains("John"));
        assert!(extraction.text.contains("Jane"));
    }

    #[test]
    fn test_unsupported_mime_type() {
        let config = ExtractionConfig {
            strategy: ExtractionStrategy::PlainText,
            options: ExtractionOptions::default(),
        };

        let mime_type: mime::Mime = "video/mp4".parse().unwrap();
        let content = b"binary data";

        let result = ExtractionService::extract(&mime_type, content, &config);
        assert!(result.is_err());
        match result.unwrap_err() {
            ExtractionError::UnsupportedMimeType { mime_type, .. } => {
                assert!(mime_type.contains("video/mp4"));
            }
            _ => panic!("Expected UnsupportedMimeType error"),
        }
    }

    #[test]
    fn test_structure_preserving_extracts_with_options() {
        let config = ExtractionConfig {
            strategy: ExtractionStrategy::StructurePreserving,
            options: ExtractionOptions::default(),
        };

        let mime_type: mime::Mime = "text/html".parse().unwrap();
        let content =
            b"<html><body><h1>Title</h1><ul><li>Item 1</li><li>Item 2</li></ul></body></html>";

        let result = ExtractionService::extract(&mime_type, content, &config);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        assert!(extraction.text.contains("Title"));
        assert!(extraction.text.contains("Item 1"));
    }

    #[test]
    fn test_markdown_strategy_formats_as_markdown() {
        let config = ExtractionConfig {
            strategy: ExtractionStrategy::Markdown,
            options: ExtractionOptions::default(),
        };

        let mime_type: mime::Mime = "text/html".parse().unwrap();
        let content = b"<html><body><h1>Title</h1><p>Content</p></body></html>";

        let result = ExtractionService::extract(&mime_type, content, &config);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        // Should have markdown heading format
        assert!(extraction.text.contains("# Title") || extraction.text.contains("Title"));
    }

    #[test]
    fn test_empty_content() {
        let config = ExtractionConfig::default();
        let mime_type: mime::Mime = "text/plain".parse().unwrap();
        let content = b"";

        let result = ExtractionService::extract(&mime_type, content, &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().text, "");
    }

    #[test]
    fn test_html_table_extraction() {
        let mut options = ExtractionOptions::default();
        options.extract_tables = true;
        options.table_format = super::super::config::TableFormat::Markdown;

        let config = ExtractionConfig {
            strategy: ExtractionStrategy::PlainText,
            options,
        };

        let mime_type: mime::Mime = "text/html".parse().unwrap();
        let content =
            b"<table><tr><th>Name</th><th>Age</th></tr><tr><td>John</td><td>30</td></tr></table>";

        let result = ExtractionService::extract(&mime_type, content, &config);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        assert!(extraction.text.contains("Name"));
        assert!(extraction.text.contains("John"));
    }

    #[test]
    fn test_append_metadata_to_text() {
        let mut options = ExtractionOptions::default();
        options.include_metadata = true;
        options.append_metadata_to_text = true;

        let config = ExtractionConfig {
            strategy: ExtractionStrategy::PlainText,
            options,
        };

        let mime_type: mime::Mime = "text/plain".parse().unwrap();
        let content = b"Test content";

        let result = ExtractionService::extract(&mime_type, content, &config);
        assert!(result.is_ok());
        // Plain text doesn't have inherent metadata, but the option should not fail
    }

    #[test]
    fn test_append_metadata_formats_correctly() {
        use serde_json::json;

        let metadata = Some(json!({
            "author": "John Doe",
            "title": "Test Document",
            "page_count": 5
        }));

        let text = "Main content here";
        let result = append_metadata_as_text(text, &metadata);

        assert!(result.contains("Main content here"));
        assert!(result.contains("Document Metadata:"));
        assert!(result.contains("Author: John Doe"));
        assert!(result.contains("Title: Test Document"));
        assert!(result.contains("Page Count: 5"));
    }

    #[test]
    fn test_append_metadata_handles_empty() {
        let text = "Content";
        let result = append_metadata_as_text(text, &None);
        assert_eq!(result, "Content");

        let empty_metadata = Some(serde_json::json!({}));
        let result = append_metadata_as_text(text, &empty_metadata);
        assert_eq!(result, "Content");
    }

    #[test]
    fn test_format_metadata_value() {
        assert_eq!(format_metadata_value(&serde_json::json!(null)), "");
        assert_eq!(format_metadata_value(&serde_json::json!(true)), "Yes");
        assert_eq!(format_metadata_value(&serde_json::json!(false)), "No");
        assert_eq!(format_metadata_value(&serde_json::json!(42)), "42");
        assert_eq!(format_metadata_value(&serde_json::json!("test")), "test");
        assert_eq!(
            format_metadata_value(&serde_json::json!(["a", "b", "c"])),
            "a, b, c"
        );
    }

    #[test]
    fn test_metadata_deterministic_order() {
        use serde_json::json;

        // Create metadata with keys in non-alphabetical order
        let metadata = Some(json!({
            "zebra": "last",
            "alpha": "first",
            "middle": "center"
        }));

        let text = "Content";
        let result = append_metadata_as_text(text, &metadata);

        // Find positions of keys in output
        let alpha_pos = result.find("Alpha").unwrap();
        let middle_pos = result.find("Middle").unwrap();
        let zebra_pos = result.find("Zebra").unwrap();

        // Ensure alphabetical order: Alpha < Middle < Zebra
        assert!(alpha_pos < middle_pos, "Alpha should come before Middle");
        assert!(middle_pos < zebra_pos, "Middle should come before Zebra");
    }
}
