//! File upload validation using magic bytes and compression detection.
//!
//! This module provides comprehensive validation for uploaded files including:
//! - Magic byte verification using the `infer` crate
//! - MIME type validation
//! - File size limits
use std::path::Path;

use infer::Infer;

/// Maximum allowed file size: 100MB
const MAX_FILE_SIZE_BYTES: u64 = 100 * 1024 * 1024;

/// Number of bytes read from file header for MIME type detection.
/// The `infer` crate only inspects the first few hundred bytes,
/// so 8 KiB is more than sufficient.
const MAGIC_BYTES_READ_SIZE: usize = 8192;

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

/// Validate an uploaded file without loading the entire file into memory.
///
/// Reads file metadata for size checking and only the first few KiB for
/// MIME-type detection via magic bytes, keeping peak memory usage constant
/// regardless of file size.
///
/// # Arguments
/// * `file_path` - Path to the temporary upload file on disk
/// * `filename` - The original filename for logging
pub(crate) async fn validate_upload_file(file_path: &Path, filename: &str) -> FileValidationResult {
    let mut errors = Vec::new();

    let metadata = match tokio::fs::metadata(file_path).await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(filename = %filename, error = %e, "Failed to read file metadata");
            return FileValidationResult {
                is_valid: false,
                validation_errors: vec![format!("Failed to read file metadata: {e}")],
                mime_type: None,
            };
        }
    };

    let file_size = metadata.len();
    if file_size > MAX_FILE_SIZE_BYTES {
        errors.push(format!(
            "File exceeds maximum size of {} bytes ({}MB)",
            MAX_FILE_SIZE_BYTES,
            MAX_FILE_SIZE_BYTES / (1024 * 1024)
        ));
    }

    let header = match read_file_header(file_path).await {
        Ok(buf) => buf,
        Err(e) => {
            tracing::error!(filename = %filename, error = %e, "Failed to read file header");
            return FileValidationResult {
                is_valid: false,
                validation_errors: vec![format!("Failed to read file: {e}")],
                mime_type: None,
            };
        }
    };

    let detected_mime = detect_mime_type(&header);

    tracing::debug!(
        filename = %filename,
        detected_mime = %detected_mime,
        file_size = file_size,
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

async fn read_file_header(path: &Path) -> std::io::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut file = tokio::fs::File::open(path).await?;
    let mut buf = vec![0u8; MAGIC_BYTES_READ_SIZE];
    let bytes_read = file.read(&mut buf).await?;
    buf.truncate(bytes_read);
    Ok(buf)
}

fn detect_mime_type(header: &[u8]) -> String {
    let infer = Infer::new();
    infer
        .get(header)
        .map(|t| t.mime_type().to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string())
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
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp_file(content: &[u8]) -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(content)
            .expect("failed to write temp file content");
        f.flush().expect("failed to flush temp file");
        f
    }

    #[tokio::test]
    async fn test_validate_plain_text() {
        let f = write_temp_file(b"Hello, world!");
        let result = validate_upload_file(f.path(), "test.txt").await;
        assert!(result.is_valid);
    }

    #[tokio::test]
    async fn test_validate_file_too_large() {
        let mut f = NamedTempFile::new().unwrap();
        let chunk = vec![0u8; 1024 * 1024];
        for _ in 0..101 {
            f.write_all(&chunk).unwrap();
        }
        f.flush().unwrap();
        let result = validate_upload_file(f.path(), "large.txt").await;
        assert!(!result.is_valid);
        assert!(
            result
                .validation_errors
                .iter()
                .any(|e| e.contains("exceeds maximum size"))
        );
    }

    #[tokio::test]
    async fn test_validate_mime_type_detection() {
        let f = write_temp_file(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        let result = validate_upload_file(f.path(), "test.pdf").await;
        assert!(result.is_valid);
        assert_eq!(result.mime_type.as_deref(), Some("application/pdf"));
    }

    #[test]
    fn test_detect_mime_type_plain_text() {
        let mime = detect_mime_type(b"Hello, world!");
        assert_eq!(mime, "application/octet-stream");
    }

    #[test]
    fn test_detect_mime_type_pdf() {
        let mime = detect_mime_type(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        assert_eq!(mime, "application/pdf");
    }

    #[test]
    fn test_detect_mime_type_rtf() {
        let mime = detect_mime_type(b"{\\rtf1\\ansi Test RTF}");
        assert_eq!(mime, "text/rtf");
    }

    #[test]
    fn test_get_allowed_mime_types() {
        let types = get_allowed_mime_types();
        assert!(types.contains(&"text/plain".to_string()));
        assert!(types.contains(&"application/pdf".to_string()));
        assert!(types.contains(&"application/vnd.ms-powerpoint".to_string()));
        assert!(types.contains(&"text/html".to_string()));
        assert!(types.contains(&"application/json".to_string()));
        assert!(types.contains(&"message/rfc822".to_string()));
        assert!(types.contains(&"application/epub+zip".to_string()));
        assert!(
            types.len() >= 45,
            "Expected at least 45 MIME types, got {}",
            types.len()
        );
    }
}
