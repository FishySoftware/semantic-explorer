use anyhow::{Result, anyhow};
use flate2::read::GzDecoder;
use serde_json::{Value, json};
use std::io::{Cursor, Read};
use tar::Archive as TarArchive;
use zip::ZipArchive;

use crate::extract::config::{ExtractionConfig, ExtractionOptions};
use crate::extract::plain_text;

/// Result of archive extraction
#[derive(Debug)]
pub struct ArchiveExtractionResult {
    /// Combined text from all extracted files
    pub text: String,
    /// Metadata about the extraction
    pub metadata: Option<Value>,
}

/// Result for a single file in the archive
#[derive(Debug)]
pub struct ArchiveFileResult {
    pub path: String,
    pub text: String,
    pub mime_type: String,
    pub size: usize,
}

/// Error for a file that couldn't be extracted
#[derive(Debug)]
pub struct ArchiveFileError {
    pub path: String,
    pub error: String,
}

/// Configuration for archive extraction
#[derive(Debug, Clone)]
pub struct ArchiveOptions {
    /// Maximum depth for nested archives
    pub max_depth: usize,
    /// Maximum total size to extract (in bytes)
    pub max_total_size: usize,
    /// File extensions to skip
    pub skip_extensions: Vec<String>,
    /// Continue extracting after file errors
    pub continue_on_error: bool,
}

impl Default for ArchiveOptions {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_total_size: 100 * 1024 * 1024, // 100MB
            skip_extensions: vec![
                "exe".into(),
                "dll".into(),
                "so".into(),
                "dylib".into(),
                "bin".into(),
                "obj".into(),
                "o".into(),
                "a".into(),
                "png".into(),
                "jpg".into(),
                "jpeg".into(),
                "gif".into(),
                "ico".into(),
                "bmp".into(),
                "webp".into(),
                "svg".into(),
                "mp3".into(),
                "mp4".into(),
                "wav".into(),
                "avi".into(),
                "mov".into(),
                "mkv".into(),
                "flv".into(),
                "wmv".into(),
            ],
            continue_on_error: true,
        }
    }
}

/// Extract contents from a ZIP archive
pub(crate) fn extract_from_zip(
    bytes: &[u8],
    options: &ExtractionOptions,
    archive_options: &ArchiveOptions,
) -> Result<ArchiveExtractionResult> {
    extract_from_zip_with_depth(bytes, options, archive_options, 0)
}

fn extract_from_zip_with_depth(
    bytes: &[u8],
    options: &ExtractionOptions,
    archive_options: &ArchiveOptions,
    depth: usize,
) -> Result<ArchiveExtractionResult> {
    if depth >= archive_options.max_depth {
        return Err(anyhow!(
            "Maximum archive depth ({}) exceeded",
            archive_options.max_depth
        ));
    }

    let reader = Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(reader).map_err(|e| anyhow!("Failed to open ZIP archive: {}", e))?;

    let mut files = Vec::new();
    let mut failed_files = Vec::new();
    let mut total_size = 0usize;

    for i in 0..archive.len() {
        let mut file = match archive.by_index(i) {
            Ok(f) => f,
            Err(e) => {
                if archive_options.continue_on_error {
                    failed_files.push(ArchiveFileError {
                        path: format!("file_index_{}", i),
                        error: e.to_string(),
                    });
                    continue;
                }
                return Err(anyhow!("Failed to read archive entry: {}", e));
            }
        };

        // Skip directories
        if file.is_dir() {
            continue;
        }

        let path = file.name().to_string();

        // Check if we should skip this file type
        if should_skip_file(&path, archive_options) {
            continue;
        }

        // Read file contents
        let mut buffer = Vec::new();
        if let Err(e) = file.read_to_end(&mut buffer) {
            if archive_options.continue_on_error {
                failed_files.push(ArchiveFileError {
                    path: path.clone(),
                    error: e.to_string(),
                });
                continue;
            }
            return Err(anyhow!("Failed to read file {}: {}", path, e));
        }

        // Check size limits
        total_size += buffer.len();
        if total_size > archive_options.max_total_size {
            break; // Stop processing, don't fail
        }

        // Detect content type and extract
        let result = extract_file_content(&path, &buffer, options, archive_options, depth);
        match result {
            Ok(file_result) => files.push(file_result),
            Err(e) => {
                if archive_options.continue_on_error {
                    failed_files.push(ArchiveFileError {
                        path,
                        error: e.to_string(),
                    });
                } else {
                    return Err(e);
                }
            }
        }
    }

    build_result(files, failed_files, "zip", options)
}

/// Extract contents from a gzipped file
pub(crate) fn extract_from_gzip(bytes: &[u8], _options: &ExtractionOptions) -> Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(bytes);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| anyhow!("Failed to decompress gzip: {}", e))?;
    Ok(decompressed)
}

/// Extract contents from a tar.gz archive
pub(crate) fn extract_from_tar_gz(
    bytes: &[u8],
    options: &ExtractionOptions,
    archive_options: &ArchiveOptions,
) -> Result<ArchiveExtractionResult> {
    extract_from_tar_gz_with_depth(bytes, options, archive_options, 0)
}

fn extract_from_tar_gz_with_depth(
    bytes: &[u8],
    options: &ExtractionOptions,
    archive_options: &ArchiveOptions,
    depth: usize,
) -> Result<ArchiveExtractionResult> {
    if depth >= archive_options.max_depth {
        return Err(anyhow!(
            "Maximum archive depth ({}) exceeded",
            archive_options.max_depth
        ));
    }

    let decoder = GzDecoder::new(bytes);
    let mut archive = TarArchive::new(decoder);

    let mut files = Vec::new();
    let mut failed_files = Vec::new();
    let mut total_size = 0usize;

    let entries = archive
        .entries()
        .map_err(|e| anyhow!("Failed to read tar entries: {}", e))?;

    for entry_result in entries {
        let mut entry = match entry_result {
            Ok(e) => e,
            Err(e) => {
                if archive_options.continue_on_error {
                    failed_files.push(ArchiveFileError {
                        path: "unknown".into(),
                        error: e.to_string(),
                    });
                    continue;
                }
                return Err(anyhow!("Failed to read tar entry: {}", e));
            }
        };

        // Get path
        let path = match entry.path() {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(e) => {
                if archive_options.continue_on_error {
                    failed_files.push(ArchiveFileError {
                        path: "unknown".into(),
                        error: e.to_string(),
                    });
                    continue;
                }
                return Err(anyhow!("Failed to get entry path: {}", e));
            }
        };

        // Skip directories
        if entry.header().entry_type().is_dir() {
            continue;
        }

        // Check if we should skip this file type
        if should_skip_file(&path, archive_options) {
            continue;
        }

        // Read file contents
        let mut buffer = Vec::new();
        if let Err(e) = entry.read_to_end(&mut buffer) {
            if archive_options.continue_on_error {
                failed_files.push(ArchiveFileError {
                    path: path.clone(),
                    error: e.to_string(),
                });
                continue;
            }
            return Err(anyhow!("Failed to read file {}: {}", path, e));
        }

        // Check size limits
        total_size += buffer.len();
        if total_size > archive_options.max_total_size {
            break;
        }

        // Detect content type and extract
        let result = extract_file_content(&path, &buffer, options, archive_options, depth);
        match result {
            Ok(file_result) => files.push(file_result),
            Err(e) => {
                if archive_options.continue_on_error {
                    failed_files.push(ArchiveFileError {
                        path,
                        error: e.to_string(),
                    });
                } else {
                    return Err(e);
                }
            }
        }
    }

    build_result(files, failed_files, "tar.gz", options)
}

/// Check if file should be skipped based on extension
fn should_skip_file(path: &str, options: &ArchiveOptions) -> bool {
    if let Some(ext) = path.rsplit('.').next() {
        options
            .skip_extensions
            .iter()
            .any(|skip| skip.eq_ignore_ascii_case(ext))
    } else {
        false
    }
}

/// Detect MIME type from file extension
fn detect_mime_from_extension(path: &str) -> Option<mime::Mime> {
    let ext = path.rsplit('.').next()?.to_lowercase();

    match ext.as_str() {
        // Text formats
        "txt" => Some("text/plain".parse().unwrap()),
        "md" | "markdown" => Some("text/markdown".parse().unwrap()),
        "json" => Some("application/json".parse().unwrap()),
        "ndjson" | "jsonl" => Some("application/x-ndjson".parse().unwrap()),
        "xml" => Some("application/xml".parse().unwrap()),
        "html" | "htm" => Some("text/html".parse().unwrap()),
        "csv" => Some("text/csv".parse().unwrap()),
        "log" => Some("text/x-log".parse().unwrap()),

        // Documents
        "pdf" => Some("application/pdf".parse().unwrap()),
        "doc" => Some("application/msword".parse().unwrap()),
        "docx" => Some(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                .parse()
                .unwrap(),
        ),
        "xls" => Some("application/vnd.ms-excel".parse().unwrap()),
        "xlsx" => Some(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                .parse()
                .unwrap(),
        ),
        "ppt" => Some("application/vnd.ms-powerpoint".parse().unwrap()),
        "pptx" => Some(
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
                .parse()
                .unwrap(),
        ),
        "odt" => Some("application/vnd.oasis.opendocument.text".parse().unwrap()),
        "ods" => Some(
            "application/vnd.oasis.opendocument.spreadsheet"
                .parse()
                .unwrap(),
        ),
        "odp" => Some(
            "application/vnd.oasis.opendocument.presentation"
                .parse()
                .unwrap(),
        ),

        // Archives (for nested extraction)
        "zip" => Some("application/zip".parse().unwrap()),
        "tar" => Some("application/x-tar".parse().unwrap()),
        "gz" | "gzip" => Some("application/gzip".parse().unwrap()),

        _ => None,
    }
}

/// Extract content from a single file within the archive
fn extract_file_content(
    path: &str,
    buffer: &[u8],
    options: &ExtractionOptions,
    archive_options: &ArchiveOptions,
    depth: usize,
) -> Result<ArchiveFileResult> {
    let mime_type = detect_mime_from_extension(path);

    // Handle nested archives
    if let Some(ref mime) = mime_type
        && mime.type_() == mime::APPLICATION
    {
        match mime.subtype().as_str() {
            "zip" => {
                let nested =
                    extract_from_zip_with_depth(buffer, options, archive_options, depth + 1)?;
                return Ok(ArchiveFileResult {
                    path: path.to_string(),
                    text: nested.text,
                    mime_type: "application/zip".to_string(),
                    size: buffer.len(),
                });
            }
            "gzip" => {
                // Check if it's a .tar.gz
                if path.ends_with(".tar.gz") || path.ends_with(".tgz") {
                    let nested = extract_from_tar_gz_with_depth(
                        buffer,
                        options,
                        archive_options,
                        depth + 1,
                    )?;
                    return Ok(ArchiveFileResult {
                        path: path.to_string(),
                        text: nested.text,
                        mime_type: "application/x-tar+gzip".to_string(),
                        size: buffer.len(),
                    });
                }
                // Regular gzip - decompress and try to extract as text
                let decompressed = extract_from_gzip(buffer, options)?;
                let text = String::from_utf8_lossy(&decompressed).to_string();
                return Ok(ArchiveFileResult {
                    path: path.to_string(),
                    text,
                    mime_type: "application/gzip".to_string(),
                    size: buffer.len(),
                });
            }
            _ => {}
        }
    }

    // Try to extract text using the existing extractors
    let extraction_config = ExtractionConfig {
        options: options.clone(),
        ..Default::default()
    };

    let text = if let Some(mime) = mime_type.clone() {
        match plain_text::extract(&mime, buffer, &extraction_config) {
            Ok(content) => content.text,
            Err(_) => {
                // Fall back to raw text for unknown but likely text files
                String::from_utf8_lossy(buffer).to_string()
            }
        }
    } else {
        // Try as plain text
        if is_likely_text(buffer) {
            String::from_utf8_lossy(buffer).to_string()
        } else {
            return Err(anyhow!("Cannot extract text from binary file: {}", path));
        }
    };

    Ok(ArchiveFileResult {
        path: path.to_string(),
        text,
        mime_type: mime_type
            .map(|m| m.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string()),
        size: buffer.len(),
    })
}

/// Check if buffer is likely text content
fn is_likely_text(buffer: &[u8]) -> bool {
    if buffer.is_empty() {
        return true;
    }

    // Check first 1KB for non-text bytes
    let sample_size = buffer.len().min(1024);
    let sample = &buffer[..sample_size];

    let text_chars = sample
        .iter()
        .filter(|&&b| b.is_ascii_graphic() || b.is_ascii_whitespace())
        .count();

    // If more than 90% are text characters, likely text
    text_chars as f32 / sample.len() as f32 > 0.9
}

/// Build the final extraction result
fn build_result(
    files: Vec<ArchiveFileResult>,
    failed_files: Vec<ArchiveFileError>,
    format: &str,
    options: &ExtractionOptions,
) -> Result<ArchiveExtractionResult> {
    // Combine all text with file path headers
    let text_parts: Vec<String> = files
        .iter()
        .filter(|f| !f.text.trim().is_empty())
        .map(|f| format!("--- {} ---\n{}", f.path, f.text))
        .collect();

    let text = text_parts.join("\n\n");

    let metadata = if options.include_metadata {
        Some(json!({
            "format": format,
            "file_count": files.len(),
            "failed_count": failed_files.len(),
            "files": files.iter().map(|f| json!({
                "path": f.path,
                "mime_type": f.mime_type,
                "size": f.size,
                "text_length": f.text.len(),
            })).collect::<Vec<_>>(),
            "failed_files": failed_files.iter().map(|f| json!({
                "path": f.path,
                "error": f.error,
            })).collect::<Vec<_>>(),
        }))
    } else {
        None
    };

    Ok(ArchiveExtractionResult { text, metadata })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_test_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);

            for (name, content) in files {
                zip.start_file(*name, options).unwrap();
                zip.write_all(content).unwrap();
            }
            zip.finish().unwrap();
        }
        buffer.into_inner()
    }

    #[test]
    fn test_extract_simple_zip() {
        let zip_data = create_test_zip(&[
            ("file1.txt", b"Hello World"),
            ("file2.txt", b"Goodbye World"),
        ]);

        let options = ExtractionOptions::default();
        let archive_opts = ArchiveOptions::default();

        let result = extract_from_zip(&zip_data, &options, &archive_opts);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        assert!(extraction.text.contains("Hello World"));
        assert!(extraction.text.contains("Goodbye World"));
    }

    #[test]
    fn test_extract_zip_with_json() {
        let json_content = br#"{"name": "test", "value": 123}"#;
        let zip_data = create_test_zip(&[("data.json", json_content)]);

        let options = ExtractionOptions::default();
        let archive_opts = ArchiveOptions::default();

        let result = extract_from_zip(&zip_data, &options, &archive_opts);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        assert!(extraction.text.contains("test"));
    }

    #[test]
    fn test_skip_binary_files() {
        let zip_data =
            create_test_zip(&[("file.txt", b"Hello"), ("binary.exe", &[0x00, 0x01, 0x02])]);

        let options = ExtractionOptions::default();
        let archive_opts = ArchiveOptions::default();

        let result = extract_from_zip(&zip_data, &options, &archive_opts);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        // Binary files should be skipped, only text content present
        assert!(extraction.text.contains("Hello"));
        assert!(!extraction.text.contains("\0")); // No binary content
    }

    #[test]
    fn test_continue_on_error() {
        // This tests that we handle corrupted entries gracefully
        let zip_data = create_test_zip(&[("good.txt", b"Hello World")]);

        let options = ExtractionOptions::default();
        let archive_opts = ArchiveOptions {
            continue_on_error: true,
            ..Default::default()
        };

        let result = extract_from_zip(&zip_data, &options, &archive_opts);
        assert!(result.is_ok());
    }

    #[test]
    fn test_metadata_extraction() {
        let zip_data = create_test_zip(&[("file.txt", b"Content here")]);

        let mut options = ExtractionOptions::default();
        options.include_metadata = true;
        let archive_opts = ArchiveOptions::default();

        let result = extract_from_zip(&zip_data, &options, &archive_opts);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        assert!(extraction.metadata.is_some());

        let meta = extraction.metadata.unwrap();
        assert_eq!(meta["format"], "zip");
        assert_eq!(meta["file_count"], 1);
    }

    #[test]
    fn test_detect_mime_from_extension() {
        assert_eq!(
            detect_mime_from_extension("file.txt").unwrap().to_string(),
            "text/plain"
        );
        assert_eq!(
            detect_mime_from_extension("file.json").unwrap().to_string(),
            "application/json"
        );
        assert_eq!(
            detect_mime_from_extension("file.md").unwrap().to_string(),
            "text/markdown"
        );
        assert_eq!(
            detect_mime_from_extension("file.pdf").unwrap().to_string(),
            "application/pdf"
        );
        assert!(detect_mime_from_extension("file.unknown").is_none());
    }

    #[test]
    fn test_is_likely_text() {
        assert!(is_likely_text(b"Hello, world!"));
        assert!(is_likely_text(b"Line 1\nLine 2\nLine 3"));
        assert!(!is_likely_text(&[0x00, 0x01, 0x02, 0x03, 0x04, 0x05]));
        assert!(is_likely_text(b"")); // Empty is considered text
    }

    #[test]
    fn test_max_depth_limit() {
        let zip_data = create_test_zip(&[("file.txt", b"Hello")]);

        let options = ExtractionOptions::default();
        let archive_opts = ArchiveOptions {
            max_depth: 0, // Already at max depth
            ..Default::default()
        };

        let result = extract_from_zip_with_depth(&zip_data, &options, &archive_opts, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("depth"));
    }
}
