//! Legacy Microsoft Excel (.xls) text extraction.
//!
//! Legacy .xls files use the OLE/CFB (Compound File Binary) format with BIFF
//! (Binary Interchange File Format) records.
//!
//! Note: This is a basic implementation that handles simple spreadsheets.
//! Complex formatting, formulas, and embedded objects may not be fully supported.

use anyhow::{Result, anyhow};
use cfb::CompoundFile;
use serde_json::{Value, json};
use std::io::{Cursor, Read};

use crate::extract::config::ExtractionOptions;

/// Result of legacy .xls extraction
#[derive(Debug)]
pub struct XlsExtractionResult {
    /// Extracted plain text content
    pub text: String,
    /// Document metadata
    pub metadata: Option<Value>,
}

/// BIFF record types for text extraction
const BIFF_LABEL: u16 = 0x0204; // Cell label (BIFF2-4)
const BIFF_LABEL_SST: u16 = 0x00FD; // String from SST (BIFF8)
const BIFF_SST: u16 = 0x00FC; // Shared String Table (BIFF8)
const BIFF_STRING: u16 = 0x0207; // String record
const BIFF_RSTRING: u16 = 0x00D6; // Rich string

/// Extract text with metadata from legacy .xls
pub fn extract_with_metadata(
    bytes: &[u8],
    options: &ExtractionOptions,
) -> Result<XlsExtractionResult> {
    let cursor = Cursor::new(bytes);
    let mut cfb = CompoundFile::open(cursor).map_err(|e| {
        anyhow!(
            "Failed to open CFB file: {}. File may not be a valid .xls format.",
            e
        )
    })?;

    let mut warnings = Vec::new();

    // Extract text from Workbook stream
    let text = match extract_workbook_text(&mut cfb) {
        Ok(text) => text,
        Err(e) => {
            warnings.push(format!("Primary extraction failed: {}", e));
            // Try fallback extraction
            extract_text_fallback(&mut cfb).unwrap_or_default()
        }
    };

    // Extract metadata if requested
    let metadata = if options.include_metadata {
        Some(extract_xls_metadata(&mut cfb, &mut warnings))
    } else {
        None
    };

    Ok(XlsExtractionResult { text, metadata })
}

/// Extract text from the Workbook stream
fn extract_workbook_text(cfb: &mut CompoundFile<Cursor<&[u8]>>) -> Result<String> {
    // Try different stream names used by different Excel versions
    let mut workbook_stream = cfb
        .open_stream("/Workbook")
        .or_else(|_| cfb.open_stream("Workbook"))
        .or_else(|_| cfb.open_stream("/Book"))
        .or_else(|_| cfb.open_stream("Book"))
        .map_err(|e| anyhow!("Workbook stream not found: {}", e))?;

    let mut data = Vec::new();
    workbook_stream
        .read_to_end(&mut data)
        .map_err(|e| anyhow!("Failed to read Workbook stream: {}", e))?;

    // Parse BIFF records and extract text
    extract_text_from_biff(&data)
}

/// Extract text from BIFF records
fn extract_text_from_biff(data: &[u8]) -> Result<String> {
    let mut texts = Vec::new();
    let mut shared_strings: Vec<String> = Vec::new();
    let mut offset = 0;

    // First pass: collect shared strings (SST)
    while offset + 4 <= data.len() {
        let record_type = u16::from_le_bytes([data[offset], data[offset + 1]]);
        let record_len = u16::from_le_bytes([data[offset + 2], data[offset + 3]]) as usize;

        let content_start = offset + 4;
        let content_end = content_start + record_len;

        if content_end > data.len() {
            break;
        }

        if record_type == BIFF_SST && record_len >= 8 {
            // Parse Shared String Table
            let record_data = &data[content_start..content_end];
            if let Ok(strings) = parse_sst(record_data) {
                shared_strings = strings;
            }
        }

        offset = content_end;
    }

    // Second pass: extract text from records
    offset = 0;
    while offset + 4 <= data.len() {
        let record_type = u16::from_le_bytes([data[offset], data[offset + 1]]);
        let record_len = u16::from_le_bytes([data[offset + 2], data[offset + 3]]) as usize;

        let content_start = offset + 4;
        let content_end = content_start + record_len;

        if content_end > data.len() {
            break;
        }

        let record_data = &data[content_start..content_end];

        match record_type {
            BIFF_LABEL if record_len >= 8 => {
                // LABEL record: row(2) + col(2) + xf(2) + string
                if let Ok(text) = parse_biff_string(&record_data[6..])
                    && !text.is_empty()
                {
                    texts.push(text);
                }
            }
            BIFF_LABEL_SST if record_len >= 10 => {
                // LABELSST record: row(2) + col(2) + xf(2) + sst_index(4)
                let sst_idx = u32::from_le_bytes([
                    record_data[6],
                    record_data[7],
                    record_data[8],
                    record_data[9],
                ]) as usize;
                if sst_idx < shared_strings.len() {
                    let text = &shared_strings[sst_idx];
                    if !text.is_empty() {
                        texts.push(text.clone());
                    }
                }
            }
            BIFF_STRING if record_len >= 3 => {
                if let Ok(text) = parse_biff_string(record_data)
                    && !text.is_empty()
                {
                    texts.push(text);
                }
            }
            BIFF_RSTRING if record_len >= 8 => {
                // Rich string record
                if let Ok(text) = parse_biff_string(&record_data[6..])
                    && !text.is_empty()
                {
                    texts.push(text);
                }
            }
            _ => {}
        }

        offset = content_end;
    }

    // If we didn't find structured text, try brute force
    if texts.is_empty() {
        return Ok(extract_ascii_strings(data));
    }

    Ok(texts.join("\t"))
}

/// Parse a BIFF string (can be either byte string or Unicode)
fn parse_biff_string(data: &[u8]) -> Result<String> {
    if data.is_empty() {
        return Ok(String::new());
    }

    // Check for BIFF8 Unicode string format
    if data.len() >= 3 {
        let char_count = u16::from_le_bytes([data[0], data[1]]) as usize;
        let flags = data[2];
        let is_unicode = (flags & 0x01) != 0;

        let string_start = 3;
        if is_unicode {
            let byte_len = char_count * 2;
            if string_start + byte_len <= data.len() {
                return decode_utf16le(&data[string_start..string_start + byte_len]);
            }
        } else if string_start + char_count <= data.len() {
            let text: String = data[string_start..string_start + char_count]
                .iter()
                .filter(|&&b| b >= 0x20 || b == 0x0A || b == 0x0D || b == 0x09)
                .map(|&b| b as char)
                .collect();
            return Ok(text);
        }
    }

    // Fall back to simple ASCII extraction
    let text: String = data
        .iter()
        .filter(|&&b| (0x20..0x7F).contains(&b))
        .map(|&b| b as char)
        .collect();
    Ok(text)
}

/// Parse Shared String Table (SST)
fn parse_sst(data: &[u8]) -> Result<Vec<String>> {
    if data.len() < 8 {
        return Err(anyhow!("SST too short"));
    }

    // Total strings (4 bytes) + unique strings (4 bytes)
    let unique_count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let mut strings = Vec::with_capacity(unique_count.min(10000));
    let mut offset = 8;

    for _ in 0..unique_count {
        if offset + 3 > data.len() {
            break;
        }

        let char_count = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
        let flags = data[offset + 2];
        let is_unicode = (flags & 0x01) != 0;
        let has_rich = (flags & 0x08) != 0;
        let has_asian = (flags & 0x04) != 0;

        offset += 3;

        // Skip rich text formatting run count
        if has_rich && offset + 2 <= data.len() {
            offset += 2;
        }

        // Skip Asian phonetic settings size
        if has_asian && offset + 4 <= data.len() {
            offset += 4;
        }

        let byte_len = if is_unicode {
            char_count * 2
        } else {
            char_count
        };

        if offset + byte_len > data.len() {
            break;
        }

        let text = if is_unicode {
            decode_utf16le(&data[offset..offset + byte_len]).unwrap_or_default()
        } else {
            data[offset..offset + byte_len]
                .iter()
                .filter(|&&b| b >= 0x20 || b == 0x0A || b == 0x0D || b == 0x09)
                .map(|&b| b as char)
                .collect()
        };

        strings.push(text);
        offset += byte_len;
    }

    Ok(strings)
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

/// Fallback text extraction
fn extract_text_fallback(cfb: &mut CompoundFile<Cursor<&[u8]>>) -> Result<String> {
    let mut all_text = String::new();

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
            if current_string.len() >= 3 && current_string.chars().any(|c| c.is_alphabetic()) {
                strings.push(current_string.clone());
            }
            current_string.clear();
        }
    }

    if current_string.len() >= 3 && current_string.chars().any(|c| c.is_alphabetic()) {
        strings.push(current_string);
    }

    strings.join(" ")
}

/// Extract metadata from document
fn extract_xls_metadata(
    cfb: &mut CompoundFile<Cursor<&[u8]>>,
    warnings: &mut Vec<String>,
) -> Value {
    let mut metadata = serde_json::Map::new();

    if let Ok(summary) = extract_summary_info(cfb) {
        for (key, value) in summary {
            metadata.insert(key, value);
        }
    } else {
        warnings.push("Could not extract summary information".to_string());
    }

    if let Ok(doc_summary) = extract_doc_summary_info(cfb) {
        for (key, value) in doc_summary {
            metadata.insert(key, value);
        }
    }

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

    for ch in text.chars() {
        match ch {
            '\n' | '\t' => {
                if !last_was_space {
                    result.push(' ');
                    last_was_space = true;
                }
            }
            ' ' => {
                if !last_was_space {
                    result.push(' ');
                    last_was_space = true;
                }
            }
            c if c.is_control() => {}
            c => {
                result.push(c);
                last_was_space = false;
            }
        }
    }

    result.trim().to_string()
}

/// Check if bytes represent a legacy .xls file (OLE/CFB format)
pub fn is_legacy_xls(bytes: &[u8]) -> bool {
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
    fn test_is_legacy_xls() {
        let valid_header = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0x00, 0x00];
        assert!(is_legacy_xls(&valid_header));

        let invalid_header = [0x50, 0x4B, 0x03, 0x04]; // ZIP header (xlsx)
        assert!(!is_legacy_xls(&invalid_header));

        let too_short = [0xD0, 0xCF];
        assert!(!is_legacy_xls(&too_short));
    }

    #[test]
    fn test_clean_text() {
        let text = "Hello   World\n\n\n\nTest";
        let result = clean_text(text);
        assert_eq!(result, "Hello World Test");
    }

    #[test]
    fn test_decode_utf16le() {
        let data = [0x48, 0x00, 0x65, 0x00, 0x6C, 0x00, 0x6C, 0x00, 0x6F, 0x00];
        let result = decode_utf16le(&data).unwrap();
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_invalid_xls_file() {
        let invalid_data = b"This is not a valid xls file";
        let result = extract_with_metadata(invalid_data, &ExtractionOptions::default());
        assert!(result.is_err());
    }
}
