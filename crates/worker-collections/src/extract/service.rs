use anyhow::Result;
use mime::Mime;

use super::config::{ExtractionConfig, ExtractionResult, ExtractionStrategy};
use super::strategies;

pub struct ExtractionService;

impl ExtractionService {
    pub fn extract(
        mime_type: &Mime,
        buffer: &[u8],
        config: &ExtractionConfig,
    ) -> Result<ExtractionResult> {
        match config.strategy {
            ExtractionStrategy::PlainText => {
                strategies::plain_text::extract(mime_type, buffer, config)
            }
            ExtractionStrategy::StructurePreserving => {
                // Phase 4: implement structure preserving
                // For now, fall back to plain text
                strategies::plain_text::extract(mime_type, buffer, config)
            }
            ExtractionStrategy::Markdown => {
                // Phase 4: implement markdown conversion
                // For now, fall back to plain text
                strategies::plain_text::extract(mime_type, buffer, config)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::config::ExtractionOptions;

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
    }

    #[test]
    fn test_structure_preserving_fallback() {
        let config = ExtractionConfig {
            strategy: ExtractionStrategy::StructurePreserving,
            options: ExtractionOptions::default(),
        };

        let mime_type: mime::Mime = "text/plain".parse().unwrap();
        let content = b"Test content";

        // Should fall back to plain text for now
        let result = ExtractionService::extract(&mime_type, content, &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().text, "Test content");
    }

    #[test]
    fn test_markdown_fallback() {
        let config = ExtractionConfig {
            strategy: ExtractionStrategy::Markdown,
            options: ExtractionOptions::default(),
        };

        let mime_type: mime::Mime = "text/plain".parse().unwrap();
        let content = b"Markdown content";

        // Should fall back to plain text for now
        let result = ExtractionService::extract(&mime_type, content, &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().text, "Markdown content");
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
}
