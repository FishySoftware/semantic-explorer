use mime::Mime;
use unicode_normalization::UnicodeNormalization;

use crate::extract::config::{ExtractionConfig, ExtractionOptions};
use crate::extract::error::{ExtractionError, ExtractionResult};
use crate::extract::{
    archive, email, epub, html, json, legacy_doc, legacy_ppt, legacy_xls, log, markdown, office,
    open_office, pdf, rtf, xml,
};

/// Result of text extraction with optional metadata
#[derive(Debug)]
pub struct ExtractedContent {
    pub text: String,
    pub metadata: Option<serde_json::Value>,
}

pub fn extract(
    mime_type: &Mime,
    buffer: &[u8],
    config: &ExtractionConfig,
) -> ExtractionResult<ExtractedContent> {
    let result = match mime_type.type_() {
        mime::APPLICATION => {
            process_application_type(mime_type.subtype().as_str(), buffer, &config.options)
        }
        mime::TEXT => process_text_type(mime_type, buffer, &config.options),
        mime::MESSAGE => {
            process_message_type(mime_type.subtype().as_str(), buffer, &config.options)
        }
        _ => Err(ExtractionError::unsupported_mime(mime_type.to_string())),
    }?;

    let text = clean_text(&result.text);

    Ok(ExtractedContent {
        text,
        metadata: result.metadata,
    })
}

/// Internal extraction result that includes metadata
struct InternalExtraction {
    text: String,
    metadata: Option<serde_json::Value>,
}

impl InternalExtraction {
    fn text_only(text: String) -> Self {
        Self {
            text,
            metadata: None,
        }
    }
}

fn process_application_type(
    sub_type: &str,
    buffer: &[u8],
    options: &ExtractionOptions,
) -> ExtractionResult<InternalExtraction> {
    match sub_type {
        "pdf" => {
            if options.include_metadata {
                let result = pdf::extract_with_metadata(buffer, options)
                    .map_err(|e| ExtractionError::parse_error("PDF", e.to_string()))?;
                Ok(InternalExtraction {
                    text: result.text,
                    metadata: result.metadata,
                })
            } else {
                let text = pdf::extract_text_from_pdf(buffer)
                    .map_err(|e| ExtractionError::parse_error("PDF", e.to_string()))?;
                Ok(InternalExtraction::text_only(text))
            }
        }
        "msword" => {
            // Check if this is a legacy .doc (OLE/CFB format) or modern .docx
            if legacy_doc::is_legacy_doc(buffer) {
                let result = legacy_doc::extract_with_metadata(buffer, options)
                    .map_err(|e| ExtractionError::parse_error("Legacy DOC", e.to_string()))?;
                Ok(InternalExtraction {
                    text: result.text,
                    metadata: result.metadata,
                })
            } else {
                // Try as modern .docx
                let text = office::extract_text_from_document(buffer)
                    .map_err(|e| ExtractionError::parse_error("Word", e.to_string()))?;

                if options.include_metadata {
                    let metadata = office::extract_document_metadata(buffer).ok();
                    Ok(InternalExtraction { text, metadata })
                } else {
                    Ok(InternalExtraction::text_only(text))
                }
            }
        }
        "vnd.openxmlformats-officedocument.wordprocessingml.document"
        | "vnd.openxmlformats-officedocument.wordprocessingml.template"
        | "vnd.ms-word.document.macroEnabled.12"
        | "vnd.ms-word.template.macroEnabled.12" => {
            let text = office::extract_text_from_document(buffer)
                .map_err(|e| ExtractionError::parse_error("Word", e.to_string()))?;

            if options.include_metadata {
                let metadata = office::extract_document_metadata(buffer).ok();
                Ok(InternalExtraction { text, metadata })
            } else {
                Ok(InternalExtraction::text_only(text))
            }
        }
        "vnd.ms-excel" => {
            // Check if this is a legacy .xls (OLE/CFB format) or modern .xlsx
            if legacy_xls::is_legacy_xls(buffer) {
                let result = legacy_xls::extract_with_metadata(buffer, options)
                    .map_err(|e| ExtractionError::parse_error("Legacy XLS", e.to_string()))?;
                Ok(InternalExtraction {
                    text: result.text,
                    metadata: result.metadata,
                })
            } else {
                // Try as modern .xlsx
                let text = office::extract_text_from_spreadsheet(buffer)
                    .map_err(|e| ExtractionError::parse_error("Excel", e.to_string()))?;
                Ok(InternalExtraction::text_only(text))
            }
        }
        "vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        | "vnd.openxmlformats-officedocument.spreadsheetml.template"
        | "vnd.ms-excel.sheet.macroEnabled.12"
        | "vnd.ms-excel.template.macroEnabled.12"
        | "vnd.ms-excel.addin.macroEnabled.12"
        | "vnd.ms-excel.sheet.binary.macroEnabled.12" => {
            let text = office::extract_text_from_spreadsheet(buffer)
                .map_err(|e| ExtractionError::parse_error("Excel", e.to_string()))?;
            Ok(InternalExtraction::text_only(text))
        }
        "mspowerpoint" | "powerpoint" | "vnd.ms-powerpoint" | "x-mspowerpoint" => {
            // Check if this is a legacy .ppt (OLE/CFB format) or modern .pptx
            if legacy_ppt::is_legacy_ppt(buffer) {
                let result = legacy_ppt::extract_with_metadata(buffer, options)
                    .map_err(|e| ExtractionError::parse_error("Legacy PPT", e.to_string()))?;
                Ok(InternalExtraction {
                    text: result.text,
                    metadata: result.metadata,
                })
            } else {
                // Try as modern .pptx
                let text = office::extract_text_from_presentation(buffer)
                    .map_err(|e| ExtractionError::parse_error("PowerPoint", e.to_string()))?;
                Ok(InternalExtraction::text_only(text))
            }
        }
        "vnd.openxmlformats-officedocument.presentationml.presentation" => {
            let text = office::extract_text_from_presentation(buffer)
                .map_err(|e| ExtractionError::parse_error("PowerPoint", e.to_string()))?;
            Ok(InternalExtraction::text_only(text))
        }
        "vnd.oasis.opendocument.text" => {
            let text = open_office::extract_text_from_document(buffer)
                .map_err(|e| ExtractionError::parse_error("ODT", e.to_string()))?;

            if options.include_metadata {
                let metadata = open_office::extract_document_metadata(buffer).ok();
                Ok(InternalExtraction { text, metadata })
            } else {
                Ok(InternalExtraction::text_only(text))
            }
        }
        "vnd.oasis.opendocument.spreadsheet" => {
            let text = open_office::extract_text_from_spreadsheet(buffer)
                .map_err(|e| ExtractionError::parse_error("ODS", e.to_string()))?;
            Ok(InternalExtraction::text_only(text))
        }
        "vnd.oasis.opendocument.presentation" => {
            let text = open_office::extract_text_from_presentation(buffer)
                .map_err(|e| ExtractionError::parse_error("ODP", e.to_string()))?;
            Ok(InternalExtraction::text_only(text))
        }
        "xml" => {
            let text = xml::extract_text_from_xml(buffer)
                .map_err(|e| ExtractionError::parse_error("XML", e.to_string()))?;
            Ok(InternalExtraction::text_only(text))
        }
        "json" => {
            let result = json::extract_with_options(buffer, options)
                .map_err(|e| ExtractionError::parse_error("JSON", e.to_string()))?;
            Ok(InternalExtraction {
                text: result.text,
                metadata: result.metadata,
            })
        }
        "x-ndjson" | "ndjson" | "x-jsonlines" | "jsonlines" => {
            let result = json::extract_ndjson_with_options(buffer, options)
                .map_err(|e| ExtractionError::parse_error("NDJSON", e.to_string()))?;
            Ok(InternalExtraction {
                text: result.text,
                metadata: result.metadata,
            })
        }
        "html" => {
            let text = html::extract_text_with_options(buffer, options)?;
            Ok(InternalExtraction::text_only(text))
        }
        "zip" | "x-zip-compressed" => {
            let archive_opts = archive::ArchiveOptions::default();
            let result = archive::extract_from_zip(buffer, options, &archive_opts)
                .map_err(|e| ExtractionError::archive_error("ZIP", e.to_string()))?;
            Ok(InternalExtraction {
                text: result.text,
                metadata: result.metadata,
            })
        }
        "gzip" | "x-gzip" => {
            // Check if it might be tar.gz based on content
            let archive_opts = archive::ArchiveOptions::default();
            match archive::extract_from_tar_gz(buffer, options, &archive_opts) {
                Ok(result) => Ok(InternalExtraction {
                    text: result.text,
                    metadata: result.metadata,
                }),
                Err(_) => {
                    // Fall back to simple gzip decompression
                    let decompressed = archive::extract_from_gzip(buffer, options)
                        .map_err(|e| ExtractionError::archive_error("GZIP", e.to_string()))?;
                    let text = String::from_utf8_lossy(&decompressed).to_string();
                    Ok(InternalExtraction::text_only(text))
                }
            }
        }
        "x-tar" | "tar" => {
            // Uncompressed tar - not yet implemented
            Err(ExtractionError::unsupported_mime_with_context(
                format!("application/{}", sub_type),
                "uncompressed tar not yet supported",
            ))
        }
        // RTF (Rich Text Format)
        "rtf" | "x-rtf" => {
            let result = rtf::extract_with_metadata(buffer, options)
                .map_err(|e| ExtractionError::parse_error("RTF", e.to_string()))?;
            Ok(InternalExtraction {
                text: result.text,
                metadata: result.metadata,
            })
        }
        // EPUB (Electronic Publication)
        "epub+zip" | "x-epub+zip" => {
            let result = epub::extract_with_metadata(buffer, options)
                .map_err(|e| ExtractionError::parse_error("EPUB", e.to_string()))?;
            Ok(InternalExtraction {
                text: result.text,
                metadata: result.metadata,
            })
        }
        // Email formats
        "vnd.ms-outlook" => {
            // .msg format - requires different parser, mark as unsupported for now
            Err(ExtractionError::unsupported_mime_with_context(
                "application/vnd.ms-outlook".to_string(),
                "Outlook .msg format requires separate parser",
            ))
        }
        _ => Err(ExtractionError::unsupported_mime_with_context(
            format!("application/{}", sub_type),
            "unsupported application subtype",
        )),
    }
}

fn process_text_type(
    content_type: &Mime,
    buffer: &[u8],
    options: &ExtractionOptions,
) -> ExtractionResult<InternalExtraction> {
    match content_type.subtype().as_str() {
        "plain" => {
            // Check if plain text is actually a log file
            let content = String::from_utf8_lossy(buffer);
            if log::is_log_file(&content) {
                let result = log::extract_with_options(buffer, options)
                    .map_err(|e| ExtractionError::parse_error("Log", e.to_string()))?;
                return Ok(InternalExtraction {
                    text: result.text,
                    metadata: result.metadata,
                });
            }
            Ok(InternalExtraction::text_only(content.to_string()))
        }
        "csv" => Ok(InternalExtraction::text_only(
            String::from_utf8_lossy(buffer).to_string(),
        )),
        "xml" => {
            let text = xml::extract_text_from_xml(buffer)
                .map_err(|e| ExtractionError::parse_error("XML", e.to_string()))?;
            Ok(InternalExtraction::text_only(text))
        }
        "html" => {
            let text = html::extract_text_with_options(buffer, options)?;
            Ok(InternalExtraction::text_only(text))
        }
        "markdown" | "x-markdown" => {
            let result = markdown::extract_with_options(buffer, options)
                .map_err(|e| ExtractionError::parse_error("Markdown", e.to_string()))?;
            Ok(InternalExtraction {
                text: result.text,
                metadata: result.metadata,
            })
        }
        "x-log" | "x-syslog" => {
            let result = log::extract_with_options(buffer, options)
                .map_err(|e| ExtractionError::parse_error("Log", e.to_string()))?;
            Ok(InternalExtraction {
                text: result.text,
                metadata: result.metadata,
            })
        }
        "json" => {
            let result = json::extract_with_options(buffer, options)
                .map_err(|e| ExtractionError::parse_error("JSON", e.to_string()))?;
            Ok(InternalExtraction {
                text: result.text,
                metadata: result.metadata,
            })
        }
        "x-ndjson" | "ndjson" => {
            let result = json::extract_ndjson_with_options(buffer, options)
                .map_err(|e| ExtractionError::parse_error("NDJSON", e.to_string()))?;
            Ok(InternalExtraction {
                text: result.text,
                metadata: result.metadata,
            })
        }
        "rtf" | "richtext" => {
            let result = rtf::extract_with_metadata(buffer, options)
                .map_err(|e| ExtractionError::parse_error("RTF", e.to_string()))?;
            Ok(InternalExtraction {
                text: result.text,
                metadata: result.metadata,
            })
        }
        _ => Err(ExtractionError::unsupported_mime_with_context(
            content_type.to_string(),
            "unsupported text subtype",
        )),
    }
}

fn process_message_type(
    sub_type: &str,
    buffer: &[u8],
    options: &ExtractionOptions,
) -> ExtractionResult<InternalExtraction> {
    match sub_type {
        "rfc822" => {
            // .eml email format
            let email_opts = email::EmailOptions::default();
            let result = email::extract_with_options(buffer, options, &email_opts)
                .map_err(|e| ExtractionError::parse_error("Email", e.to_string()))?;
            Ok(InternalExtraction {
                text: result.text,
                metadata: result.metadata,
            })
        }
        _ => Err(ExtractionError::unsupported_mime_with_context(
            format!("message/{}", sub_type),
            "unsupported message subtype",
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
