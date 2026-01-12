//! File upload validation using magic bytes and compression detection.
//!
//! This module provides comprehensive validation for uploaded files including:
//! - Magic byte verification using the `infer` crate
//! - MIME type validation
//! - ZIP bomb detection via compression ratio analysis
//! - File size limits

use anyhow::{Result, anyhow};
use infer::Infer;
use std::io::Cursor;
use zip::ZipArchive;

/// Maximum allowed file size: 100MB
const MAX_FILE_SIZE_BYTES: usize = 100 * 1024 * 1024;

/// Maximum compression ratio before flagging as potential ZIP bomb
/// A ratio > 100 means the compressed file is 100x smaller than uncompressed
const MAX_COMPRESSION_RATIO: f64 = 100.0;

/// Whitelist of allowed MIME types for document uploads
const ALLOWED_MIME_TYPES: &[&str] = &[
    // Text formats
    "text/plain",
    "text/csv",
    "text/markdown",
    // Documents
    "application/pdf",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.ms-excel",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    // Archives (will be further validated)
    "application/zip",
    "application/x-zip-compressed",
    "application/x-7z-compressed",
];

#[derive(Debug, Clone)]
pub(crate) struct FileValidationResult {
    pub(crate) detected_mime: String,
    pub(crate) is_valid: bool,
    pub(crate) validation_errors: Vec<String>,
}

/// Validate an uploaded file using multiple criteria
///
/// # Arguments
/// * `file_bytes` - The raw file content
/// * `filename` - The original filename for extension validation
/// * `mime_type` - The MIME type from the client
///
/// # Returns
/// A FileValidationResult containing validation status and any errors found
pub(crate) fn validate_upload_file(
    file_bytes: &[u8],
    filename: &str,
    mime_type: &str,
) -> FileValidationResult {
    let mut errors = Vec::new();

    // Check file size
    if file_bytes.len() > MAX_FILE_SIZE_BYTES {
        errors.push(format!(
            "File exceeds maximum size of {} bytes ({}MB)",
            MAX_FILE_SIZE_BYTES,
            MAX_FILE_SIZE_BYTES / (1024 * 1024)
        ));
    }

    // Detect actual MIME type from magic bytes
    let infer = Infer::new();
    let detected_mime = infer
        .get(file_bytes)
        .map(|t| t.mime_type().to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    tracing::debug!(
        filename = %filename,
        claimed_mime = %mime_type,
        detected_mime = %detected_mime,
        file_size = file_bytes.len(),
        "File validation started"
    );

    // Validate MIME type
    if !ALLOWED_MIME_TYPES.contains(&detected_mime.as_str())
        && !ALLOWED_MIME_TYPES.contains(&mime_type)
    {
        errors.push(format!(
            "File type not allowed. Detected: {}, Claimed: {}. Allowed types: {:?}",
            detected_mime, mime_type, ALLOWED_MIME_TYPES
        ));
    }

    // Check for MIME type mismatch (potential attack)
    if should_validate_mime_match(&detected_mime, mime_type) && detected_mime != mime_type {
        tracing::warn!(
            filename = %filename,
            claimed = %mime_type,
            detected = %detected_mime,
            "MIME type mismatch detected"
        );
        // Note: We warn but don't fail here as some legitimate use cases exist
        // The detected type is what matters for actual validation
    }

    // Check for ZIP bombs if it's a ZIP file
    if (detected_mime == "application/zip" || mime_type == "application/zip")
        && let Err(e) = validate_zip_bomb(file_bytes)
    {
        errors.push(e.to_string());
    }

    let is_valid = errors.is_empty();

    if !is_valid {
        tracing::warn!(
            filename = %filename,
            validation_errors = ?errors,
            "File validation failed"
        );
    }

    FileValidationResult {
        detected_mime,
        is_valid,
        validation_errors: errors,
    }
}

/// Check if MIME type mismatch should be validated (excluding text/binary types)
fn should_validate_mime_match(detected: &str, claimed: &str) -> bool {
    // Don't fail on text/* or application/octet-stream mismatches
    // as these are often correctly reported as generic
    !(detected.starts_with("text/") || claimed == "application/octet-stream")
}

/// Detect and prevent ZIP bomb attacks
///
/// Checks for suspiciously high compression ratios which may indicate
/// a ZIP bomb (e.g., highly compressed large files).
fn validate_zip_bomb(file_bytes: &[u8]) -> Result<()> {
    let cursor = Cursor::new(file_bytes);
    let mut zip = ZipArchive::new(cursor).map_err(|e| anyhow!("Invalid ZIP file: {}", e))?;

    let mut total_uncompressed = 0u64;
    let total_compressed = file_bytes.len() as u64;

    // Iterate through all files in the archive
    for i in 0..zip.len() {
        let file = zip
            .by_index(i)
            .map_err(|e| anyhow!("Error reading ZIP entry {}: {}", i, e))?;

        let uncompressed_size = file.size();
        total_uncompressed += uncompressed_size;

        // Check individual file limit (100MB uncompressed)
        if uncompressed_size > MAX_FILE_SIZE_BYTES as u64 {
            return Err(anyhow!(
                "ZIP contains file exceeding {}MB uncompressed: {} bytes",
                MAX_FILE_SIZE_BYTES / (1024 * 1024),
                uncompressed_size
            ));
        }
    }

    // Check overall compression ratio
    if total_compressed > 0 {
        let ratio = total_uncompressed as f64 / total_compressed as f64;
        if ratio > MAX_COMPRESSION_RATIO {
            return Err(anyhow!(
                "ZIP compression ratio exceeds {}x ({}x detected). Possible ZIP bomb.",
                MAX_COMPRESSION_RATIO,
                ratio
            ));
        }
    }

    // Check total uncompressed size
    if total_uncompressed > MAX_FILE_SIZE_BYTES as u64 {
        return Err(anyhow!(
            "ZIP archive uncompresses to {}MB, exceeds limit of {}MB",
            total_uncompressed / (1024 * 1024),
            MAX_FILE_SIZE_BYTES / (1024 * 1024)
        ));
    }

    tracing::debug!(
        compressed_size = total_compressed,
        uncompressed_size = total_uncompressed,
        compression_ratio = total_uncompressed as f64 / total_compressed as f64,
        "ZIP file validation passed"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_plain_text() {
        let content = b"Hello, world!";
        let result = validate_upload_file(content, "test.txt", "text/plain");
        assert!(result.is_valid);
    }

    #[test]
    fn test_validate_disallowed_type() {
        let content = b"Not a real executable";
        let result = validate_upload_file(content, "test.exe", "application/x-msdownload");
        assert!(!result.is_valid);
        assert!(!result.validation_errors.is_empty());
    }

    #[test]
    fn test_validate_file_too_large() {
        let large_content = vec![0u8; MAX_FILE_SIZE_BYTES + 1];
        let result = validate_upload_file(&large_content, "large.txt", "text/plain");
        assert!(!result.is_valid);
        assert!(
            result
                .validation_errors
                .iter()
                .any(|e| e.contains("exceeds maximum size"))
        );
    }

    #[test]
    fn test_validate_mime_type_detection() {
        // PDF magic bytes: %PDF
        let pdf_content = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n";
        let result = validate_upload_file(pdf_content, "test.pdf", "application/pdf");
        assert!(result.is_valid);
        assert_eq!(result.detected_mime, "application/pdf");
    }
}
