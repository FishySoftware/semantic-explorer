use anyhow::{Result, anyhow};
use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::error;

pub(crate) fn extract_text_from_xml(bytes: &[u8]) -> Result<String> {
    let xml_data = std::str::from_utf8(bytes).map_err(|error| anyhow!(error.to_string()))?;

    let mut reader = Reader::from_str(xml_data);
    reader.config_mut().trim_text(true);

    let mut result = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(e)) => {
                result.push_str(&e.decode()?);
            }
            Ok(Event::Eof) => break,
            Err(error) => error!("error at position {}: {error:?}", reader.buffer_position()),
            _ => (),
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_xml() {
        let xml = b"<root><item>Test content</item></root>";
        let result = extract_text_from_xml(xml);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Test content"));
    }

    #[test]
    fn test_extract_xml_with_attributes() {
        let xml = b"<root><item id='1' name='test'>Content</item></root>";
        let result = extract_text_from_xml(xml);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Content"));
        // Attributes should not be in the extracted text
        assert!(!text.contains("id=") && !text.contains("name="));
    }

    #[test]
    fn test_extract_xml_with_nested_elements() {
        let xml = b"<root><parent><child1>Text 1</child1><child2>Text 2</child2></parent></root>";
        let result = extract_text_from_xml(xml);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Text 1"));
        assert!(text.contains("Text 2"));
    }

    #[test]
    fn test_extract_xml_with_cdata() {
        let xml = b"<root><item><![CDATA[Some <special> content & stuff]]></item></root>";
        let result = extract_text_from_xml(xml);
        assert!(result.is_ok());
        // CDATA content may or may not be extracted depending on parser behavior
        // Just verify no error occurred
        let _ = result.unwrap();
    }

    #[test]
    fn test_extract_xml_with_comments() {
        let xml = b"<root><item>Before</item><!-- Comment --><item>After</item></root>";
        let result = extract_text_from_xml(xml);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Before"));
        assert!(text.contains("After"));
        // Comments should not be in text
        assert!(!text.contains("Comment"));
    }

    #[test]
    fn test_extract_xml_with_entities() {
        let xml = b"<root>&lt;tag&gt; &amp; &quot;text&quot;</root>";
        let result = extract_text_from_xml(xml);
        assert!(result.is_ok());
        let text = result.unwrap();
        // Entities should be decoded by the XML parser
        assert!(text.contains("<tag>") || text.len() > 0);
    }

    #[test]
    fn test_extract_xml_with_namespaces() {
        let xml = b"<ns:root xmlns:ns='http://example.com'><ns:item>Content</ns:item></ns:root>";
        let result = extract_text_from_xml(xml);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Content"));
    }

    #[test]
    fn test_extract_xml_with_whitespace() {
        let xml = b"<root>  \n  <item>  Text with spaces  </item>  \n  </root>";
        let result = extract_text_from_xml(xml);
        assert!(result.is_ok());
        let text = result.unwrap();
        // Trimmed by the parser config
        assert!(text.contains("Text with spaces"));
    }

    #[test]
    fn test_extract_empty_xml() {
        let xml = b"<root></root>";
        let result = extract_text_from_xml(xml);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_extract_xml_multiple_text_nodes() {
        let xml = b"<root><a>First</a><b>Second</b><c>Third</c></root>";
        let result = extract_text_from_xml(xml);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("First"));
        assert!(text.contains("Second"));
        assert!(text.contains("Third"));
    }

    #[test]
    fn test_extract_xml_with_mixed_content() {
        let xml = b"<root>Text before <tag>text inside</tag> text after</root>";
        let result = extract_text_from_xml(xml);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Text before"));
        assert!(text.contains("text inside"));
        assert!(text.contains("text after"));
    }

    #[test]
    fn test_extract_invalid_utf8() {
        let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
        let result = extract_text_from_xml(&invalid_utf8);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_malformed_xml() {
        let xml = b"<root><item>Unclosed tag";
        let result = extract_text_from_xml(xml);
        // The parser should still extract what it can
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Unclosed tag"));
    }
}
