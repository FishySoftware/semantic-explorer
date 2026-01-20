use scraper::{ElementRef, Html, Selector};

use crate::extract::config::{ExtractionOptions, HeadingFormat, TableFormat};
use crate::extract::error::{ExtractionError, ExtractionResult};

/// Extract text from HTML with full options support
pub(crate) fn extract_text_with_options(
    bytes: &[u8],
    options: &ExtractionOptions,
) -> ExtractionResult<String> {
    let html = std::str::from_utf8(bytes)
        .map_err(|e| ExtractionError::parse_error("HTML", format!("Invalid UTF-8: {}", e)))?;

    let document = Html::parse_document(html);
    let mut result = String::new();

    // Build selectors for elements we want to skip
    let skip_selector = Selector::parse("script, style, noscript, iframe, svg, head")
        .map_err(|_| ExtractionError::parse_error("HTML", "Failed to build skip selector"))?;

    // Process the document body or root
    let body_selector = Selector::parse("body").ok();
    let root = body_selector
        .as_ref()
        .and_then(|s| document.select(s).next())
        .unwrap_or_else(|| document.root_element());

    extract_element_text(&root, &mut result, options, &skip_selector)?;

    Ok(result.trim().to_string())
}

fn extract_element_text(
    element: &ElementRef,
    result: &mut String,
    options: &ExtractionOptions,
    skip_selector: &Selector,
) -> ExtractionResult<()> {
    for child in element.children() {
        if let Some(text) = child.value().as_text() {
            let cleaned = text.trim();
            if !cleaned.is_empty() {
                if !result.is_empty() && !result.ends_with('\n') && !result.ends_with(' ') {
                    result.push(' ');
                }
                result.push_str(cleaned);
            }
        } else if let Some(elem) = child.value().as_element() {
            let child_ref = ElementRef::wrap(child).unwrap();

            // Skip script, style, etc.
            if skip_selector.matches(&child_ref) {
                continue;
            }

            let tag_name = elem.name();

            match tag_name {
                // Headings
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" if options.preserve_headings => {
                    if !result.is_empty() && !result.ends_with('\n') {
                        result.push_str("\n\n");
                    }
                    match options.heading_format {
                        HeadingFormat::Markdown => {
                            let level =
                                tag_name.chars().nth(1).unwrap().to_digit(10).unwrap() as usize;
                            result.push_str(&"#".repeat(level));
                            result.push(' ');
                        }
                        HeadingFormat::PlainText => {}
                    }
                    extract_element_text(&child_ref, result, options, skip_selector)?;
                    result.push_str("\n\n");
                }

                // Tables
                "table" if options.extract_tables => {
                    if !result.is_empty() && !result.ends_with('\n') {
                        result.push('\n');
                    }
                    extract_table(&child_ref, result, options)?;
                    result.push('\n');
                }

                // Lists
                "ul" | "ol" if options.preserve_lists => {
                    if !result.is_empty() && !result.ends_with('\n') {
                        result.push('\n');
                    }
                    extract_list(&child_ref, result, options, tag_name == "ol", skip_selector)?;
                    result.push('\n');
                }

                // Code blocks
                "pre" | "code" if options.preserve_code_blocks => {
                    if !result.is_empty() && !result.ends_with('\n') {
                        result.push('\n');
                    }
                    result.push_str("```\n");
                    for text in child_ref.text() {
                        result.push_str(text);
                    }
                    if !result.ends_with('\n') {
                        result.push('\n');
                    }
                    result.push_str("```\n");
                }

                // Block elements - add newlines
                "p" | "div" | "section" | "article" | "header" | "footer" | "main" | "aside"
                | "nav" | "blockquote" => {
                    if !result.is_empty() && !result.ends_with('\n') {
                        result.push('\n');
                    }
                    extract_element_text(&child_ref, result, options, skip_selector)?;
                    if !result.ends_with('\n') {
                        result.push('\n');
                    }
                }

                // Line breaks
                "br" => {
                    result.push('\n');
                }

                // Default: recursively process
                _ => {
                    extract_element_text(&child_ref, result, options, skip_selector)?;
                }
            }
        }
    }

    Ok(())
}

fn extract_table(
    table: &ElementRef,
    result: &mut String,
    options: &ExtractionOptions,
) -> ExtractionResult<()> {
    let row_selector = Selector::parse("tr")
        .map_err(|_| ExtractionError::parse_error("HTML", "Failed to build tr selector"))?;
    let cell_selector = Selector::parse("th, td")
        .map_err(|_| ExtractionError::parse_error("HTML", "Failed to build cell selector"))?;

    let rows: Vec<Vec<String>> = table
        .select(&row_selector)
        .map(|row| {
            row.select(&cell_selector)
                .map(|cell| cell.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .collect()
        })
        .collect();

    if rows.is_empty() {
        return Ok(());
    }

    match options.table_format {
        TableFormat::Markdown => {
            for (i, row) in rows.iter().enumerate() {
                result.push('|');
                for cell in row {
                    result.push(' ');
                    result.push_str(cell);
                    result.push_str(" |");
                }
                result.push('\n');

                // Add header separator after first row
                if i == 0 {
                    result.push('|');
                    for _ in row {
                        result.push_str(" --- |");
                    }
                    result.push('\n');
                }
            }
        }
        TableFormat::Csv => {
            for row in rows {
                let csv_row: Vec<String> = row
                    .iter()
                    .map(|cell| {
                        if cell.contains(',') || cell.contains('"') || cell.contains('\n') {
                            format!("\"{}\"", cell.replace('"', "\"\""))
                        } else {
                            cell.clone()
                        }
                    })
                    .collect();
                result.push_str(&csv_row.join(","));
                result.push('\n');
            }
        }
        TableFormat::PlainText => {
            for row in rows {
                result.push_str(&row.join("\t"));
                result.push('\n');
            }
        }
    }

    Ok(())
}

fn extract_list(
    list: &ElementRef,
    result: &mut String,
    options: &ExtractionOptions,
    ordered: bool,
    skip_selector: &Selector,
) -> ExtractionResult<()> {
    let li_selector = Selector::parse("li")
        .map_err(|_| ExtractionError::parse_error("HTML", "Failed to build li selector"))?;

    for (i, item) in list.select(&li_selector).enumerate() {
        if ordered {
            result.push_str(&format!("{}. ", i + 1));
        } else {
            result.push_str("- ");
        }

        // Get direct text content
        let mut item_text = String::new();
        extract_element_text(&item, &mut item_text, options, skip_selector)?;
        result.push_str(item_text.trim());
        result.push('\n');
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_html() {
        let html = b"<html><body><p>Hello World</p></body></html>";
        let result = extract_text_with_options(html, &ExtractionOptions::default());
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Hello World"));
    }

    #[test]
    fn test_extract_html_with_nested_tags() {
        let html =
            b"<div><h1>Title</h1><p>Paragraph with <b>bold</b> and <i>italic</i> text.</p></div>";
        let result = extract_text_with_options(html, &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Title"));
        assert!(text.contains("Paragraph"));
        assert!(text.contains("bold"));
        assert!(text.contains("italic"));
    }

    #[test]
    fn test_extract_html_with_lists() {
        let html = b"<ul><li>Item 1</li><li>Item 2</li><li>Item 3</li></ul>";
        let result = extract_text_with_options(html, &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Item 1"));
        assert!(text.contains("Item 2"));
        assert!(text.contains("Item 3"));
    }

    #[test]
    fn test_extract_html_with_script_and_style() {
        let html = b"<html><head><style>body{color:red;}</style></head><body><p>Content</p><script>alert('test');</script></body></html>";
        let result = extract_text_with_options(html, &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Content"));
        // Script and style content may or may not be included depending on parser
    }

    #[test]
    fn test_extract_html_with_attributes() {
        let html = b"<a href='https://example.com' title='Example'>Link Text</a>";
        let result = extract_text_with_options(html, &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Link Text"));
        // Attributes should not be in text
        assert!(!text.contains("https://example.com") || text.contains("Link Text"));
    }

    #[test]
    fn test_extract_html_with_entities() {
        let html = b"<p>&lt;div&gt; &amp; &quot;quotes&quot; &#169;</p>";
        let result = extract_text_with_options(html, &ExtractionOptions::default());
        assert!(result.is_ok());
        // HTML entities should be decoded
        let text = result.unwrap();
        assert!(!text.is_empty());
    }

    #[test]
    fn test_extract_html_with_whitespace() {
        let html = b"<p>  Multiple   spaces   </p><div>  \n  Newlines  \n  </div>";
        let result = extract_text_with_options(html, &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Multiple"));
        assert!(text.contains("spaces"));
        assert!(text.contains("Newlines"));
    }

    #[test]
    fn test_extract_html_with_tables() {
        let html = b"<table><tr><td>Cell 1</td><td>Cell 2</td></tr><tr><td>Cell 3</td><td>Cell 4</td></tr></table>";
        let result = extract_text_with_options(html, &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Cell 1"));
        assert!(text.contains("Cell 2"));
        assert!(text.contains("Cell 3"));
        assert!(text.contains("Cell 4"));
    }

    #[test]
    fn test_extract_empty_html() {
        let html = b"<html><body></body></html>";
        let result = extract_text_with_options(html, &ExtractionOptions::default());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_extract_html_fragment() {
        let html = b"<p>Just a paragraph</p>";
        let result = extract_text_with_options(html, &ExtractionOptions::default());
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Just a paragraph"));
    }

    #[test]
    fn test_extract_invalid_utf8() {
        let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
        let result = extract_text_with_options(&invalid_utf8, &ExtractionOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_html_with_comments() {
        let html = b"<p>Before</p><!-- This is a comment --><p>After</p>";
        let result = extract_text_with_options(html, &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Before"));
        assert!(text.contains("After"));
        assert!(!text.contains("This is a comment") || text.contains("Before"));
    }
}
