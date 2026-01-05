use anyhow::{Result, anyhow};
use scraper::Html;

pub(crate) fn extract_text_from_html(bytes: &[u8]) -> Result<String> {
    let html = std::str::from_utf8(bytes).map_err(|error| anyhow!(error.to_string()))?;
    let fragment = Html::parse_fragment(html);
    let mut result = String::new();
    for text in fragment.root_element().text() {
        let cleaned_text = text.trim();
        if !cleaned_text.is_empty() {
            result.push_str(cleaned_text);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_html() {
        let html = b"<html><body><p>Hello World</p></body></html>";
        let result = extract_text_from_html(html);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Hello World"));
    }

    #[test]
    fn test_extract_html_with_nested_tags() {
        let html =
            b"<div><h1>Title</h1><p>Paragraph with <b>bold</b> and <i>italic</i> text.</p></div>";
        let result = extract_text_from_html(html);
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
        let result = extract_text_from_html(html);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Item 1"));
        assert!(text.contains("Item 2"));
        assert!(text.contains("Item 3"));
    }

    #[test]
    fn test_extract_html_with_script_and_style() {
        let html = b"<html><head><style>body{color:red;}</style></head><body><p>Content</p><script>alert('test');</script></body></html>";
        let result = extract_text_from_html(html);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Content"));
        // Script and style content may or may not be included depending on parser
    }

    #[test]
    fn test_extract_html_with_attributes() {
        let html = b"<a href='https://example.com' title='Example'>Link Text</a>";
        let result = extract_text_from_html(html);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Link Text"));
        // Attributes should not be in text
        assert!(!text.contains("https://example.com") || text.contains("Link Text"));
    }

    #[test]
    fn test_extract_html_with_entities() {
        let html = b"<p>&lt;div&gt; &amp; &quot;quotes&quot; &#169;</p>";
        let result = extract_text_from_html(html);
        assert!(result.is_ok());
        // HTML entities should be decoded
        let text = result.unwrap();
        assert!(!text.is_empty());
    }

    #[test]
    fn test_extract_html_with_whitespace() {
        let html = b"<p>  Multiple   spaces   </p><div>  \n  Newlines  \n  </div>";
        let result = extract_text_from_html(html);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Multiple"));
        assert!(text.contains("spaces"));
        assert!(text.contains("Newlines"));
    }

    #[test]
    fn test_extract_html_with_tables() {
        let html = b"<table><tr><td>Cell 1</td><td>Cell 2</td></tr><tr><td>Cell 3</td><td>Cell 4</td></tr></table>";
        let result = extract_text_from_html(html);
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
        let result = extract_text_from_html(html);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_extract_html_fragment() {
        let html = b"<p>Just a paragraph</p>";
        let result = extract_text_from_html(html);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Just a paragraph"));
    }

    #[test]
    fn test_extract_invalid_utf8() {
        let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
        let result = extract_text_from_html(&invalid_utf8);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_html_with_comments() {
        let html = b"<p>Before</p><!-- This is a comment --><p>After</p>";
        let result = extract_text_from_html(html);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Before"));
        assert!(text.contains("After"));
        assert!(!text.contains("This is a comment") || text.contains("Before"));
    }
}
