use anyhow::{Result, anyhow};
use regex::Regex;
use serde_json::{Value, json};
use std::sync::LazyLock;

use crate::extract::config::ExtractionOptions;

/// Result of log file extraction
#[derive(Debug)]
pub struct LogExtractionResult {
    pub text: String,
    pub metadata: Option<Value>,
}

/// Common timestamp patterns for log detection
static TIMESTAMP_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // ISO 8601: 2024-01-15T10:30:45.123Z
        Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}").unwrap(),
        // Common log format: 2024-01-15 10:30:45
        Regex::new(r"^\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}").unwrap(),
        // Syslog: Jan 15 10:30:45
        Regex::new(r"^[A-Z][a-z]{2}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2}").unwrap(),
        // Bracketed timestamp: [2024-01-15 10:30:45]
        Regex::new(r"^\[\d{4}-\d{2}-\d{2}").unwrap(),
        // Unix timestamp prefix
        Regex::new(r"^\d{10,13}\s").unwrap(),
        // Apache/nginx: 15/Jan/2024:10:30:45
        Regex::new(r"^\d{2}/[A-Z][a-z]{2}/\d{4}:\d{2}:\d{2}:\d{2}").unwrap(),
    ]
});

/// Common log level patterns
static LOG_LEVEL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(DEBUG|INFO|WARN|WARNING|ERROR|FATAL|CRITICAL|TRACE|NOTICE)\b").unwrap()
});

/// Extract text from log file with options
pub(crate) fn extract_with_options(
    bytes: &[u8],
    options: &ExtractionOptions,
) -> Result<LogExtractionResult> {
    let content =
        std::str::from_utf8(bytes).map_err(|e| anyhow!("Invalid UTF-8 in log file: {}", e))?;

    let lines: Vec<&str> = content.lines().collect();
    let mut processed_lines = Vec::new();
    let mut entry_count = 0;
    let mut log_levels: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut detected_format: Option<String> = None;

    for line in &lines {
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            if options.preserve_formatting {
                processed_lines.push(String::new());
            }
            continue;
        }

        // Detect timestamp format on first match
        if detected_format.is_none() {
            for (i, pattern) in TIMESTAMP_PATTERNS.iter().enumerate() {
                if pattern.is_match(trimmed) {
                    detected_format = Some(format!("pattern_{}", i));
                    break;
                }
            }
        }

        // Count log levels
        if let Some(caps) = LOG_LEVEL_PATTERN.captures(trimmed) {
            let level = caps.get(1).unwrap().as_str().to_uppercase();
            *log_levels.entry(level).or_insert(0) += 1;
        }

        // Check if this is a new log entry (starts with timestamp)
        let is_new_entry = TIMESTAMP_PATTERNS.iter().any(|p| p.is_match(trimmed));

        if is_new_entry {
            entry_count += 1;
        }

        if options.preserve_formatting {
            processed_lines.push((*line).to_string());
        } else {
            // Clean up the line - remove excessive whitespace
            let cleaned: String = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
            processed_lines.push(cleaned);
        }
    }

    let text = processed_lines.join("\n");

    let metadata = if options.include_metadata {
        Some(json!({
            "format": "log",
            "entry_count": entry_count,
            "line_count": lines.len(),
            "detected_format": detected_format,
            "log_levels": log_levels,
            "has_timestamps": detected_format.is_some(),
        }))
    } else {
        None
    };

    Ok(LogExtractionResult { text, metadata })
}

/// Detect if content appears to be a log file
pub(crate) fn is_log_file(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().take(10).collect();
    if lines.is_empty() {
        return false;
    }

    // Count lines that start with a timestamp pattern
    let timestamp_lines = lines
        .iter()
        .filter(|line| {
            let trimmed = line.trim();
            TIMESTAMP_PATTERNS.iter().any(|p| p.is_match(trimmed))
        })
        .count();

    // If more than 30% of first 10 lines have timestamps, likely a log file
    timestamp_lines as f32 / lines.len() as f32 > 0.3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_log() {
        let log = b"2024-01-15 10:30:45 INFO Starting application\n2024-01-15 10:30:46 DEBUG Loading config";
        let result = extract_with_options(log, &ExtractionOptions::default());
        assert!(result.is_ok());
        let text = result.unwrap().text;
        assert!(text.contains("Starting application"));
        assert!(text.contains("Loading config"));
    }

    #[test]
    fn test_log_entry_count() {
        let log = b"2024-01-15 10:30:45 INFO First\n2024-01-15 10:30:46 INFO Second\n2024-01-15 10:30:47 INFO Third";
        let options = ExtractionOptions {
            include_metadata: true,
            ..Default::default()
        };

        let result = extract_with_options(log, &options);
        assert!(result.is_ok());
        let extraction = result.unwrap();
        // Check entry_count in metadata instead
        let meta = extraction.metadata.unwrap();
        assert_eq!(meta["entry_count"], 3);
    }

    #[test]
    fn test_log_level_counting() {
        let log = b"2024-01-15 10:30:45 INFO First\n2024-01-15 10:30:46 ERROR Problem\n2024-01-15 10:30:47 INFO Second";
        let options = ExtractionOptions {
            include_metadata: true,
            ..Default::default()
        };

        let result = extract_with_options(log, &options);
        assert!(result.is_ok());
        let meta = result.unwrap().metadata.unwrap();
        let levels = meta["log_levels"].as_object().unwrap();
        assert_eq!(levels.get("INFO").and_then(|v| v.as_u64()), Some(2));
        assert_eq!(levels.get("ERROR").and_then(|v| v.as_u64()), Some(1));
    }

    #[test]
    fn test_iso_timestamp_detection() {
        let log = b"2024-01-15T10:30:45.123Z INFO Message";
        let options = ExtractionOptions {
            include_metadata: true,
            ..Default::default()
        };

        let result = extract_with_options(log, &options);
        assert!(result.is_ok());
        let meta = result.unwrap().metadata.unwrap();
        assert!(meta["has_timestamps"].as_bool().unwrap());
    }

    #[test]
    fn test_syslog_format() {
        let log = b"Jan 15 10:30:45 hostname app[1234]: Message here";
        let result = is_log_file(std::str::from_utf8(log).unwrap());
        assert!(result);
    }

    #[test]
    fn test_not_a_log_file() {
        let text = "Hello world\nThis is just regular text\nNo timestamps here";
        let result = is_log_file(text);
        assert!(!result);
    }

    #[test]
    fn test_bracketed_timestamp() {
        let log = b"[2024-01-15 10:30:45] INFO Message";
        let result = is_log_file(std::str::from_utf8(log).unwrap());
        assert!(result);
    }

    #[test]
    fn test_multiline_log_entries() {
        let log = b"2024-01-15 10:30:45 ERROR Exception occurred\n    at Main.run(Main.java:42)\n    at App.main(App.java:10)\n2024-01-15 10:30:46 INFO Continuing";
        let options = ExtractionOptions {
            include_metadata: true,
            ..Default::default()
        };

        let result = extract_with_options(log, &options);
        assert!(result.is_ok());
        // Only lines starting with timestamp count as entries - check in metadata
        let meta = result.unwrap().metadata.unwrap();
        assert_eq!(meta["entry_count"], 2);
    }

    #[test]
    fn test_preserve_formatting() {
        let log = b"2024-01-15 10:30:45    INFO    Spaced message";
        let options = ExtractionOptions {
            preserve_formatting: true,
            ..Default::default()
        };

        let result = extract_with_options(log, &options);
        assert!(result.is_ok());
        let text = result.unwrap().text;
        // Original spacing preserved
        assert!(text.contains("    INFO    "));
    }

    #[test]
    fn test_clean_formatting() {
        let log = b"2024-01-15 10:30:45    INFO    Spaced message";
        let options = ExtractionOptions {
            preserve_formatting: false,
            ..Default::default()
        };

        let result = extract_with_options(log, &options);
        assert!(result.is_ok());
        let text = result.unwrap().text;
        // Whitespace normalized
        assert!(!text.contains("    "));
    }
}
