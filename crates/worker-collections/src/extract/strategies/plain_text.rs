use anyhow::{Result, anyhow};
use mime::Mime;
use unicode_normalization::UnicodeNormalization;

use crate::extract::config::{ExtractionConfig, ExtractionResult};
use crate::extract::{html, office, open_office, pdf, xml};

pub fn extract(
    mime_type: &Mime,
    buffer: &[u8],
    _config: &ExtractionConfig,
) -> Result<ExtractionResult> {
    let raw_text = match mime_type.type_() {
        mime::APPLICATION => process_application_type(mime_type.subtype().as_str(), buffer),
        mime::TEXT => process_text_type(mime_type, buffer),
        _ => Err(anyhow!("unsupported content type: {}", mime_type)),
    }?;

    let text = clean_text(&raw_text);

    Ok(ExtractionResult {
        text,
        metadata: None,
    })
}

fn process_application_type(sub_type: &str, buffer: &[u8]) -> Result<String> {
    match sub_type {
        "pdf" => Ok(pdf::extract_text_from_pdf(buffer)?),
        "msword"
        | "vnd.openxmlformats-officedocument.wordprocessingml.document"
        | "vnd.openxmlformats-officedocument.wordprocessingml.template"
        | "vnd.ms-word.document.macroEnabled.12"
        | "vnd.ms-word.template.macroEnabled.12" => Ok(office::extract_text_from_document(buffer)?),
        "vnd.ms-excel"
        | "vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        | "vnd.openxmlformats-officedocument.spreadsheetml.template"
        | "vnd.ms-excel.sheet.macroEnabled.12"
        | "vnd.ms-excel.template.macroEnabled.12"
        | "vnd.ms-excel.addin.macroEnabled.12"
        | "vnd.ms-excel.sheet.binary.macroEnabled.12" => {
            Ok(office::extract_text_from_spreadsheet(buffer)?)
        }
        "mspowerpoint"
        | "powerpoint"
        | "vnd.ms-powerpoint"
        | "x-mspowerpoint"
        | "vnd.openxmlformats-officedocument.presentationml.presentation" => {
            Ok(office::extract_text_from_presentation(buffer)?)
        }
        "vnd.oasis.opendocument.text" => Ok(open_office::extract_text_from_document(buffer)?),
        "vnd.oasis.opendocument.spreadsheet" => {
            Ok(open_office::extract_text_from_spreadsheet(buffer)?)
        }
        "vnd.oasis.opendocument.presentation" => {
            Ok(open_office::extract_text_from_presentation(buffer)?)
        }
        "xml" => Ok(xml::extract_text_from_xml(buffer)?),
        "html" => Ok(html::extract_text_from_html(buffer)?),
        _ => Err(anyhow!("unsupported application subtype: {}", sub_type)),
    }
}

fn process_text_type(content_type: &Mime, buffer: &[u8]) -> Result<String> {
    match content_type.subtype() {
        mime::PLAIN | mime::CSV => Ok(String::from_utf8_lossy(buffer).to_string()),
        mime::XML => Ok(xml::extract_text_from_xml(buffer)?),
        mime::HTML => Ok(html::extract_text_from_html(buffer)?),
        _ => Err(anyhow!(
            "unsupported text subtype: {}",
            content_type.subtype()
        )),
    }
}

fn clean_text(raw_text: &str) -> String {
    let normalized = raw_text.nfc().collect::<String>();
    let without_controls: String = normalized
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t' | '\r'))
        .collect();
    let lines: Vec<String> = without_controls
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect();
    lines.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::config::{ExtractionConfig, ExtractionStrategy};

    fn create_default_config() -> ExtractionConfig {
        ExtractionConfig {
            strategy: ExtractionStrategy::PlainText,
            ..Default::default()
        }
    }

    #[test]
    fn test_extract_plain_text() {
        let content = b"Hello, World! This is a test.";
        let mime_type: mime::Mime = "text/plain".parse().unwrap();
        let config = create_default_config();

        let result = extract(&mime_type, content, &config);
        assert!(result.is_ok());

        let extraction = result.unwrap();
        assert_eq!(extraction.text, "Hello, World! This is a test.");
    }

    #[test]
    fn test_extract_text_with_unicode() {
        let content = "Hello 世界! Testing émojis 🚀 and ñoño".as_bytes();
        let mime_type: mime::Mime = "text/plain".parse().unwrap();
        let config = create_default_config();

        let result = extract(&mime_type, content, &config);
        assert!(result.is_ok());

        let extraction = result.unwrap();
        assert_eq!(extraction.text, "Hello 世界! Testing émojis 🚀 and ñoño");
    }

    #[test]
    fn test_extract_text_with_multiple_lines() {
        let content = b"Line 1\nLine 2\nLine 3";
        let mime_type: mime::Mime = "text/plain".parse().unwrap();
        let config = create_default_config();

        let result = extract(&mime_type, content, &config);
        assert!(result.is_ok());

        let extraction = result.unwrap();
        assert_eq!(extraction.text, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_extract_text_with_extra_whitespace() {
        let content = b"Line 1     \n\n\nLine 2\t\t\tLine 3   ";
        let mime_type: mime::Mime = "text/plain".parse().unwrap();
        let config = create_default_config();

        let result = extract(&mime_type, content, &config);
        assert!(result.is_ok());

        let extraction = result.unwrap();
        // Should normalize whitespace
        assert_eq!(extraction.text, "Line 1\nLine 2 Line 3");
    }

    #[test]
    fn test_extract_csv() {
        let content = b"Name,Age,City\nJohn,30,NYC\nJane,25,LA";
        let mime_type: mime::Mime = "text/csv".parse().unwrap();
        let config = create_default_config();

        let result = extract(&mime_type, content, &config);
        assert!(result.is_ok());

        let extraction = result.unwrap();
        assert!(extraction.text.contains("Name"));
        assert!(extraction.text.contains("John"));
    }

    #[test]
    fn test_extract_html() {
        let content = b"<html><body><p>Hello, <b>World</b>!</p></body></html>";
        let mime_type: mime::Mime = "text/html".parse().unwrap();
        let config = create_default_config();

        let result = extract(&mime_type, content, &config);
        assert!(result.is_ok());

        let extraction = result.unwrap();
        // HTML tags should be stripped
        assert!(extraction.text.contains("Hello"));
        assert!(extraction.text.contains("World"));
        assert!(!extraction.text.contains("<p>"));
        assert!(!extraction.text.contains("<b>"));
    }

    #[test]
    fn test_extract_xml() {
        let content = b"<root><item>Text 1</item><item>Text 2</item></root>";
        let mime_type: mime::Mime = "text/xml".parse().unwrap();
        let config = create_default_config();

        let result = extract(&mime_type, content, &config);
        assert!(result.is_ok());

        let extraction = result.unwrap();
        assert!(extraction.text.contains("Text 1"));
        assert!(extraction.text.contains("Text 2"));
    }

    #[test]
    fn test_extract_application_xml() {
        let content = b"<root><item>Application XML</item></root>";
        let mime_type: mime::Mime = "application/xml".parse().unwrap();
        let config = create_default_config();

        let result = extract(&mime_type, content, &config);
        assert!(result.is_ok());

        let extraction = result.unwrap();
        assert!(extraction.text.contains("Application XML"));
    }

    #[test]
    fn test_extract_application_html() {
        let content = b"<html><body><h1>Title</h1><p>Content</p></body></html>";
        let mime_type: mime::Mime = "application/html".parse().unwrap();
        let config = create_default_config();

        let result = extract(&mime_type, content, &config);
        assert!(result.is_ok());

        let extraction = result.unwrap();
        assert!(extraction.text.contains("Title"));
        assert!(extraction.text.contains("Content"));
    }

    #[test]
    fn test_extract_empty_content() {
        let content = b"";
        let mime_type: mime::Mime = "text/plain".parse().unwrap();
        let config = create_default_config();

        let result = extract(&mime_type, content, &config);
        assert!(result.is_ok());

        let extraction = result.unwrap();
        assert_eq!(extraction.text, "");
    }

    #[test]
    fn test_extract_unsupported_mime_type() {
        let content = b"Some binary data";
        let mime_type: mime::Mime = "application/octet-stream".parse().unwrap();
        let config = create_default_config();

        let result = extract(&mime_type, content, &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unsupported"));
    }

    #[test]
    fn test_extract_control_characters() {
        let content = b"Hello\x00World\x01Test\x02";
        let mime_type: mime::Mime = "text/plain".parse().unwrap();
        let config = create_default_config();

        let result = extract(&mime_type, content, &config);
        assert!(result.is_ok());

        let extraction = result.unwrap();
        // Control characters should be filtered out (except newline, tab, carriage return)
        assert!(!extraction.text.contains('\x00'));
        assert!(!extraction.text.contains('\x01'));
        assert!(!extraction.text.contains('\x02'));
    }

    #[test]
    fn test_extract_tabs_and_newlines_preserved() {
        let content = b"Line1\tTabbed\nLine2\rLine3\r\nLine4";
        let mime_type: mime::Mime = "text/plain".parse().unwrap();
        let config = create_default_config();

        let result = extract(&mime_type, content, &config);
        assert!(result.is_ok());

        let extraction = result.unwrap();
        // Tabs, newlines, and carriage returns should be normalized
        assert!(extraction.text.contains("Line1"));
        assert!(extraction.text.contains("Line2"));
    }
}
