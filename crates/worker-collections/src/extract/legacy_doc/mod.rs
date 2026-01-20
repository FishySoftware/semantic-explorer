//! Legacy Microsoft Word (.doc) text extraction.
//!
//! Legacy .doc files use the OLE/CFB (Compound File Binary) format.
//! This module extracts text from the WordDocument stream.
//!
//! Note: This is a basic implementation that handles simple documents.
//! Complex formatting, tables, and embedded objects may not be fully supported.

use anyhow::{Result, anyhow};
use cfb::CompoundFile;
use serde_json::{Value, json};
use std::io::{Cursor, Read};

use crate::extract::config::ExtractionOptions;

/// Result of legacy .doc extraction
#[derive(Debug)]
pub struct DocExtractionResult {
    /// Extracted plain text content
    pub text: String,
    /// Document metadata
    pub metadata: Option<Value>,
}

/// Extract text with metadata from legacy .doc
pub fn extract_with_metadata(
    bytes: &[u8],
    options: &ExtractionOptions,
) -> Result<DocExtractionResult> {
    let cursor = Cursor::new(bytes);
    let mut cfb = CompoundFile::open(cursor).map_err(|e| {
        anyhow!(
            "Failed to open CFB file: {}. File may not be a valid .doc format.",
            e
        )
    })?;

    let mut warnings = Vec::new();

    // Extract text from WordDocument stream
    let text = match extract_word_document_text(&mut cfb) {
        Ok(text) => text,
        Err(e) => {
            warnings.push(format!("Primary extraction failed: {}", e));
            // Try fallback extraction
            extract_text_fallback(&mut cfb).unwrap_or_default()
        }
    };

    // Extract metadata if requested
    let metadata = if options.include_metadata {
        Some(extract_doc_metadata(&mut cfb, &mut warnings))
    } else {
        None
    };

    Ok(DocExtractionResult { text, metadata })
}

/// Extract text from the WordDocument stream
fn extract_word_document_text(cfb: &mut CompoundFile<Cursor<&[u8]>>) -> Result<String> {
    // Try to read the WordDocument stream
    let mut word_doc_stream = cfb
        .open_stream("/WordDocument")
        .or_else(|_| cfb.open_stream("WordDocument"))
        .map_err(|e| anyhow!("WordDocument stream not found: {}", e))?;

    let mut word_data = Vec::new();
    word_doc_stream
        .read_to_end(&mut word_data)
        .map_err(|e| anyhow!("Failed to read WordDocument stream: {}", e))?;

    // Parse FIB (File Information Block) header
    if word_data.len() < 1472 {
        return Err(anyhow!("WordDocument stream too short for FIB header"));
    }

    // Check magic number
    let magic = u16::from_le_bytes([word_data[0], word_data[1]]);
    if magic != 0xA5EC && magic != 0xA5DC {
        return Err(anyhow!(
            "Invalid Word document magic number: 0x{:04X}",
            magic
        ));
    }

    // Try to extract text using different methods
    // Method 1: Look for text in the main document
    let text = extract_text_from_stream(&word_data);

    if text.is_empty() {
        // Method 2: Try reading from 0Table or 1Table streams
        if let Ok(table_text) = extract_from_table_stream(cfb) {
            return Ok(table_text);
        }
    }

    Ok(text)
}

/// Extract text from the word document stream data
fn extract_text_from_stream(data: &[u8]) -> String {
    let mut text = String::new();
    let mut i = 0;

    // Skip the FIB header (typically ~1472 bytes but varies)
    // Look for text after headers
    let start_offset = 1024.min(data.len());

    while i < data.len().saturating_sub(start_offset) {
        let offset = start_offset + i;
        if offset >= data.len() {
            break;
        }

        let byte = data[offset];

        // Check for ASCII printable characters
        if (0x20..0x7F).contains(&byte) {
            text.push(byte as char);
        } else if byte == 0x0D || byte == 0x0A {
            // Carriage return or line feed
            if !text.ends_with('\n') {
                text.push('\n');
            }
        } else if byte == 0x09 {
            // Tab
            text.push('\t');
        } else if byte == 0x0B {
            // Vertical tab (used as soft line break in Word)
            text.push('\n');
        } else if byte == 0x0C {
            // Form feed (page break)
            text.push_str("\n\n");
        } else if byte == 0x00 {
            // Null byte might indicate Unicode
            if i + 1 + start_offset < data.len() {
                let next_byte = data[offset + 1];
                if (0x20..0x7F).contains(&next_byte) {
                    // This might be UTF-16LE
                    text.push(next_byte as char);
                    i += 1;
                }
            }
        }

        i += 1;
    }

    clean_extracted_text(&text)
}

/// Try to extract text from table stream
fn extract_from_table_stream(cfb: &mut CompoundFile<Cursor<&[u8]>>) -> Result<String> {
    // Try 1Table first (Word 97+), then 0Table (Word 95)
    let stream = cfb
        .open_stream("/1Table")
        .or_else(|_| cfb.open_stream("/0Table"))
        .or_else(|_| cfb.open_stream("1Table"))
        .or_else(|_| cfb.open_stream("0Table"))
        .map_err(|e| anyhow!("Table stream not found: {}", e))?;

    let mut data = Vec::new();
    std::io::BufReader::new(stream)
        .read_to_end(&mut data)
        .map_err(|e| anyhow!("Failed to read table stream: {}", e))?;

    let text = extract_text_from_stream(&data);
    Ok(text)
}

/// Fallback text extraction using brute force
fn extract_text_fallback(cfb: &mut CompoundFile<Cursor<&[u8]>>) -> Result<String> {
    let mut all_text = String::new();

    // Try to read all streams and extract any text
    let entries: Vec<_> = cfb.walk().map(|e| e.path().to_path_buf()).collect();

    for path in entries {
        if let Ok(mut stream) = cfb.open_stream(&path) {
            let mut data = Vec::new();
            if stream.read_to_end(&mut data).is_ok() {
                let text = extract_ascii_strings(&data);
                if !text.is_empty() {
                    all_text.push_str(&text);
                    all_text.push('\n');
                }
            }
        }
    }

    Ok(clean_extracted_text(&all_text))
}

/// Extract ASCII strings from binary data
fn extract_ascii_strings(data: &[u8]) -> String {
    let mut strings = Vec::new();
    let mut current_string = String::new();

    for &byte in data {
        if (0x20..0x7F).contains(&byte) {
            current_string.push(byte as char);
        } else if !current_string.is_empty() {
            // Only keep strings that look like words (at least 4 chars)
            if current_string.len() >= 4 && current_string.chars().any(|c| c.is_alphabetic()) {
                strings.push(current_string.clone());
            }
            current_string.clear();
        }
    }

    if current_string.len() >= 4 && current_string.chars().any(|c| c.is_alphabetic()) {
        strings.push(current_string);
    }

    strings.join(" ")
}

/// Extract metadata from document
fn extract_doc_metadata(
    cfb: &mut CompoundFile<Cursor<&[u8]>>,
    warnings: &mut Vec<String>,
) -> Value {
    let mut metadata = serde_json::Map::new();

    // Try to read SummaryInformation stream (OLE property set)
    if let Ok(summary) = extract_summary_info(cfb) {
        for (key, value) in summary {
            metadata.insert(key, value);
        }
    } else {
        warnings.push("Could not extract summary information".to_string());
    }

    // Try to read DocumentSummaryInformation
    if let Ok(doc_summary) = extract_doc_summary_info(cfb) {
        for (key, value) in doc_summary {
            metadata.insert(key, value);
        }
    }

    // List available streams
    let streams: Vec<String> = cfb.walk().map(|e| e.path().display().to_string()).collect();
    metadata.insert("available_streams".to_string(), json!(streams));

    json!(metadata)
}

/// Extract summary information from OLE property set
fn extract_summary_info(cfb: &mut CompoundFile<Cursor<&[u8]>>) -> Result<Vec<(String, Value)>> {
    let mut stream = cfb
        .open_stream("/\u{0005}SummaryInformation")
        .or_else(|_| cfb.open_stream("\u{0005}SummaryInformation"))
        .map_err(|e| anyhow!("SummaryInformation not found: {}", e))?;

    let mut data = Vec::new();
    stream.read_to_end(&mut data)?;

    // Parse OLE property set format
    // This is a simplified parser for common properties
    let mut properties = Vec::new();

    // The format is complex; for now, extract ASCII strings as property values
    let strings = extract_property_strings(&data);
    for (i, s) in strings.into_iter().enumerate() {
        let key = match i {
            0 => "title",
            1 => "subject",
            2 => "author",
            3 => "keywords",
            4 => "comments",
            _ => continue,
        };
        properties.push((key.to_string(), json!(s)));
    }

    Ok(properties)
}

/// Extract document summary information
fn extract_doc_summary_info(cfb: &mut CompoundFile<Cursor<&[u8]>>) -> Result<Vec<(String, Value)>> {
    let mut stream = cfb
        .open_stream("/\u{0005}DocumentSummaryInformation")
        .or_else(|_| cfb.open_stream("\u{0005}DocumentSummaryInformation"))
        .map_err(|e| anyhow!("DocumentSummaryInformation not found: {}", e))?;

    let mut data = Vec::new();
    stream.read_to_end(&mut data)?;

    let mut properties = Vec::new();
    let strings = extract_property_strings(&data);

    for (i, s) in strings.into_iter().enumerate() {
        let key = match i {
            0 => "category",
            1 => "company",
            _ => continue,
        };
        properties.push((key.to_string(), json!(s)));
    }

    Ok(properties)
}

/// Extract string properties from OLE property data
fn extract_property_strings(data: &[u8]) -> Vec<String> {
    let mut strings = Vec::new();
    let mut i = 0;

    while i < data.len() {
        // Look for null-terminated strings
        if data[i] >= 0x20 && data[i] < 0x7F {
            let mut s = String::new();
            while i < data.len() && data[i] >= 0x20 && data[i] < 0x7F {
                s.push(data[i] as char);
                i += 1;
            }
            if s.len() >= 2 {
                strings.push(s);
            }
        }
        i += 1;
    }

    strings
}

/// Clean up extracted text
fn clean_extracted_text(text: &str) -> String {
    let mut result = String::new();
    let mut last_was_space = false;
    let mut consecutive_newlines = 0;

    for ch in text.chars() {
        match ch {
            '\n' => {
                consecutive_newlines += 1;
                if consecutive_newlines <= 2 {
                    result.push('\n');
                }
                last_was_space = false;
            }
            ' ' | '\t' => {
                if !last_was_space {
                    result.push(' ');
                    last_was_space = true;
                }
                consecutive_newlines = 0;
            }
            c if c.is_control() => {
                // Skip other control characters
            }
            c => {
                result.push(c);
                last_was_space = false;
                consecutive_newlines = 0;
            }
        }
    }

    result.trim().to_string()
}

/// Check if bytes represent a legacy .doc file
pub fn is_legacy_doc(bytes: &[u8]) -> bool {
    // Check for OLE magic number (D0 CF 11 E0 A1 B1 1A E1)
    if bytes.len() < 8 {
        return false;
    }

    bytes[0..8] == [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_legacy_doc() {
        let valid_header = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0x00, 0x00];
        assert!(is_legacy_doc(&valid_header));

        let invalid_header = [0x50, 0x4B, 0x03, 0x04]; // ZIP header (docx)
        assert!(!is_legacy_doc(&invalid_header));

        let too_short = [0xD0, 0xCF];
        assert!(!is_legacy_doc(&too_short));
    }

    #[test]
    fn test_clean_extracted_text() {
        let text = "Hello   World\n\n\n\nTest";
        let result = clean_extracted_text(text);
        assert_eq!(result, "Hello World\n\nTest");
    }

    #[test]
    fn test_extract_ascii_strings() {
        let data = b"Hello\x00World\x00\x01\x02Test String Here";
        let result = extract_ascii_strings(data);
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
        assert!(result.contains("Test String Here"));
    }

    #[test]
    fn test_invalid_doc_file() {
        let invalid_data = b"This is not a valid doc file";
        let result = extract_with_metadata(invalid_data, &ExtractionOptions::default());
        assert!(result.is_err());
    }

    // Note: Full .doc extraction tests require actual .doc files
    // These tests verify helper functions work correctly
}
