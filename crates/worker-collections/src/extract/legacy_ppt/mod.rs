//! Legacy Microsoft PowerPoint (.ppt) text extraction.
//!
//! Legacy .ppt files use the OLE/CFB (Compound File Binary) format.
//! This module extracts text from the PowerPoint Document stream.
//!
//! Note: This is a basic implementation that handles simple presentations.
//! Complex formatting, animations, and embedded objects may not be fully supported.

use anyhow::{Result, anyhow};
use cfb::CompoundFile;
use serde_json::{Value, json};
use std::io::{Cursor, Read};

use crate::extract::config::ExtractionOptions;

/// Result of legacy .ppt extraction
#[derive(Debug)]
pub struct PptExtractionResult {
    /// Extracted plain text content
    pub text: String,
    /// Document metadata
    pub metadata: Option<Value>,
}

/// PowerPoint record types for text extraction
const TEXT_CHARS_ATOM: u16 = 0x0FA0; // Unicode text
const TEXT_BYTES_ATOM: u16 = 0x0FA8; // ANSI text
const CSTRING: u16 = 0x0FBA; // C-style string (used in some contexts)

/// Extract text with metadata from legacy .ppt
pub fn extract_with_metadata(
    bytes: &[u8],
    options: &ExtractionOptions,
) -> Result<PptExtractionResult> {
    let cursor = Cursor::new(bytes);
    let mut cfb = CompoundFile::open(cursor).map_err(|e| {
        anyhow!(
            "Failed to open CFB file: {}. File may not be a valid .ppt format.",
            e
        )
    })?;

    let mut warnings = Vec::new();

    // Extract text from PowerPoint Document stream
    let text = match extract_powerpoint_text(&mut cfb) {
        Ok(text) => text,
        Err(e) => {
            warnings.push(format!("Primary extraction failed: {}", e));
            // Try fallback extraction
            extract_text_fallback(&mut cfb).unwrap_or_default()
        }
    };

    // Extract metadata if requested
    let metadata = if options.include_metadata {
        Some(extract_ppt_metadata(&mut cfb, &mut warnings))
    } else {
        None
    };

    Ok(PptExtractionResult { text, metadata })
}

/// Extract text from the PowerPoint Document stream
fn extract_powerpoint_text(cfb: &mut CompoundFile<Cursor<&[u8]>>) -> Result<String> {
    // Try to read the PowerPoint Document stream
    let mut ppt_stream = cfb
        .open_stream("/PowerPoint Document")
        .or_else(|_| cfb.open_stream("PowerPoint Document"))
        .map_err(|e| anyhow!("PowerPoint Document stream not found: {}", e))?;

    let mut data = Vec::new();
    ppt_stream
        .read_to_end(&mut data)
        .map_err(|e| anyhow!("Failed to read PowerPoint Document stream: {}", e))?;

    // Parse records and extract text
    extract_text_from_records(&data)
}

/// Extract text from PowerPoint binary records
fn extract_text_from_records(data: &[u8]) -> Result<String> {
    let mut texts = Vec::new();
    let mut offset = 0;

    while offset + 8 <= data.len() {
        // Read record header (8 bytes)
        // Bytes 0-1: recVer (4 bits) + recInstance (12 bits)
        // Bytes 2-3: recType
        // Bytes 4-7: recLen
        let rec_type = u16::from_le_bytes([data[offset + 2], data[offset + 3]]);
        let rec_len = u32::from_le_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize;

        let content_start = offset + 8;
        let content_end = content_start + rec_len;

        if content_end > data.len() {
            break;
        }

        match rec_type {
            TEXT_CHARS_ATOM => {
                // Unicode (UTF-16LE) text
                if rec_len >= 2 {
                    let text_data = &data[content_start..content_end];
                    if let Ok(text) = decode_utf16le(text_data) {
                        let cleaned = clean_text(&text);
                        if !cleaned.is_empty() {
                            texts.push(cleaned);
                        }
                    }
                }
            }
            TEXT_BYTES_ATOM => {
                // ANSI text (Windows-1252 / Latin-1)
                if rec_len >= 1 {
                    let text_data = &data[content_start..content_end];
                    let text: String = text_data
                        .iter()
                        .filter(|&&b| b >= 0x20 || b == 0x0A || b == 0x0D || b == 0x09)
                        .map(|&b| b as char)
                        .collect();
                    let cleaned = clean_text(&text);
                    if !cleaned.is_empty() {
                        texts.push(cleaned);
                    }
                }
            }
            CSTRING => {
                // C-style null-terminated string
                if rec_len >= 1 {
                    let text_data = &data[content_start..content_end];
                    let text: String = text_data
                        .iter()
                        .take_while(|&&b| b != 0)
                        .filter(|&&b| b >= 0x20 || b == 0x0A || b == 0x0D || b == 0x09)
                        .map(|&b| b as char)
                        .collect();
                    let cleaned = clean_text(&text);
                    if !cleaned.is_empty() {
                        texts.push(cleaned);
                    }
                }
            }
            _ => {
                // Skip other record types, but check if it's a container
                // Containers have recVer = 0xF
                let rec_ver = data[offset] & 0x0F;
                if rec_ver == 0x0F && rec_len > 0 {
                    // This is a container record, parse its contents recursively
                    if let Ok(nested_text) =
                        extract_text_from_records(&data[content_start..content_end])
                        && !nested_text.is_empty()
                    {
                        texts.push(nested_text);
                    }
                    offset = content_end;
                    continue;
                }
            }
        }

        offset = content_end;
    }

    Ok(texts.join("\n"))
}

/// Decode UTF-16LE bytes to a String
fn decode_utf16le(data: &[u8]) -> Result<String> {
    if !data.len().is_multiple_of(2) {
        return Err(anyhow!("Invalid UTF-16LE data length"));
    }

    let u16_vec: Vec<u16> = data
        .chunks(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    String::from_utf16(&u16_vec).map_err(|e| anyhow!("UTF-16 decode error: {}", e))
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
                // Try to extract text from records first
                if let Ok(text) = extract_text_from_records(&data)
                    && !text.is_empty()
                {
                    all_text.push_str(&text);
                    all_text.push('\n');
                    continue;
                }
                // Fall back to ASCII string extraction
                let text = extract_ascii_strings(&data);
                if !text.is_empty() {
                    all_text.push_str(&text);
                    all_text.push('\n');
                }
            }
        }
    }

    Ok(clean_text(&all_text))
}

/// Extract ASCII strings from binary data
fn extract_ascii_strings(data: &[u8]) -> String {
    let mut strings = Vec::new();
    let mut current_string = String::new();

    for &byte in data {
        if (0x20..0x7F).contains(&byte) {
            current_string.push(byte as char);
        } else if !current_string.is_empty() {
            // Only keep strings that look like words (at least 4 chars with letters)
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
fn extract_ppt_metadata(
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

    // Parse OLE property set format (simplified)
    let mut properties = Vec::new();
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
fn clean_text(text: &str) -> String {
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

/// Check if bytes represent a legacy .ppt file (OLE/CFB format)
pub fn is_legacy_ppt(bytes: &[u8]) -> bool {
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
    fn test_is_legacy_ppt() {
        let valid_header = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0x00, 0x00];
        assert!(is_legacy_ppt(&valid_header));

        let invalid_header = [0x50, 0x4B, 0x03, 0x04]; // ZIP header (pptx)
        assert!(!is_legacy_ppt(&invalid_header));

        let too_short = [0xD0, 0xCF];
        assert!(!is_legacy_ppt(&too_short));
    }

    #[test]
    fn test_clean_text() {
        let text = "Hello   World\n\n\n\nTest";
        let result = clean_text(text);
        assert_eq!(result, "Hello World\n\nTest");
    }

    #[test]
    fn test_decode_utf16le() {
        let data = [0x48, 0x00, 0x65, 0x00, 0x6C, 0x00, 0x6C, 0x00, 0x6F, 0x00]; // "Hello"
        let result = decode_utf16le(&data).unwrap();
        assert_eq!(result, "Hello");
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
    fn test_invalid_ppt_file() {
        let invalid_data = b"This is not a valid ppt file";
        let result = extract_with_metadata(invalid_data, &ExtractionOptions::default());
        assert!(result.is_err());
    }
}
