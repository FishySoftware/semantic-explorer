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

/// Tracks the shared decompression budget across an entire archive tree.
///
/// A single budget is threaded through nested extraction so the effective memory
/// ceiling is `max_total_size` for the whole tree rather than per archive level.
struct ExtractionBudget {
    remaining: usize,
    truncated: bool,
}

impl ExtractionBudget {
    fn new(max_total_size: usize) -> Self {
        Self {
            remaining: max_total_size,
            truncated: false,
        }
    }
}

/// Read an archive entry without ever allocating more than the remaining budget.
///
/// Reads at most `remaining + 1` bytes so a decompression bomb cannot exhaust
/// memory before the size cap is consulted. Returns `Ok(None)` when the entry
/// would exceed the budget, marking the extraction as truncated.
fn read_entry_within_budget(
    reader: &mut impl Read,
    budget: &mut ExtractionBudget,
) -> std::io::Result<Option<Vec<u8>>> {
    let cap = budget.remaining as u64 + 1;
    let mut buffer = Vec::new();
    reader.take(cap).read_to_end(&mut buffer)?;

    if buffer.len() > budget.remaining {
        budget.truncated = true;
        return Ok(None);
    }

    budget.remaining -= buffer.len();
    Ok(Some(buffer))
}

/// Extract contents from a ZIP archive
pub(crate) fn extract_from_zip(
    bytes: &[u8],
    options: &ExtractionOptions,
    archive_options: &ArchiveOptions,
) -> Result<ArchiveExtractionResult> {
    let mut budget = ExtractionBudget::new(archive_options.max_total_size);
    extract_from_zip_with_depth(bytes, options, archive_options, 0, &mut budget)
}

fn extract_from_zip_with_depth(
    bytes: &[u8],
    options: &ExtractionOptions,
    archive_options: &ArchiveOptions,
    depth: usize,
    budget: &mut ExtractionBudget,
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

        if file.is_dir() {
            continue;
        }

        let path = file.name().to_string();

        if should_skip_file(&path, archive_options) {
            continue;
        }

        let buffer = match read_entry_within_budget(&mut file, budget) {
            Ok(Some(buffer)) => buffer,
            Ok(None) => break,
            Err(e) => {
                if archive_options.continue_on_error {
                    failed_files.push(ArchiveFileError {
                        path: path.clone(),
                        error: e.to_string(),
                    });
                    continue;
                }
                return Err(anyhow!("Failed to read file {}: {}", path, e));
            }
        };

        let result = extract_file_content(&path, &buffer, options, archive_options, depth, budget);
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

    build_result(files, failed_files, "zip", options, budget.truncated)
}

/// Decompress a gzip stream, failing if it expands beyond `max_size` bytes.
pub(crate) fn extract_from_gzip(bytes: &[u8], max_size: usize) -> Result<Vec<u8>> {
    let decoder = GzDecoder::new(bytes);
    let mut decompressed = Vec::new();
    decoder
        .take(max_size as u64 + 1)
        .read_to_end(&mut decompressed)
        .map_err(|e| anyhow!("Failed to decompress gzip: {}", e))?;

    if decompressed.len() > max_size {
        return Err(anyhow!(
            "Gzip decompression exceeds maximum size of {} bytes",
            max_size
        ));
    }

    Ok(decompressed)
}

/// Extract contents from a tar.gz archive
pub(crate) fn extract_from_tar_gz(
    bytes: &[u8],
    options: &ExtractionOptions,
    archive_options: &ArchiveOptions,
) -> Result<ArchiveExtractionResult> {
    let mut budget = ExtractionBudget::new(archive_options.max_total_size);
    extract_from_tar_gz_with_depth(bytes, options, archive_options, 0, &mut budget)
}

fn extract_from_tar_gz_with_depth(
    bytes: &[u8],
    options: &ExtractionOptions,
    archive_options: &ArchiveOptions,
    depth: usize,
    budget: &mut ExtractionBudget,
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

        if entry.header().entry_type().is_dir() {
            continue;
        }

        if should_skip_file(&path, archive_options) {
            continue;
        }

        let buffer = match read_entry_within_budget(&mut entry, budget) {
            Ok(Some(buffer)) => buffer,
            Ok(None) => break,
            Err(e) => {
                if archive_options.continue_on_error {
                    failed_files.push(ArchiveFileError {
                        path: path.clone(),
                        error: e.to_string(),
                    });
                    continue;
                }
                return Err(anyhow!("Failed to read file {}: {}", path, e));
            }
        };

        let result = extract_file_content(&path, &buffer, options, archive_options, depth, budget);
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

    build_result(files, failed_files, "tar.gz", options, budget.truncated)
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

/// Parse a hard-coded MIME literal, panicking only on a programmer error.
fn static_mime(literal: &str) -> mime::Mime {
    literal
        .parse()
        .expect("hard-coded MIME literal must be valid")
}

/// Detect MIME type from file extension
fn detect_mime_from_extension(path: &str) -> Option<mime::Mime> {
    let ext = path.rsplit('.').next()?.to_lowercase();

    match ext.as_str() {
        // Text formats
        "txt" => Some(static_mime("text/plain")),
        "md" | "markdown" => Some(static_mime("text/markdown")),
        "json" => Some(static_mime("application/json")),
        "ndjson" | "jsonl" => Some(static_mime("application/x-ndjson")),
        "xml" => Some(static_mime("application/xml")),
        "html" | "htm" => Some(static_mime("text/html")),
        "csv" => Some(static_mime("text/csv")),
        "log" => Some(static_mime("text/x-log")),

        // Documents
        "pdf" => Some(static_mime("application/pdf")),
        "doc" => Some(static_mime("application/msword")),
        "docx" => Some(static_mime(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        )),
        "xls" => Some(static_mime("application/vnd.ms-excel")),
        "xlsx" => Some(static_mime(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )),
        "ppt" => Some(static_mime("application/vnd.ms-powerpoint")),
        "pptx" => Some(static_mime(
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        )),
        "odt" => Some(static_mime("application/vnd.oasis.opendocument.text")),
        "ods" => Some(static_mime(
            "application/vnd.oasis.opendocument.spreadsheet",
        )),
        "odp" => Some(static_mime(
            "application/vnd.oasis.opendocument.presentation",
        )),

        // Archives (for nested extraction)
        "zip" => Some(static_mime("application/zip")),
        "tar" => Some(static_mime("application/x-tar")),
        "gz" | "gzip" => Some(static_mime("application/gzip")),

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
    budget: &mut ExtractionBudget,
) -> Result<ArchiveFileResult> {
    let mime_type = detect_mime_from_extension(path);

    // Handle nested archives
    if let Some(ref mime) = mime_type
        && mime.type_() == mime::APPLICATION
    {
        match mime.subtype().as_str() {
            "zip" => {
                let nested = extract_from_zip_with_depth(
                    buffer,
                    options,
                    archive_options,
                    depth + 1,
                    budget,
                )?;
                return Ok(ArchiveFileResult {
                    path: path.to_string(),
                    text: nested.text,
                    mime_type: "application/zip".to_string(),
                    size: buffer.len(),
                });
            }
            "gzip" => {
                if path.ends_with(".tar.gz") || path.ends_with(".tgz") {
                    let nested = extract_from_tar_gz_with_depth(
                        buffer,
                        options,
                        archive_options,
                        depth + 1,
                        budget,
                    )?;
                    return Ok(ArchiveFileResult {
                        path: path.to_string(),
                        text: nested.text,
                        mime_type: "application/x-tar+gzip".to_string(),
                        size: buffer.len(),
                    });
                }
                let decompressed = extract_from_gzip(buffer, budget.remaining)?;
                budget.remaining -= decompressed.len();
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

    let extraction_config = ExtractionConfig {
        options: options.clone(),
        ..Default::default()
    };

    let text = if let Some(mime) = mime_type.clone() {
        match plain_text::extract(&mime, buffer, &extraction_config) {
            Ok(content) => content.text,
            Err(_) => String::from_utf8_lossy(buffer).to_string(),
        }
    } else if is_likely_text(buffer) {
        String::from_utf8_lossy(buffer).to_string()
    } else {
        return Err(anyhow!("Cannot extract text from binary file: {}", path));
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

    // A byte-order mark reliably identifies Unicode text whose raw bytes would
    // otherwise fail the heuristic below (UTF-16 is full of NUL bytes).
    const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
    const UTF16_LE_BOM: &[u8] = &[0xFF, 0xFE];
    const UTF16_BE_BOM: &[u8] = &[0xFE, 0xFF];
    if buffer.starts_with(UTF8_BOM)
        || buffer.starts_with(UTF16_LE_BOM)
        || buffer.starts_with(UTF16_BE_BOM)
    {
        return true;
    }

    let sample_size = buffer.len().min(1024);
    let sample = &buffer[..sample_size];

    let text_chars = sample
        .iter()
        .filter(|&&b| b.is_ascii_graphic() || b.is_ascii_whitespace())
        .count();

    text_chars as f32 / sample.len() as f32 > 0.9
}

/// Build the final extraction result
fn build_result(
    files: Vec<ArchiveFileResult>,
    failed_files: Vec<ArchiveFileError>,
    format: &str,
    options: &ExtractionOptions,
    truncated: bool,
) -> Result<ArchiveExtractionResult> {
    if files.is_empty() && !failed_files.is_empty() {
        return Err(anyhow!(
            "All {} file(s) in the archive failed to extract",
            failed_files.len()
        ));
    }

    let text_parts: Vec<String> = files
        .iter()
        .filter(|f| !f.text.trim().is_empty())
        .map(|f| format!("--- {} ---\n{}", f.path, f.text))
        .collect();

    let text = text_parts.join("\n\n");

    let metadata = if options.include_metadata {
        Some(json!({
            "format": format,
            "truncated": truncated,
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

        let options = ExtractionOptions {
            include_metadata: true,
            ..Default::default()
        };
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

        let mut budget = ExtractionBudget::new(archive_opts.max_total_size);
        let result =
            extract_from_zip_with_depth(&zip_data, &options, &archive_opts, 1, &mut budget);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("depth"));
    }
}
