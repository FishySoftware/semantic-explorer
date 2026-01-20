use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ExtractionStrategy {
    #[default]
    PlainText,
    StructurePreserving,
    Markdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum TableFormat {
    Markdown,
    Csv,
    #[default]
    PlainText,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum HeadingFormat {
    Markdown,
    #[default]
    PlainText,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionOptions {
    #[serde(default)]
    pub preserve_formatting: bool,

    #[serde(default)]
    pub extract_tables: bool,

    #[serde(default)]
    pub table_format: TableFormat,

    #[serde(default)]
    pub preserve_headings: bool,

    #[serde(default)]
    pub heading_format: HeadingFormat,

    #[serde(default)]
    pub preserve_lists: bool,

    #[serde(default)]
    pub preserve_code_blocks: bool,

    /// Extract metadata from document (author, title, dates, etc.)
    #[serde(default)]
    pub include_metadata: bool,

    /// Append extracted metadata as formatted text at the end of content
    /// This enables metadata to be chunked alongside the main content
    #[serde(default)]
    pub append_metadata_to_text: bool,
}

impl Default for ExtractionOptions {
    fn default() -> Self {
        Self {
            preserve_formatting: false,
            extract_tables: true,
            table_format: TableFormat::PlainText,
            preserve_headings: false,
            heading_format: HeadingFormat::PlainText,
            preserve_lists: false,
            preserve_code_blocks: false,
            include_metadata: false,
            append_metadata_to_text: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionConfig {
    #[serde(default)]
    pub strategy: ExtractionStrategy,

    #[serde(default)]
    pub options: ExtractionOptions,
}

impl Default for ExtractionConfig {
    fn default() -> Self {
        Self {
            strategy: ExtractionStrategy::PlainText,
            options: ExtractionOptions::default(),
        }
    }
}

/// Result of text extraction with optional metadata
#[derive(Debug, Clone)]
pub struct ExtractionOutput {
    pub text: String,
    pub metadata: Option<serde_json::Value>,
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extraction_config_defaults() {
        let config = ExtractionConfig::default();
        assert!(matches!(config.strategy, ExtractionStrategy::PlainText));
        assert!(!config.options.preserve_formatting);
        assert!(config.options.extract_tables);
        assert!(!config.options.include_metadata);
    }

    #[test]
    fn test_deserialize_extraction_config() {
        let json = json!({
            "strategy": "structure_preserving",
            "options": {
                "preserve_formatting": true,
                "extract_tables": false,
                "heading_format": "markdown",
                "include_metadata": true
            }
        });

        let config: ExtractionConfig = serde_json::from_value(json).unwrap();
        assert!(matches!(
            config.strategy,
            ExtractionStrategy::StructurePreserving
        ));
        assert!(config.options.preserve_formatting);
        assert!(!config.options.extract_tables);
        assert!(matches!(
            config.options.heading_format,
            HeadingFormat::Markdown
        ));
        assert!(config.options.include_metadata);
    }

    #[test]
    fn test_partial_options_override() {
        let json = json!({
            "options": {
                "preserve_code_blocks": true
            }
        });

        let config: ExtractionConfig = serde_json::from_value(json).unwrap();
        // Strategy should default to PlainText
        assert!(matches!(config.strategy, ExtractionStrategy::PlainText));
        // Overridden option
        assert!(config.options.preserve_code_blocks);
        // Default options
        assert!(!config.options.preserve_formatting);
    }
}
