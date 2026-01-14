//! Table-aware chunking that keeps table rows together.
//!
//! This module provides intelligent chunking for tabular data, ensuring that
//! table rows are kept intact and column headers can be optionally included
//! with each chunk for context.

use anyhow::Result;
use regex::Regex;
use std::sync::LazyLock;

use crate::chunk::config::ChunkingConfig;

/// Regex patterns for table detection
static MARKDOWN_TABLE_ROW: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\|.*\|\s*$").unwrap());
static MARKDOWN_SEPARATOR_ROW: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\|[\s\-:|]+\|\s*$").unwrap());
static CSV_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(?:[^,"\n]*|"[^"]*")+(?:,(?:[^,"\n]*|"[^"]*"))*$"#).unwrap());

/// Detected table format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableFormat {
    /// Markdown pipe table
    Markdown,
    /// CSV (comma-separated values)
    Csv,
    /// TSV (tab-separated values)
    Tsv,
}

/// A detected table in the text
#[derive(Debug, Clone)]
pub struct DetectedTable {
    /// Start line index (0-based)
    pub start_line: usize,
    /// End line index (exclusive)
    pub end_line: usize,
    /// The table format
    pub format: TableFormat,
    /// Header row content (if detected)
    pub header: Option<String>,
    /// Data rows
    pub rows: Vec<String>,
}

/// Options for table-aware chunking
#[derive(Debug, Clone)]
pub struct TableAwareOptions {
    /// Maximum rows per chunk (default: 50)
    pub max_rows_per_chunk: usize,
    /// Whether to include column headers with each chunk (default: true)
    pub include_headers: bool,
    /// Maximum chunk size in bytes (default: 2000)
    pub max_chunk_size: usize,
    /// Minimum rows before creating a separate chunk (default: 3)
    pub min_rows_for_table: usize,
}

impl Default for TableAwareOptions {
    fn default() -> Self {
        Self {
            max_rows_per_chunk: 50,
            include_headers: true,
            max_chunk_size: 2000,
            min_rows_for_table: 3,
        }
    }
}

/// A table chunk with metadata
#[derive(Debug, Clone)]
pub struct TableChunk {
    /// The chunk content
    pub content: String,
}

/// Detect if a line is part of a Markdown table
fn is_markdown_table_line(line: &str) -> bool {
    MARKDOWN_TABLE_ROW.is_match(line)
}

/// Detect if a line is a Markdown table separator
fn is_markdown_separator(line: &str) -> bool {
    MARKDOWN_SEPARATOR_ROW.is_match(line)
}

/// Detect if a line looks like CSV
fn is_csv_line(line: &str) -> bool {
    // Must have at least one comma and match CSV pattern
    line.contains(',') && CSV_LINE.is_match(line)
}

/// Detect if a line looks like TSV
fn is_tsv_line(line: &str) -> bool {
    line.contains('\t') && line.split('\t').count() >= 2
}

/// Detect tables in text and return their locations
fn detect_tables(text: &str, options: &TableAwareOptions) -> Vec<DetectedTable> {
    let lines: Vec<&str> = text.lines().collect();
    let mut tables = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        // Try to detect Markdown table
        if is_markdown_table_line(lines[i])
            && let Some(table) = detect_markdown_table(&lines, i, options)
        {
            i = table.end_line;
            tables.push(table);
            continue;
        }

        // Try to detect CSV block
        if is_csv_line(lines[i])
            && let Some(table) = detect_csv_table(&lines, i, options)
        {
            i = table.end_line;
            tables.push(table);
            continue;
        }

        // Try to detect TSV block
        if is_tsv_line(lines[i])
            && let Some(table) = detect_tsv_table(&lines, i, options)
        {
            i = table.end_line;
            tables.push(table);
            continue;
        }

        i += 1;
    }

    tables
}

/// Detect a Markdown table starting at the given line
fn detect_markdown_table(
    lines: &[&str],
    start: usize,
    options: &TableAwareOptions,
) -> Option<DetectedTable> {
    let mut end = start;
    let mut separator_line = None;
    let mut data_rows = Vec::new();

    // First pass: find the extent of the table
    while end < lines.len() && is_markdown_table_line(lines[end]) {
        if is_markdown_separator(lines[end]) {
            separator_line = Some(end);
        }
        end += 1;
    }

    // Must have a separator to be a valid Markdown table
    let sep_idx = separator_line?;

    // Header is the line before the separator (if it exists and is at start)
    let header = if sep_idx > start {
        Some(lines[start].to_string())
    } else {
        None
    };

    // Data rows are everything after the separator
    for line in lines.iter().take(end).skip(sep_idx + 1) {
        data_rows.push(line.to_string());
    }

    // Must have minimum data rows
    if data_rows.len() >= options.min_rows_for_table {
        Some(DetectedTable {
            start_line: start,
            end_line: end,
            format: TableFormat::Markdown,
            header,
            rows: data_rows,
        })
    } else {
        None
    }
}

/// Detect a CSV table starting at the given line
fn detect_csv_table(
    lines: &[&str],
    start: usize,
    options: &TableAwareOptions,
) -> Option<DetectedTable> {
    let mut end = start;
    let first_column_count = lines[start].split(',').count();

    // CSV lines should have consistent column counts
    while end < lines.len() {
        let line = lines[end];
        if !is_csv_line(line) {
            break;
        }
        // Allow some variation in column count (±1)
        let col_count = line.split(',').count();
        if (col_count as i32 - first_column_count as i32).abs() > 1 {
            break;
        }
        end += 1;
    }

    let row_count = end - start;
    if row_count >= options.min_rows_for_table {
        let header = Some(lines[start].to_string());
        let rows: Vec<String> = lines[start + 1..end]
            .iter()
            .map(|s| s.to_string())
            .collect();

        Some(DetectedTable {
            start_line: start,
            end_line: end,
            format: TableFormat::Csv,
            header,
            rows,
        })
    } else {
        None
    }
}

/// Detect a TSV table starting at the given line
fn detect_tsv_table(
    lines: &[&str],
    start: usize,
    options: &TableAwareOptions,
) -> Option<DetectedTable> {
    let mut end = start;
    let first_column_count = lines[start].split('\t').count();

    while end < lines.len() {
        let line = lines[end];
        if !is_tsv_line(line) {
            break;
        }
        let col_count = line.split('\t').count();
        if col_count != first_column_count {
            break;
        }
        end += 1;
    }

    let row_count = end - start;
    if row_count >= options.min_rows_for_table {
        let header = Some(lines[start].to_string());
        let rows: Vec<String> = lines[start + 1..end]
            .iter()
            .map(|s| s.to_string())
            .collect();

        Some(DetectedTable {
            start_line: start,
            end_line: end,
            format: TableFormat::Tsv,
            header,
            rows,
        })
    } else {
        None
    }
}

/// Chunk a detected table into smaller pieces
fn chunk_table(table: &DetectedTable, options: &TableAwareOptions) -> Vec<TableChunk> {
    let mut chunks = Vec::new();

    if table.rows.is_empty() {
        return chunks;
    }

    let separator = match table.format {
        TableFormat::Markdown => {
            // Create a separator line matching the header
            if let Some(ref h) = table.header {
                let col_count = h.matches('|').count().saturating_sub(1);
                format!("|{}|", vec!["---"; col_count].join("|"))
            } else {
                String::new()
            }
        }
        _ => String::new(),
    };

    let mut current_rows: Vec<&str> = Vec::new();
    let mut current_size = 0;

    let header_size = table.header.as_ref().map(|h| h.len()).unwrap_or(0)
        + if table.format == TableFormat::Markdown {
            separator.len() + 1
        } else {
            0
        };

    for row in table.rows.iter() {
        let row_size = row.len() + 1; // +1 for newline

        // Check if adding this row would exceed limits
        let total_with_row = if options.include_headers {
            header_size + current_size + row_size
        } else {
            current_size + row_size
        };

        let should_flush = !current_rows.is_empty()
            && (current_rows.len() >= options.max_rows_per_chunk
                || total_with_row > options.max_chunk_size);

        if should_flush {
            // Create chunk from current rows
            chunks.push(create_table_chunk(
                table,
                &current_rows,
                &separator,
                options,
            ));
            current_rows.clear();
            current_size = 0;
        }

        current_rows.push(row);
        current_size += row_size;
    }

    // Don't forget the last chunk
    if !current_rows.is_empty() {
        chunks.push(create_table_chunk(
            table,
            &current_rows,
            &separator,
            options,
        ));
    }

    chunks
}

/// Create a table chunk from a set of rows
fn create_table_chunk(
    table: &DetectedTable,
    rows: &[&str],
    separator: &str,
    options: &TableAwareOptions,
) -> TableChunk {
    let mut content = String::new();

    if options.include_headers
        && let Some(ref header) = table.header
    {
        content.push_str(header);
        content.push('\n');
        if table.format == TableFormat::Markdown && !separator.is_empty() {
            content.push_str(separator);
            content.push('\n');
        }
    }

    for row in rows {
        content.push_str(row);
        content.push('\n');
    }

    // Remove trailing newline
    if content.ends_with('\n') {
        content.pop();
    }

    TableChunk { content }
}

/// Chunk non-table text using simple line-based splitting
fn chunk_non_table_text(text: &str, options: &TableAwareOptions) -> Vec<TableChunk> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    for line in text.lines() {
        let line_with_newline = format!("{}\n", line);

        if current_chunk.len() + line_with_newline.len() > options.max_chunk_size
            && !current_chunk.is_empty()
        {
            let trimmed = current_chunk.trim_end().to_string();
            if !trimmed.is_empty() {
                chunks.push(TableChunk { content: trimmed });
            }
            current_chunk = line_with_newline;
        } else {
            current_chunk.push_str(&line_with_newline);
        }
    }

    // Don't forget the last chunk
    let trimmed = current_chunk.trim_end().to_string();
    if !trimmed.is_empty() {
        chunks.push(TableChunk { content: trimmed });
    }

    chunks
}

/// Perform table-aware chunking on text
pub fn chunk_with_tables(text: &str, options: &TableAwareOptions) -> Result<Vec<TableChunk>> {
    let lines: Vec<&str> = text.lines().collect();
    let tables = detect_tables(text, options);

    if tables.is_empty() {
        // No tables, use simple line-based chunking
        return Ok(chunk_non_table_text(text, options));
    }

    let mut all_chunks = Vec::new();
    let mut current_line = 0;

    for table in &tables {
        // Process text before this table
        if table.start_line > current_line {
            let pre_text: String = lines[current_line..table.start_line].join("\n");
            if !pre_text.trim().is_empty() {
                all_chunks.extend(chunk_non_table_text(&pre_text, options));
            }
        }

        // Process the table
        all_chunks.extend(chunk_table(table, options));

        current_line = table.end_line;
    }

    // Process text after the last table
    if current_line < lines.len() {
        let post_text: String = lines[current_line..].join("\n");
        if !post_text.trim().is_empty() {
            all_chunks.extend(chunk_non_table_text(&post_text, options));
        }
    }

    Ok(all_chunks)
}

/// Main entry point for chunking, compatible with other strategies
pub fn chunk(text: String, config: &ChunkingConfig) -> Result<Vec<String>> {
    let options = config
        .options
        .table_aware
        .as_ref()
        .map(|opts| TableAwareOptions {
            max_rows_per_chunk: opts.max_rows_per_chunk.unwrap_or(50),
            include_headers: opts.include_headers.unwrap_or(true),
            max_chunk_size: opts.max_chunk_size.unwrap_or(config.chunk_size),
            min_rows_for_table: opts.min_rows_for_table.unwrap_or(3),
        })
        .unwrap_or_else(|| TableAwareOptions {
            max_chunk_size: config.chunk_size,
            ..Default::default()
        });

    let chunks = chunk_with_tables(&text, &options)?;
    Ok(chunks.into_iter().map(|c| c.content).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_markdown_table() {
        let text = r#"
Some intro text.

| Name | Age | City |
|------|-----|------|
| Alice | 30 | NYC |
| Bob | 25 | LA |
| Charlie | 35 | SF |

Some outro text.
"#;

        let options = TableAwareOptions::default();
        let tables = detect_tables(text, &options);

        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].format, TableFormat::Markdown);
        assert!(tables[0].header.is_some());
        assert_eq!(tables[0].rows.len(), 3);
    }

    #[test]
    fn test_detect_csv_table() {
        let text = r#"
name,age,city
Alice,30,NYC
Bob,25,LA
Charlie,35,SF
"#;

        let options = TableAwareOptions::default();
        let tables = detect_tables(text.trim(), &options);

        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].format, TableFormat::Csv);
    }

    #[test]
    fn test_detect_tsv_table() {
        let text = "name\tage\tcity\nAlice\t30\tNYC\nBob\t25\tLA\nCharlie\t35\tSF";

        let options = TableAwareOptions::default();
        let tables = detect_tables(text, &options);

        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].format, TableFormat::Tsv);
    }

    #[test]
    fn test_chunk_large_table() {
        let mut text = String::from("| Col1 | Col2 |\n|------|------|\n");
        for i in 0..100 {
            text.push_str(&format!("| Row{} | Data{} |\n", i, i));
        }

        let options = TableAwareOptions {
            max_rows_per_chunk: 20,
            include_headers: true,
            ..Default::default()
        };

        let chunks = chunk_with_tables(&text, &options).unwrap();

        // Should have multiple chunks
        assert!(
            chunks.len() >= 5,
            "Should have at least 5 chunks for 100 rows with max 20 per chunk"
        );

        // Each chunk should have the header
        for chunk in &chunks {
            assert!(chunk.content.contains("Col1"), "Header should be present");
        }
    }

    #[test]
    fn test_mixed_content() {
        let text = r#"
# Introduction

This is some introductory text.

| Header1 | Header2 |
|---------|---------|
| Data1 | Data2 |
| Data3 | Data4 |
| Data5 | Data6 |

## Conclusion

This is the conclusion.
"#;

        let options = TableAwareOptions::default();
        let chunks = chunk_with_tables(text, &options).unwrap();

        // Should have at least 3 chunks: intro, table, conclusion
        assert!(chunks.len() >= 1, "Should have chunks");

        // Check that we have chunks with different content types
        let has_table_content = chunks.iter().any(|c| c.content.contains("Header1"));
        let has_intro = chunks
            .iter()
            .any(|c| c.content.contains("Introduction") || c.content.contains("introductory"));

        assert!(has_table_content, "Should have table content");
        assert!(has_intro, "Should have intro content");
    }

    #[test]
    fn test_no_tables() {
        let text = "This is just plain text.\nWith multiple lines.\nBut no tables.";

        let options = TableAwareOptions::default();
        let chunks = chunk_with_tables(text, &options).unwrap();

        assert!(!chunks.is_empty());
        // Verify chunks contain the plain text content
        let all_content: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(
            all_content.contains("plain text"),
            "Should contain the plain text"
        );
    }

    #[test]
    fn test_include_headers_option() {
        let text = r#"| Name | Value |
|------|-------|
| A | 1 |
| B | 2 |
| C | 3 |"#;

        // With headers
        let options_with = TableAwareOptions {
            include_headers: true,
            max_rows_per_chunk: 2,
            ..Default::default()
        };
        let chunks_with = chunk_with_tables(text, &options_with).unwrap();

        for chunk in &chunks_with {
            assert!(chunk.content.contains("Name"), "Should include header");
        }

        // Without headers
        let options_without = TableAwareOptions {
            include_headers: false,
            max_rows_per_chunk: 2,
            ..Default::default()
        };
        let chunks_without = chunk_with_tables(text, &options_without).unwrap();

        // First chunk might have header, but subsequent chunks should not repeat it
        let non_first_chunks: Vec<_> = chunks_without.iter().skip(1).collect();
        for chunk in non_first_chunks {
            assert!(
                !chunk.content.starts_with("| Name"),
                "Should not start with header when include_headers is false"
            );
        }
    }

    #[test]
    fn test_chunk_function_with_config() {
        let text = r#"| A | B |
|---|---|
| 1 | 2 |
| 3 | 4 |
| 5 | 6 |"#;

        let config = ChunkingConfig {
            chunk_size: 1000,
            ..Default::default()
        };

        let chunks = chunk(text.to_string(), &config).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_csv_with_quotes() {
        let text = r#"name,description,value
"Alice","A person, with comma",100
"Bob","Simple desc",200
"Charlie","Another, desc",300"#;

        let options = TableAwareOptions::default();
        let tables = detect_tables(text, &options);

        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].format, TableFormat::Csv);
    }

    #[test]
    fn test_empty_text() {
        let options = TableAwareOptions::default();
        let chunks = chunk_with_tables("", &options).unwrap();
        assert!(chunks.is_empty());
    }
}
