use anyhow::{Result, anyhow};
use pdf_extract::extract_text_from_mem;
use serde_json::json;

use crate::extract::config::ExtractionOptions;

/// Result of PDF extraction with text and metadata
#[derive(Debug)]
pub struct PdfExtractionResult {
    pub text: String,
    pub metadata: Option<serde_json::Value>,
}

/// Extract text from a PDF document
pub(crate) fn extract_text_from_pdf(bytes: &[u8]) -> Result<String> {
    extract_text_from_mem(bytes).map_err(|error| anyhow!(error.to_string()))
}

/// Extract text and metadata from a PDF document
pub(crate) fn extract_with_metadata(
    bytes: &[u8],
    options: &ExtractionOptions,
) -> Result<PdfExtractionResult> {
    let text = extract_text_from_mem(bytes).map_err(|error| anyhow!(error.to_string()))?;

    // Try to extract metadata if requested
    let metadata = if options.include_metadata {
        extract_pdf_metadata(bytes).ok()
    } else {
        None
    };

    Ok(PdfExtractionResult { text, metadata })
}

/// Extract metadata from PDF using lopdf
fn extract_pdf_metadata(bytes: &[u8]) -> Result<serde_json::Value> {
    use std::io::Cursor;

    let doc = lopdf::Document::load_from(Cursor::new(bytes))
        .map_err(|e| anyhow!("Failed to parse PDF: {}", e))?;

    let mut metadata = serde_json::Map::new();

    // Try to get document info from the trailer
    if let Ok(info_ref) = doc.trailer.get(b"Info")
        && let Ok(info_ref) = info_ref.as_reference()
        && let Ok(info_dict) = doc.get_dictionary(info_ref)
    {
        // Extract common metadata fields
        for (key, display_name) in [
            (b"Title".as_slice(), "title"),
            (b"Author".as_slice(), "author"),
            (b"Subject".as_slice(), "subject"),
            (b"Keywords".as_slice(), "keywords"),
            (b"Creator".as_slice(), "creator"),
            (b"Producer".as_slice(), "producer"),
            (b"CreationDate".as_slice(), "creation_date"),
            (b"ModDate".as_slice(), "modification_date"),
        ] {
            if let Ok(value) = info_dict.get(key)
                && let Some(text) = extract_pdf_string(value)
            {
                metadata.insert(display_name.to_string(), json!(text));
            }
        }
    }

    // Add page count
    let page_count = doc.get_pages().len();
    metadata.insert("page_count".to_string(), json!(page_count));

    if metadata.is_empty() {
        Ok(json!({}))
    } else {
        Ok(serde_json::Value::Object(metadata))
    }
}

/// Extract string value from PDF object
fn extract_pdf_string(obj: &lopdf::Object) -> Option<String> {
    match obj {
        lopdf::Object::String(bytes, _) => {
            // Try UTF-8 first
            if let Ok(s) = std::str::from_utf8(bytes) {
                return Some(s.to_string());
            }
            // Try UTF-16BE (common in PDFs)
            if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
                let utf16: Vec<u16> = bytes[2..]
                    .chunks_exact(2)
                    .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
                    .collect();
                return String::from_utf16(&utf16).ok();
            }
            // Fall back to lossy conversion
            Some(String::from_utf8_lossy(bytes).to_string())
        }
        lopdf::Object::Name(name) => Some(String::from_utf8_lossy(name).to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text_from_pdf_invalid() {
        let result = extract_text_from_pdf(b"not a pdf");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_with_metadata_invalid() {
        let options = ExtractionOptions::default();
        let result = extract_with_metadata(b"not a pdf", &options);
        assert!(result.is_err());
    }
}
