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

    #[serde(default)]
    pub include_metadata: bool,
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

#[derive(Debug)]
pub struct ExtractionResult {
    pub text: String,
    pub metadata: Option<serde_json::Value>,
}
