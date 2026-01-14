use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::extract::config::ExtractionOptions;

/// Result of Markdown extraction with text and structure metadata
#[derive(Debug)]
pub struct MarkdownExtractionResult {
    pub text: String,
    pub metadata: Option<Value>,
}

/// Extracted heading information for navigation and chunking
#[derive(Debug, Clone)]
pub struct MarkdownHeading {
    pub level: u8,
    pub text: String,
    pub line_number: usize,
}

/// Extract text from Markdown with full options
pub(crate) fn extract_with_options(
    bytes: &[u8],
    options: &ExtractionOptions,
) -> Result<MarkdownExtractionResult> {
    let content =
        std::str::from_utf8(bytes).map_err(|e| anyhow!("Invalid UTF-8 in Markdown: {}", e))?;

    let mut headings = Vec::new();
    let mut processed_lines = Vec::new();

    // Track code block state
    let mut in_code_block = false;
    let mut code_fence = String::new();

    for (line_num, line) in content.lines().enumerate() {
        // Handle code blocks
        if line.starts_with("```") || line.starts_with("~~~") {
            if !in_code_block {
                in_code_block = true;
                code_fence = if line.starts_with("```") {
                    "```".to_string()
                } else {
                    "~~~".to_string()
                };
                if options.preserve_code_blocks {
                    processed_lines.push(line.to_string());
                }
                continue;
            } else if line.starts_with(&code_fence) {
                in_code_block = false;
                if options.preserve_code_blocks {
                    processed_lines.push(line.to_string());
                }
                continue;
            }
        }

        if in_code_block {
            if options.preserve_code_blocks {
                processed_lines.push(line.to_string());
            }
            continue;
        }

        // Extract headings
        if line.starts_with('#') {
            let level = line.chars().take_while(|c| *c == '#').count() as u8;
            let heading_text = line.trim_start_matches('#').trim().to_string();

            if !heading_text.is_empty() && level <= 6 {
                headings.push(MarkdownHeading {
                    level,
                    text: heading_text.clone(),
                    line_number: line_num + 1,
                });

                if options.preserve_headings {
                    processed_lines.push(line.to_string());
                } else {
                    processed_lines.push(heading_text);
                }
                continue;
            }
        }

        // Handle lists
        let trimmed = line.trim_start();
        if is_list_item(trimmed) {
            if options.preserve_lists {
                processed_lines.push(line.to_string());
            } else {
                // Strip list markers
                let text = strip_list_marker(trimmed);
                if !text.is_empty() {
                    processed_lines.push(text);
                }
            }
            continue;
        }

        // Handle blockquotes
        if trimmed.starts_with('>') {
            let quote_text = trimmed.trim_start_matches('>').trim();
            if options.preserve_formatting {
                processed_lines.push(line.to_string());
            } else {
                processed_lines.push(quote_text.to_string());
            }
            continue;
        }

        // Handle horizontal rules
        if is_horizontal_rule(trimmed) {
            if options.preserve_formatting {
                processed_lines.push(line.to_string());
            }
            continue;
        }

        // Regular paragraph text
        processed_lines.push(line.to_string());
    }

    let text = processed_lines.join("\n");

    let metadata = if options.include_metadata {
        Some(json!({
            "format": "markdown",
            "heading_count": headings.len(),
            "has_code_blocks": content.contains("```") || content.contains("~~~"),
            "has_tables": content.contains("|") && content.lines().any(|l| l.contains("|--")),
            "headings": headings.iter().map(|h| json!({
                "level": h.level,
                "text": h.text,
                "line": h.line_number
            })).collect::<Vec<_>>(),
        }))
    } else {
        None
    };

    Ok(MarkdownExtractionResult { text, metadata })
}

/// Check if line is a list item
fn is_list_item(line: &str) -> bool {
    // Unordered list: -, *, +
    if line.starts_with("- ") || line.starts_with("* ") || line.starts_with("+ ") {
        return true;
    }

    // Ordered list: 1. 2. etc.
    let first_space = line.find(' ');
    if let Some(pos) = first_space {
        let prefix = &line[..pos];
        if let Some(num_part) = prefix.strip_suffix('.')
            && num_part.chars().all(|c| c.is_ascii_digit())
        {
            return true;
        }
    }

    false
}

/// Strip list marker from line
fn strip_list_marker(line: &str) -> String {
    // Unordered
    if line.starts_with("- ") || line.starts_with("* ") || line.starts_with("+ ") {
        return line[2..].trim().to_string();
    }

    // Ordered
    if let Some(pos) = line.find(". ") {
        let prefix = &line[..pos];
        if prefix.chars().all(|c| c.is_ascii_digit()) {
            return line[pos + 2..].trim().to_string();
        }
    }

    line.to_string()
}

/// Check if line is a horizontal rule
fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }

    // ---, ***, ___ (at least 3 chars, optionally with spaces)
    let chars_only: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    if chars_only.len() < 3 {
        return false;
    }

    let first_char = chars_only.chars().next().unwrap();
    matches!(first_char, '-' | '*' | '_') && chars_only.chars().all(|c| c == first_char)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_markdown() {
        let md = b"# Title\n\nSome paragraph text.\n\n## Subtitle\n\nMore content.";
        let result = extract_with_options(md, &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap().text;
        assert!(text.contains("Title"));
        assert!(text.contains("Some paragraph text"));
        assert!(text.contains("Subtitle"));
    }

    #[test]
    fn test_extract_headings() {
        let md = b"# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6";
        let mut options = ExtractionOptions::default();
        options.include_metadata = true;

        let result = extract_with_options(md, &options);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        // Check headings in metadata
        let meta = extraction.metadata.unwrap();
        assert_eq!(meta["heading_count"], 6);
        let headings = meta["headings"].as_array().unwrap();
        assert_eq!(headings[0]["level"], 1);
        assert_eq!(headings[5]["level"], 6);
    }

    #[test]
    fn test_preserve_code_blocks() {
        let md =
            b"Text before\n\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```\n\nText after";
        let mut options = ExtractionOptions::default();
        options.preserve_code_blocks = true;

        let result = extract_with_options(md, &options);
        assert!(result.is_ok());
        let text = result.unwrap().text;
        assert!(text.contains("```rust"));
        assert!(text.contains("fn main()"));
    }

    #[test]
    fn test_strip_code_blocks() {
        let md = b"Text before\n\n```rust\nfn main() {}\n```\n\nText after";
        let mut options = ExtractionOptions::default();
        options.preserve_code_blocks = false;

        let result = extract_with_options(md, &options);
        assert!(result.is_ok());
        let text = result.unwrap().text;
        assert!(!text.contains("```"));
        assert!(!text.contains("fn main()"));
        assert!(text.contains("Text before"));
        assert!(text.contains("Text after"));
    }

    #[test]
    fn test_list_extraction() {
        let md = b"- Item 1\n- Item 2\n- Item 3";
        let mut options = ExtractionOptions::default();
        options.preserve_lists = false;

        let result = extract_with_options(md, &options);
        assert!(result.is_ok());
        let text = result.unwrap().text;
        assert!(text.contains("Item 1"));
        assert!(!text.contains("- Item"));
    }

    #[test]
    fn test_ordered_list() {
        let md = b"1. First\n2. Second\n3. Third";
        let result = extract_with_options(md, &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap().text;
        assert!(text.contains("First"));
        assert!(text.contains("Second"));
    }

    #[test]
    fn test_blockquotes() {
        let md = b"> This is a quote\n> More quote";
        let result = extract_with_options(md, &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap().text;
        assert!(text.contains("This is a quote"));
    }

    #[test]
    fn test_horizontal_rule_detection() {
        assert!(is_horizontal_rule("---"));
        assert!(is_horizontal_rule("***"));
        assert!(is_horizontal_rule("___"));
        assert!(is_horizontal_rule("- - -"));
        assert!(!is_horizontal_rule("--"));
        assert!(!is_horizontal_rule("abc"));
    }

    #[test]
    fn test_metadata_extraction() {
        let md = b"# Main\n\n## Sub\n\n```code```\n\n| a | b |\n|---|---|\n| 1 | 2 |";
        let mut options = ExtractionOptions::default();
        options.include_metadata = true;

        let result = extract_with_options(md, &options);
        assert!(result.is_ok());
        let extraction = result.unwrap();

        let meta = extraction.metadata.unwrap();
        assert_eq!(meta["format"], "markdown");
        assert_eq!(meta["heading_count"], 2);
        assert_eq!(meta["has_code_blocks"], true);
        assert_eq!(meta["has_tables"], true);
    }
}
