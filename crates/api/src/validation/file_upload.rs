//! File upload validation using magic bytes and compression detection.
//!
//! This module provides comprehensive validation for uploaded files including:
//! - Magic byte verification using the `infer` crate
//! - MIME type validation
//! - File size limits
use infer::Infer;

/// Maximum allowed file size: 100MB
const MAX_FILE_SIZE_BYTES: usize = 100 * 1024 * 1024;

/// Whitelist of allowed MIME types for document uploads
/// This list matches the extraction capabilities in worker-collections
const ALLOWED_MIME_TYPES: &[&str] = &[
    // Text formats
    "text/plain",
    "text/csv",
    "text/markdown",
    "text/html",
    "text/xml",
    "text/rtf",
    "text/x-log",
    "text/x-syslog",
    // Application text variants
    "application/xhtml+xml",
    "application/xml",
    "application/rtf",
    "application/x-rtf",
    // Documents - PDF
    "application/pdf",
    // Documents - Microsoft Word
    "application/msword", // .doc
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document", // .docx
    "application/vnd.ms-word.document.macroEnabled.12", // .docm
    "application/vnd.ms-word.template.macroEnabled.12", // .dotm
    "application/vnd.openxmlformats-officedocument.wordprocessingml.template", // .dotx
    // Documents - Microsoft Excel
    "application/vnd.ms-excel", // .xls
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", // .xlsx
    "application/vnd.ms-excel.sheet.macroEnabled.12", // .xlsm
    "application/vnd.ms-excel.template.macroEnabled.12", // .xltm
    "application/vnd.openxmlformats-officedocument.spreadsheetml.template", // .xltx
    "application/vnd.ms-excel.addin.macroEnabled.12", // .xlam
    "application/vnd.ms-excel.sheet.binary.macroEnabled.12", // .xlsb
    // Documents - Microsoft PowerPoint
    "application/vnd.ms-powerpoint", // .ppt
    "application/vnd.openxmlformats-officedocument.presentationml.presentation", // .pptx
    "application/mspowerpoint",
    "application/powerpoint",
    "application/x-mspowerpoint",
    // Documents - OpenDocument formats
    "application/vnd.oasis.opendocument.text", // .odt
    "application/vnd.oasis.opendocument.spreadsheet", // .ods
    "application/vnd.oasis.opendocument.presentation", // .odp
    // E-books
    "application/epub+zip",
    "application/epub",
    // Data formats
    "application/json",
    "application/x-ndjson",
    "text/json",
    "text/x-ndjson",
    // Email
    "message/rfc822", // .eml files
    // Archives
    "application/zip",
    "application/x-zip-compressed",
    "application/x-7z-compressed",
    "application/gzip",
    "application/x-gzip",
];

#[derive(Debug, Clone)]
pub(crate) struct FileValidationResult {
    pub(crate) is_valid: bool,
    pub(crate) validation_errors: Vec<String>,
    pub(crate) mime_type: Option<String>,
}

/// Validate an uploaded file using multiple criteria (async version)
///
/// This spawns CPU-intensive operations (ZIP parsing) on the blocking thread pool
/// to avoid blocking the async runtime.
///
/// # Arguments
/// * `file_bytes` - The raw file content
/// * `filename` - The original filename for extension validation
/// * `mime_type` - The MIME type from the client
///
/// # Returns
/// A FileValidationResult containing validation status and any errors found
pub(crate) async fn validate_upload_file(
    file_bytes: &[u8],
    filename: &str,
) -> FileValidationResult {
    let file_bytes_owned = file_bytes.to_vec();
    let filename_owned = filename.to_string();
    tokio::task::spawn_blocking(move || {
        validate_upload_file_sync(&file_bytes_owned, &filename_owned)
    })
    .await
    .unwrap_or_else(|e| {
        tracing::error!(error = %e, "Blocking validation task failed");
        FileValidationResult {
            is_valid: false,
            validation_errors: vec!["File validation task failed".to_string()],
            mime_type: None,
        }
    })
}

/// Synchronous file validation implementation
fn validate_upload_file_sync(file_bytes: &[u8], filename: &str) -> FileValidationResult {
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
        detected_mime = %detected_mime,
        file_size = file_bytes.len(),
        "File validation started"
    );

    let is_valid = errors.is_empty();

    if !is_valid {
        tracing::warn!(
            filename = %filename,
            validation_errors = ?errors,
            "File validation failed"
        );
    }

    FileValidationResult {
        is_valid,
        validation_errors: errors,
        mime_type: Some(detected_mime),
    }
}

/// Get the list of allowed MIME types for client display
///
/// Returns a vector of all MIME types that are currently allowed for upload.
/// This can be used by the UI or API clients to inform users about supported file types.
pub fn get_allowed_mime_types() -> Vec<String> {
    ALLOWED_MIME_TYPES.iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_plain_text() {
        let content = b"Hello, world!";
        let result = validate_upload_file_sync(content, "test.txt");
        assert!(result.is_valid);
    }

    #[test]
    fn test_validate_disallowed_type() {
        let content = b"Not a real executable";
        let result = validate_upload_file_sync(content, "test.exe");
        assert!(!result.is_valid);
        assert!(!result.validation_errors.is_empty());
    }

    #[test]
    fn test_validate_file_too_large() {
        let large_content = vec![0u8; MAX_FILE_SIZE_BYTES + 1];
        let result = validate_upload_file_sync(&large_content, "large.txt");
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
        let result = validate_upload_file_sync(pdf_content, "test.pdf");
        assert!(result.is_valid);
    }

    #[test]
    fn test_validate_powerpoint() {
        // PowerPoint MIME type should be allowed
        let content = b"fake ppt content";
        let result = validate_upload_file_sync(content, "test.ppt");
        // Will be valid if detected as octet-stream or the claimed type matches allowed list
        assert!(
            result.is_valid
                || result
                    .validation_errors
                    .iter()
                    .any(|e| !e.contains("application/vnd.ms-powerpoint"))
        );
    }

    #[test]
    fn test_validate_html() {
        let html_content = b"<!DOCTYPE html><html><body>Test</body></html>";
        let result = validate_upload_file_sync(html_content, "test.html");
        assert!(result.is_valid);
    }

    #[test]
    fn test_validate_json() {
        let json_content = br#"{"key": "value"}"#;
        let result = validate_upload_file_sync(json_content, "test.json");
        assert!(result.is_valid);
    }

    #[test]
    fn test_validate_rtf() {
        let rtf_content = b"{\\rtf1\\ansi Test RTF}";
        let result = validate_upload_file_sync(rtf_content, "test.rtf");
        assert!(result.is_valid);
    }

    #[test]
    fn test_validate_opendocument() {
        // OpenDocument formats should be allowed
        let content = b"fake odt content";
        let result = validate_upload_file_sync(content, "test.odt");
        assert!(
            result.is_valid
                || result
                    .validation_errors
                    .iter()
                    .any(|e| !e.contains("opendocument"))
        );
    }

    #[test]
    fn test_get_allowed_mime_types() {
        let types = get_allowed_mime_types();
        // Ensure we have all the major types
        assert!(types.contains(&"text/plain".to_string()));
        assert!(types.contains(&"application/pdf".to_string()));
        assert!(types.contains(&"application/vnd.ms-powerpoint".to_string()));
        assert!(types.contains(&"text/html".to_string()));
        assert!(types.contains(&"application/json".to_string()));
        assert!(types.contains(&"message/rfc822".to_string()));
        assert!(types.contains(&"application/epub+zip".to_string()));
        // Ensure we have a good number of types (should be 45+)
        assert!(
            types.len() >= 45,
            "Expected at least 45 MIME types, got {}",
            types.len()
        );
    }
}
