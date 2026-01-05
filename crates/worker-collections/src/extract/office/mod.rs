mod document;
mod presentation;
mod spreadsheet;

use anyhow::Result;

pub(crate) fn extract_text_from_document(content: &[u8]) -> Result<String> {
    document::extract_text(content)
}

pub(crate) fn extract_text_from_spreadsheet(content: &[u8]) -> Result<String> {
    spreadsheet::extract_text(content)
}

pub(crate) fn extract_text_from_presentation(content: &[u8]) -> Result<String> {
    presentation::extract_text(content)
}
