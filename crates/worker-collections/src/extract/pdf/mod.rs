use anyhow::{Result, anyhow};

use pdf_extract::extract_text_from_mem;
pub(crate) fn extract_text_from_pdf(bytes: &[u8]) -> Result<String> {
    extract_text_from_mem(bytes).map_err(|error| anyhow!(error.to_string()))
}
