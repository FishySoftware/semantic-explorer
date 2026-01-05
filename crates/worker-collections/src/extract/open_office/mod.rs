mod open_document;
mod open_presentation;
mod open_spreadsheet;

use anyhow::Result;

pub(crate) fn extract_text_from_document(content: &[u8]) -> Result<String> {
    open_document::extract_text(content)
}

pub(crate) fn extract_text_from_spreadsheet(content: &[u8]) -> Result<String> {
    open_spreadsheet::extract_text(content)
}

pub(crate) fn extract_text_from_presentation(content: &[u8]) -> Result<String> {
    open_presentation::extract_text(content)
}
