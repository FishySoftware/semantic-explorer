//! EPUB (Electronic Publication) text extraction.
//!
//! This module extracts plain text from EPUB ebook files.

use anyhow::{Context, Result};
use epub::doc::EpubDoc;
use serde_json::{Value, json};
use std::io::Cursor;

use crate::extract::config::ExtractionOptions;

/// Result of EPUB extraction
#[derive(Debug)]
pub struct EpubExtractionResult {
    /// Extracted plain text content
    pub text: String,
    /// Document metadata
    pub metadata: Option<Value>,
}

/// Extract text with metadata from EPUB
pub fn extract_with_metadata(
    bytes: &[u8],
    options: &ExtractionOptions,
) -> Result<EpubExtractionResult> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut doc = EpubDoc::from_reader(cursor).context("Failed to parse EPUB document")?;

    // Extract text
    let mut all_text = Vec::new();
    let spine = doc.spine.clone();
    for item in spine {
        if let Some((content_bytes, _mime)) = doc.get_resource(&item.idref) {
            let content = String::from_utf8_lossy(&content_bytes);
            let cleaned = strip_html_tags(&content);
            if !cleaned.trim().is_empty() {
                all_text.push(cleaned);
            }
        }
    }
    let text = all_text.join("\n\n");

    let metadata = if options.include_metadata {
        Some(extract_epub_metadata(&mut doc))
    } else {
        None
    };

    Ok(EpubExtractionResult { text, metadata })
}

/// Extract metadata from EPUB document
fn extract_epub_metadata(doc: &mut EpubDoc<Cursor<Vec<u8>>>) -> Value {
    let mut metadata = serde_json::Map::new();

    // Title
    if let Some(item) = doc.mdata("title") {
        metadata.insert("title".to_string(), json!(item.value));
    }

    // Author/Creator
    if let Some(item) = doc.mdata("creator") {
        metadata.insert("author".to_string(), json!(item.value));
    }

    // Publisher
    if let Some(item) = doc.mdata("publisher") {
        metadata.insert("publisher".to_string(), json!(item.value));
    }

    // Language
    if let Some(item) = doc.mdata("language") {
        metadata.insert("language".to_string(), json!(item.value));
    }

    // Description
    if let Some(item) = doc.mdata("description") {
        metadata.insert("description".to_string(), json!(item.value));
    }

    // Subject
    if let Some(item) = doc.mdata("subject") {
        metadata.insert("subject".to_string(), json!(item.value));
    }

    // Rights
    if let Some(item) = doc.mdata("rights") {
        metadata.insert("rights".to_string(), json!(item.value));
    }

    // Date
    if let Some(item) = doc.mdata("date") {
        metadata.insert("date".to_string(), json!(item.value));
    }

    // Identifier (ISBN, etc.)
    if let Some(item) = doc.mdata("identifier") {
        metadata.insert("identifier".to_string(), json!(item.value));
    }

    // Chapter count
    metadata.insert("chapter_count".to_string(), json!(doc.spine.len()));

    // Resource count
    metadata.insert("resource_count".to_string(), json!(doc.resources.len()));

    json!(metadata)
}

/// Strip HTML tags from content to get plain text
fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let mut in_script_or_style = false;
    let mut tag_name = String::new();

    let chars: Vec<char> = html.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c == '<' {
            in_tag = true;
            tag_name.clear();
            i += 1;
            continue;
        }

        if in_tag {
            if c == '>' {
                in_tag = false;
                let lower_tag = tag_name.to_lowercase();

                // Check for script/style start
                if lower_tag == "script" || lower_tag == "style" {
                    in_script_or_style = true;
                }
                // Check for script/style end
                if lower_tag == "/script" || lower_tag == "/style" {
                    in_script_or_style = false;
                }

                // Add whitespace for block elements
                if is_block_element(&lower_tag) && !result.ends_with('\n') {
                    result.push('\n');
                }
            } else if !c.is_whitespace() {
                tag_name.push(c);
            }
            i += 1;
            continue;
        }

        if !in_script_or_style {
            // Handle HTML entities
            if c == '&'
                && let Some((entity, skip)) = parse_html_entity(&chars[i..])
            {
                result.push_str(&entity);
                i += skip;
                continue;
            }
            result.push(c);
        }

        i += 1;
    }

    // Clean up whitespace
    clean_whitespace(&result)
}

/// Check if a tag is a block element
fn is_block_element(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "/p"
            | "div"
            | "/div"
            | "br"
            | "br/"
            | "h1"
            | "/h1"
            | "h2"
            | "/h2"
            | "h3"
            | "/h3"
            | "h4"
            | "/h4"
            | "h5"
            | "/h5"
            | "h6"
            | "/h6"
            | "li"
            | "/li"
            | "tr"
            | "/tr"
            | "td"
            | "/td"
            | "th"
            | "/th"
            | "blockquote"
            | "/blockquote"
            | "pre"
            | "/pre"
            | "section"
            | "/section"
            | "article"
            | "/article"
            | "header"
            | "/header"
            | "footer"
            | "/footer"
    )
}

/// Parse common HTML entities
fn parse_html_entity(chars: &[char]) -> Option<(String, usize)> {
    let mut entity = String::new();
    entity.push('&');

    for (i, &c) in chars.iter().skip(1).enumerate() {
        if c == ';' {
            entity.push(';');
            let decoded = match entity.as_str() {
                "&amp;" => "&",
                "&lt;" => "<",
                "&gt;" => ">",
                "&quot;" => "\"",
                "&apos;" => "'",
                "&nbsp;" => " ",
                "&mdash;" => "\u{2014}",
                "&ndash;" => "\u{2013}",
                "&ldquo;" => "\u{201C}",
                "&rdquo;" => "\u{201D}",
                "&lsquo;" => "\u{2018}",
                "&rsquo;" => "\u{2019}",
                "&hellip;" => "\u{2026}",
                "&copy;" => "\u{00A9}",
                "&reg;" => "\u{00AE}",
                "&trade;" => "\u{2122}",
                _ => {
                    // Try numeric entity
                    if entity.starts_with("&#")
                        && let Some(decoded) = decode_numeric_entity(&entity)
                    {
                        return Some((decoded, i + 2));
                    }
                    return None;
                }
            };
            return Some((decoded.to_string(), i + 2));
        }

        if !c.is_alphanumeric() && c != '#' {
            return None;
        }

        entity.push(c);

        if i > 10 {
            return None; // Entity too long
        }
    }
    None
}

/// Decode numeric HTML entities (&#123; or &#x7B;)
fn decode_numeric_entity(entity: &str) -> Option<String> {
    let inner = entity.strip_prefix("&#")?.strip_suffix(';')?;

    let codepoint = if let Some(hex) = inner.strip_prefix('x').or_else(|| inner.strip_prefix('X')) {
        u32::from_str_radix(hex, 16).ok()?
    } else {
        inner.parse::<u32>().ok()?
    };

    char::from_u32(codepoint).map(|c| c.to_string())
}

/// Clean up excessive whitespace
fn clean_whitespace(text: &str) -> String {
    let mut result = String::new();
    let mut prev_whitespace = false;
    let mut prev_newline = false;

    for c in text.chars() {
        if c == '\n' {
            if !prev_newline {
                result.push('\n');
            }
            prev_newline = true;
            prev_whitespace = true;
        } else if c.is_whitespace() {
            if !prev_whitespace {
                result.push(' ');
            }
            prev_whitespace = true;
        } else {
            result.push(c);
            prev_whitespace = false;
            prev_newline = false;
        }
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_simple() {
        let html = "<p>Hello <b>World</b></p>";
        let result = strip_html_tags(html);
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
        assert!(!result.contains("<"));
    }

    #[test]
    fn test_strip_html_entities() {
        let html = "<p>Tom &amp; Jerry</p>";
        let result = strip_html_tags(html);
        assert!(result.contains("Tom & Jerry"));
    }

    #[test]
    fn test_strip_html_script() {
        let html = "<p>Text</p><script>alert('hi');</script><p>More</p>";
        let result = strip_html_tags(html);
        assert!(result.contains("Text"));
        assert!(result.contains("More"));
        assert!(!result.contains("alert"));
    }

    #[test]
    fn test_clean_whitespace() {
        let text = "Hello   World\n\n\nNew paragraph";
        let result = clean_whitespace(text);
        assert_eq!(result, "Hello World\nNew paragraph");
    }

    #[test]
    fn test_decode_numeric_entity() {
        assert_eq!(decode_numeric_entity("&#65;"), Some("A".to_string()));
        assert_eq!(decode_numeric_entity("&#x41;"), Some("A".to_string()));
        assert_eq!(decode_numeric_entity("&#8212;"), Some("—".to_string()));
    }

    #[test]
    fn test_is_block_element() {
        assert!(is_block_element("p"));
        assert!(is_block_element("/p"));
        assert!(is_block_element("div"));
        assert!(is_block_element("br"));
        assert!(!is_block_element("span"));
        assert!(!is_block_element("a"));
    }
}
